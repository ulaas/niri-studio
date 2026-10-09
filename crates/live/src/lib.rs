//! Live data from the running niri over IPC.
//!
//! A background thread follows niri's event stream (windows, workspaces, keyboard layouts and
//! config reload results); outputs are fetched on demand since they have no event.

use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context as _, Result};
use niri_ipc::socket::Socket;
use niri_ipc::state::{EventStreamState, EventStreamStatePart as _};
use niri_ipc::{Event, KeyboardLayouts, Output, Request, Response, Window, Workspace};

#[derive(Default)]
pub struct State {
    pub connected: bool,
    pub version: Option<String>,
    pub error: Option<String>,
    pub windows: Vec<Window>,
    pub workspaces: Vec<Workspace>,
    pub keyboard: Option<KeyboardLayouts>,
    pub outputs: Vec<Output>,
    /// Number of config (re)loads niri reported since we connected, excluding the initial
    /// replay. Compare before/after saving to learn whether niri accepted the file.
    pub config_loads: u64,
    pub last_load_failed: bool,
    pub last_load_at: Option<Instant>,
}

#[derive(Clone)]
pub struct Live {
    state: Arc<Mutex<State>>,
}

type Repaint = Arc<dyn Fn() + Send + Sync>;

impl Live {
    /// Starts following niri. `repaint` is called whenever state changes.
    pub fn start(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let live = Self { state: Arc::default() };
        let repaint: Repaint = Arc::new(repaint);
        let (l, r) = (live.clone(), repaint.clone());
        thread::Builder::new()
            .name("niri-events".into())
            .spawn(move || loop {
                if let Err(e) = l.follow(&r) {
                    let mut s = l.state();
                    s.connected = false;
                    s.error = Some(format!("{e:#}"));
                    drop(s);
                    r();
                }
                thread::sleep(Duration::from_secs(2));
            })
            .expect("spawning event thread");
        live
    }

    /// A disconnected instance, for running without niri.
    pub fn offline() -> Self {
        Self { state: Arc::default() }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn follow(&self, repaint: &Repaint) -> Result<()> {
        let version = match request(Request::Version)? {
            Response::Version(v) => Some(v),
            _ => None,
        };
        let outputs = fetch_outputs().unwrap_or_default();

        let mut socket = Socket::connect().context("connecting to niri")?;
        match socket.send(Request::EventStream)? {
            Ok(Response::Handled) => {}
            Ok(other) => bail!("unexpected reply {other:?}"),
            Err(e) => bail!("{e}"),
        }
        {
            let mut s = self.state();
            s.connected = true;
            s.error = None;
            s.version = version;
            s.outputs = outputs;
        }
        repaint();

        let mut read = socket.read_events();
        let mut es = EventStreamState::default();
        let mut seen_initial_config = false;
        loop {
            let event = read().context("reading niri events")?;
            let config_loaded = match &event {
                Event::ConfigLoaded { failed } => Some(*failed),
                _ => None,
            };
            es.apply(event);

            let mut s = self.state();
            let mut windows: Vec<Window> = es.windows.windows.values().cloned().collect();
            windows.sort_by_key(|w| w.id);
            s.windows = windows;
            let mut ws: Vec<Workspace> = es.workspaces.workspaces.values().cloned().collect();
            ws.sort_by_key(|w| (w.output.clone(), w.idx));
            s.workspaces = ws;
            s.keyboard = es.keyboard_layouts.keyboard_layouts.clone();
            if let Some(failed) = config_loaded {
                if seen_initial_config {
                    s.config_loads += 1;
                    s.last_load_failed = failed;
                    s.last_load_at = Some(Instant::now());
                    drop(s);
                    // Output config may have changed with the reload.
                    let outputs = fetch_outputs().unwrap_or_default();
                    self.state().outputs = outputs;
                } else {
                    seen_initial_config = true;
                }
            }
            repaint();
        }
    }

    pub fn refresh_outputs(&self) {
        if let Ok(outputs) = fetch_outputs() {
            self.state().outputs = outputs;
        }
    }
}

/// One-shot request on a fresh connection.
pub fn request(req: Request) -> Result<Response> {
    let mut socket = Socket::connect().context("connecting to niri (is NIRI_SOCKET set?)")?;
    socket.send(req)?.map_err(|e| anyhow!(e))
}

fn fetch_outputs() -> Result<Vec<Output>> {
    match request(Request::Outputs)? {
        Response::Outputs(map) => {
            let mut v: Vec<Output> = map.into_values().collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(v)
        }
        other => bail!("unexpected reply {other:?}"),
    }
}

/// Runs a config-style action (`focus-workspace 3`, `spawn "foot"`) over IPC. The arguments are
/// parsed by niri-ipc's own command-line definition, the same one `niri msg action` uses, whose
/// subcommands are spelled like the config's actions. Properties become `--key value`.
pub fn run_action(name: &str, args: &[String], props: &[(String, String)]) -> Result<()> {
    match request(Request::Action(parse_action(name, args, props)?))? {
        Response::Handled => Ok(()),
        other => bail!("unexpected reply {other:?}"),
    }
}

fn parse_action(name: &str, args: &[String], props: &[(String, String)]) -> Result<niri_ipc::Action> {
    use clap::Parser as _;

    let mut argv = vec!["action".to_owned(), name.to_owned()];
    for (k, v) in props {
        argv.push(format!("--{k}"));
        argv.push(v.clone());
    }
    if !args.is_empty() {
        argv.push("--".into());
        argv.extend(args.iter().cloned());
    }
    let action = niri_ipc::Action::try_parse_from(&argv).map_err(|e| {
        let msg = e.to_string();
        anyhow!("{}", msg.lines().next().unwrap_or("invalid action").trim_start_matches("error: "))
    })?;
    Ok(action)
}

/// Asks niri to let the user click a window (Esc cancels). Runs on a thread because niri
/// replies only after the click; poll the receiver each frame.
pub fn pick_window(repaint: impl Fn() + Send + 'static) -> std::sync::mpsc::Receiver<Result<Option<Window>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let res = match request(Request::PickWindow) {
            Ok(Response::PickedWindow(w)) => Ok(w),
            Ok(other) => Err(anyhow!("unexpected reply {other:?}")),
            Err(e) => Err(e),
        };
        let _ = tx.send(res);
        repaint();
    });
    rx
}

/// Namespaces of the layer-shell surfaces currently open (bars, launchers, wallpapers…).
pub fn layer_namespaces() -> Vec<String> {
    match request(Request::Layers) {
        Ok(Response::Layers(layers)) => {
            let mut v: Vec<String> = layers.into_iter().map(|l| l.namespace).collect();
            v.sort();
            v.dedup();
            v
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use niri_ipc::Action;

    use super::parse_action;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_config_style_actions() {
        assert!(matches!(parse_action("close-window", &[], &[]).unwrap(), Action::CloseWindow { .. }));
        assert!(matches!(
            parse_action("spawn", &s(&["foot", "-e", "htop"]), &[]).unwrap(),
            Action::Spawn { command } if command == s(&["foot", "-e", "htop"])
        ));
        assert!(matches!(parse_action("spawn-sh", &s(&["a | b"]), &[]).unwrap(), Action::SpawnSh { .. }));
        assert!(matches!(parse_action("focus-workspace", &s(&["3"]), &[]).unwrap(), Action::FocusWorkspace { .. }));
        let props = vec![("show-pointer".to_owned(), "false".to_owned())];
        assert!(matches!(parse_action("screenshot", &[], &props).unwrap(), Action::Screenshot { show_pointer: false, .. }));
        assert!(parse_action("no-such-action", &[], &[]).is_err());
    }

    #[test]
    fn every_catalogued_action_parses() {
        // Arguments mirror schema's ACTIONS hints.
        for (name, arg) in [("set-column-width", "+10%"), ("switch-layout", "next"), ("switch-layout", "1"), ("move-window-to-workspace", "2")] {
            parse_action(name, &s(&[arg]), &[]).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}
