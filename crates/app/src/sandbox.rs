//! Try changes in a nested niri before saving them to the running one.
//!
//! niri runs as an ordinary window when started inside a Wayland session. The sandbox starts
//! one with a private copy of the config (in `$XDG_RUNTIME_DIR`), rewrites that copy whenever
//! the edited config is valid, and lets niri's own file watcher reload it. Your real config and
//! the running session are untouched until you save.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use doc::Document;
use niri_ipc::{Action, Request, Response};

/// Terminals tried, in order, when the config doesn't name one.
const TERMINALS: [&str; 8] = ["foot", "alacritty", "kitty", "ghostty", "wezterm", "ptyxis", "gnome-terminal", "konsole"];

pub struct Sandbox {
    child: Child,
    dir: PathBuf,
    /// Root config inside `dir`.
    config: PathBuf,
    /// The nested niri's IPC socket, once it has created it.
    socket: Option<PathBuf>,
    started: Instant,
    /// Document generation last written, so unchanged configs aren't rewritten.
    written: u64,
    /// Start programs from `spawn-at-startup` too (off by default: bars and daemons would
    /// start a second time).
    pub startup: bool,
    /// Windows to open once the session is up, so layout and decorations have something to show.
    initial_windows: usize,
    pub terminal: Vec<String>,
    pub exited: Option<String>,
}

impl Sandbox {
    pub fn start(doc: &mut Document, startup: bool) -> Result<Self> {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            bail!("A nested session needs a running Wayland session.");
        }
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        remove_stale(&runtime);
        let dir = runtime.join(format!("niri-studio-sandbox-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {dir:?}"))?;
        let config = dir.join("config.kdl");
        write_copy(doc, &dir, startup)?;

        let niri = niri_binary();
        let mut cmd = Command::new(&niri);
        // Take the nested session down with us even if we are killed rather than closed.
        // SAFETY: prctl is async-signal-safe and touches no memory of ours.
        unsafe {
            std::os::unix::process::CommandExt::pre_exec(&mut cmd, || {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let child = cmd
            .arg("--config")
            .arg(&config)
            .env_remove("NIRI_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting {}", niri.display()))?;

        Ok(Self {
            child,
            dir,
            config,
            socket: None,
            started: Instant::now(),
            written: doc.generation,
            startup,
            initial_windows: 2,
            terminal: guess_terminal(doc),
            exited: None,
        })
    }

    pub fn running(&self) -> bool {
        self.exited.is_none()
    }

    pub fn ready(&self) -> bool {
        self.running() && self.socket.is_some()
    }

    pub fn config_path(&self) -> &Path {
        &self.config
    }

    /// Call every frame: notices the window closing, finds the socket, mirrors edits.
    pub fn poll(&mut self, doc: &mut Document) -> Option<String> {
        if self.exited.is_some() {
            return None;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                let quick = self.started.elapsed() < Duration::from_secs(3);
                let msg = if quick && !status.success() {
                    format!("The nested niri exited right away ({status}). Run `niri -c {}` in a terminal to see why.", self.config.display())
                } else {
                    "Nested session closed.".to_owned()
                };
                self.exited = Some(msg.clone());
                return Some(msg);
            }
            Ok(None) => {}
            Err(e) => {
                self.exited = Some(e.to_string());
                return Some(format!("Lost track of the nested session: {e}"));
            }
        }
        if self.socket.is_none() {
            self.socket = find_socket(self.child.id());
            if self.socket.is_some() {
                for _ in 0..self.initial_windows {
                    let _ = self.spawn(self.terminal.clone());
                }
            }
        }
        if doc.generation != self.written && doc.validation().ok() {
            self.written = doc.generation;
            if let Err(e) = write_copy(doc, &self.dir, self.startup) {
                return Some(format!("Updating the nested session failed: {e:#}"));
            }
        }
        None
    }

    /// Rewrites the copy now, e.g. after toggling `startup`.
    pub fn resync(&mut self, doc: &mut Document) -> Result<()> {
        self.written = doc.generation;
        write_copy(doc, &self.dir, self.startup)
    }

    pub fn spawn(&self, command: Vec<String>) -> Result<()> {
        let socket = self.socket.as_deref().context("the nested session is still starting")?;
        if command.is_empty() {
            bail!("no command");
        }
        match live::request_to(socket, Request::Action(Action::Spawn { command }))? {
            Response::Handled => Ok(()),
            other => bail!("unexpected reply {other:?}"),
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exited.get_or_insert_with(|| "Nested session closed.".to_owned());
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Removes copies left behind by instances that didn't exit cleanly.
fn remove_stale(runtime: &Path) {
    let Ok(entries) = std::fs::read_dir(runtime) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(pid) = name.strip_prefix("niri-studio-sandbox-") {
            if !Path::new(&format!("/proc/{pid}")).exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

/// Writes the current text of every file into `dir`, keeping include paths relative to the main
/// config working. Includes outside the config's folder are read from disk by niri as usual.
fn write_copy(doc: &Document, dir: &Path, startup: bool) -> Result<()> {
    let root = doc.main().path.parent().map(Path::to_path_buf).unwrap_or_default();
    for (i, f) in doc.files.iter().enumerate() {
        let target = if i == 0 {
            dir.join("config.kdl")
        } else if let Ok(rel) = f.path.strip_prefix(&root) {
            dir.join(rel)
        } else {
            continue;
        };
        let mut text = f.text.clone();
        if !startup {
            text = without_startup(text);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Replace atomically so niri never reads a half-written file.
        let tmp = target.with_extension("kdl.tmp");
        std::fs::write(&tmp, text).with_context(|| format!("writing {tmp:?}"))?;
        std::fs::rename(&tmp, &target)?;
    }
    Ok(())
}

fn without_startup(text: String) -> String {
    let mut d = Document::from_text(PathBuf::from("config.kdl"), text);
    for name in ["spawn-at-startup", "spawn-sh-at-startup"] {
        let _ = d.remove_all(&doc::p(name));
    }
    d.main().text.clone()
}

/// The niri that is running us when possible (it may not be on PATH when launched from a
/// desktop entry), else whatever `niri` resolves to.
fn niri_binary() -> PathBuf {
    let running = std::env::var("NIRI_SOCKET").ok().and_then(|s| {
        let pid = s.rsplit('.').nth(1)?.to_owned();
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    });
    if let Some(p) = running.filter(|p| p.exists()) {
        return p;
    }
    for p in [doc::home().join(".local/bin/niri"), PathBuf::from("/usr/local/bin/niri"), PathBuf::from("/usr/bin/niri")] {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("niri")
}

/// niri names its socket `niri.<wayland display>.<pid>.sock` in the runtime dir.
fn find_socket(pid: u32) -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let suffix = format!(".{pid}.sock");
    std::fs::read_dir(runtime).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("niri.") && n.ends_with(&suffix))
    })
}

/// The terminal the config's own binds launch, else the first one installed.
fn guess_terminal(doc: &Document) -> Vec<String> {
    let text = &doc.main().text;
    for t in TERMINALS {
        if text.contains(&format!("spawn \"{t}\"")) && on_path(t) {
            return vec![t.to_owned()];
        }
    }
    TERMINALS.iter().find(|t| on_path(t)).map(|t| vec![t.to_string()]).unwrap_or_else(|| vec!["foot".into()])
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(bin).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_startup_programs_only() {
        let text = "spawn-at-startup \"waybar\"\nlayout {\n    gaps 8\n}\nspawn-sh-at-startup \"a | b\"\nbinds {\n    Mod+T { spawn \"foot\"; }\n}\n";
        let out = without_startup(text.to_owned());
        assert!(!out.contains("waybar") && !out.contains("a | b"));
        assert!(out.contains("gaps 8") && out.contains("spawn \"foot\""));
    }

    #[test]
    fn mirrors_includes_next_to_the_copy() {
        let src = std::env::temp_dir().join(format!("niri-studio-sbx-src-{}", std::process::id()));
        let out = std::env::temp_dir().join(format!("niri-studio-sbx-out-{}", std::process::id()));
        std::fs::create_dir_all(src.join("parts")).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(src.join("config.kdl"), "include \"parts/look.kdl\"\nspawn-at-startup \"bar\"\n").unwrap();
        std::fs::write(src.join("parts/look.kdl"), "layout { gaps 4; }\n").unwrap();
        let mut doc = Document::load(&src.join("config.kdl")).unwrap();
        doc.set_text(1, "layout { gaps 12; }\n".into());
        write_copy(&doc, &out, false).unwrap();
        assert!(!std::fs::read_to_string(out.join("config.kdl")).unwrap().contains("bar"));
        // The unsaved include edit reaches the sandbox; the real file is untouched.
        assert!(std::fs::read_to_string(out.join("parts/look.kdl")).unwrap().contains("gaps 12"));
        assert!(std::fs::read_to_string(src.join("parts/look.kdl")).unwrap().contains("gaps 4"));
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&out);
    }
}
