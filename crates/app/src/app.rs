use std::path::PathBuf;
use std::time::{Duration, Instant};

use doc::{Backup, Document};
use egui::{Align2, Color32, Key, KeyboardShortcut, Modifiers, RichText, Sense};
use live::Live;
use schema::Page;

use crate::pages;
use crate::sandbox::Sandbox;
use crate::theme::{self, icons, Palette};
use crate::widgets;

/// How long display changes stay applied before reverting unless confirmed.
const TRIAL: Duration = Duration::from_secs(15);
/// Title bar and sidebar geometry, shared so the bar's file name lines up with the pages.
const TITLEBAR_HEIGHT: f32 = 48.0;
const TITLEBAR_MARGIN: f32 = 14.0;
const NAV_WIDTH: f32 = 236.0;
const CONTENT_MARGIN: f32 = 28.0;
/// Live mode waits this long after the last edit before saving.
const LIVE_DEBOUNCE: Duration = Duration::from_millis(400);
/// How long to wait for niri to report a reload after saving.
const RELOAD_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, PartialEq)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Error,
}

pub struct Toasts(Vec<(String, Level, Instant)>);

impl Toasts {
    pub fn push(&mut self, level: Level, msg: impl Into<String>) {
        self.0.push((msg.into(), level, Instant::now()));
    }
    pub fn info(&mut self, m: impl Into<String>) {
        self.push(Level::Info, m)
    }
    pub fn ok(&mut self, m: impl Into<String>) {
        self.push(Level::Ok, m)
    }
    pub fn warn(&mut self, m: impl Into<String>) {
        self.push(Level::Warn, m)
    }
    pub fn error(&mut self, m: impl Into<String>) {
        self.push(Level::Error, m)
    }
}

/// Navigation requests from pages.
pub enum Nav {
    Page(Page),
    /// Open the raw editor at a file and 1-based line.
    Raw { file: usize, line: usize },
}

/// What pages get to work with.
pub struct Ctx<'a> {
    pub doc: &'a mut Document,
    pub live: &'a Live,
    pub toasts: &'a mut Toasts,
    /// Last config niri accepted; drives previews while the text is temporarily invalid.
    pub cfg: &'a niri_config::Config,
    /// Schema path of an option to flash and scroll to (from search).
    pub highlight: Option<&'static str>,
    pub nav: &'a mut Option<Nav>,
    /// Set by a page to save with a timed revert (display changes).
    pub request_trial: &'a mut bool,
    /// Set by a page to save now.
    pub request_save: &'a mut bool,
}

struct Pending {
    backups: Vec<Backup>,
    loads_before: u64,
    at: Instant,
    trial: bool,
}

struct Trial {
    backups: Vec<Backup>,
    deadline: Instant,
}

pub struct App {
    doc: Document,
    load_error: Option<String>,
    live: Live,
    page: Page,
    toasts: Toasts,
    cfg: std::rc::Rc<niri_config::Config>,
    cfg_gen: u64,

    search: String,
    search_open: bool,
    highlight: Option<(&'static str, Instant)>,

    pending: Option<Pending>,
    trial: Option<Trial>,
    request_trial: bool,
    request_save: bool,
    live_apply: bool,
    edit_seen: (u64, Instant),

    disk_check: Instant,
    disk_changed: Vec<usize>,
    nav: Option<Nav>,
    pending_shortcut: Option<Shortcut>,
    pages: pages::State,
    shot: Option<crate::shot::Shot>,

    theme: theme::Settings,
    /// What the style was last built from, to rebuild only on change.
    applied: Option<(Palette, theme::Settings)>,
    appearance_open: bool,
    /// Highlighted entry in the search palette.
    search_sel: usize,
    sandbox: Option<Sandbox>,
    sandbox_startup: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: PathBuf) -> Self {
        let ctx = cc.egui_ctx.clone();
        let live = if std::env::var_os("NIRI_SOCKET").is_some() {
            Live::start(move || ctx.request_repaint())
        } else {
            Live::offline()
        };
        let (doc, load_error) = match Document::load(&path) {
            Ok(d) => (d, None),
            Err(e) => (Document::from_text(path, String::new()), Some(format!("{e:#}"))),
        };
        let theme = theme::Settings::load();
        theme::install_fonts(&cc.egui_ctx, theme.system_font);

        let mut app = Self {
            doc,
            load_error,
            live,
            page: Page::Layout,
            toasts: Toasts(Vec::new()),
            cfg: std::rc::Rc::default(),
            cfg_gen: u64::MAX,
            search: String::new(),
            search_open: false,
            highlight: None,
            pending: None,
            trial: None,
            request_trial: false,
            request_save: false,
            live_apply: false,
            edit_seen: (0, Instant::now()),
            disk_check: Instant::now(),
            disk_changed: Vec::new(),
            nav: None,
            pending_shortcut: None,
            pages: pages::State::default(),
            shot: crate::shot::Shot::from_env(),
            theme,
            applied: None,
            appearance_open: false,
            search_sel: 0,
            sandbox: None,
            sandbox_startup: false,
        };
        if let Some(shot) = &app.shot {
            app.page = shot.page;
            app.appearance_open = std::env::var_os("NIRI_STUDIO_APPEARANCE").is_some();
            if let Ok(q) = std::env::var("NIRI_STUDIO_SEARCH") {
                app.search_open = true;
                app.search = q;
            }
            if let Some(i) = std::env::var("NIRI_STUDIO_SELECT").ok().and_then(|s| s.parse().ok()) {
                app.pages.binds.selected = Some(i);
                app.pages.rules.preselect = Some(i);
            }
        }
        app.refresh_cfg();
        if std::env::var_os("NIRI_STUDIO_SANDBOX").is_some() {
            app.start_sandbox();
        }
        app
    }

    fn refresh_cfg(&mut self) {
        if self.cfg_gen == self.doc.generation {
            return;
        }
        self.cfg_gen = self.doc.generation;
        if let Some(cfg) = &self.doc.validation().config {
            self.cfg = cfg.clone();
        }
    }

    fn save(&mut self, trial: bool) {
        if !self.doc.is_dirty() {
            return;
        }
        let errors = self.doc.validation().errors.len();
        if errors > 0 {
            self.toasts.error(format!("Not saved: niri would reject this config ({errors} error{}).", plural(errors)));
            return;
        }
        let loads_before = self.live.state().config_loads;
        match self.doc.save() {
            Ok(out) => {
                self.pending = Some(Pending { backups: out.backups, loads_before, at: Instant::now(), trial });
            }
            Err(e) => self.toasts.error(format!("Saving failed: {e:#}")),
        }
    }

    /// Restores the on-disk state from backups and puts that text back in the editor.
    fn revert_to(&mut self, backups: &[Backup]) {
        for b in backups {
            if let (Ok(text), Some(i)) = (b.read(), self.doc.files.iter().position(|f| f.path == b.of)) {
                self.doc.set_text(i, text);
            }
        }
        if let Err(e) = self.doc.restore_backups(backups) {
            self.toasts.error(format!("Restoring the backup failed: {e:#}"));
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        // Did niri accept what we saved?
        if let Some(p) = &self.pending {
            let (loads, failed, connected) = {
                let s = self.live.state();
                (s.config_loads, s.last_load_failed, s.connected)
            };
            if loads > p.loads_before {
                let p = self.pending.take().unwrap();
                if failed {
                    self.revert_to(&p.backups);
                    self.toasts.error("niri rejected the config, so the previous file was restored.");
                } else if p.trial {
                    self.trial = Some(Trial { backups: p.backups, deadline: Instant::now() + TRIAL });
                } else if !self.live_apply {
                    self.toasts.ok("Saved and applied.");
                }
            } else if p.at.elapsed() > RELOAD_TIMEOUT {
                self.pending = None;
                if connected {
                    self.toasts.warn("Saved, but niri didn't report reloading the config.");
                } else {
                    self.toasts.info("Saved. niri isn't running, so nothing was applied.");
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }

        if let Some(t) = &self.trial {
            if Instant::now() >= t.deadline {
                let t = self.trial.take().unwrap();
                self.revert_to(&t.backups);
                self.toasts.info("Display settings reverted.");
            } else {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }

        if self.request_trial {
            self.request_trial = false;
            self.save(true);
        }
        if self.request_save {
            self.request_save = false;
            self.save(false);
        }

        // Live mode: apply shortly after the last edit.
        if self.edit_seen.0 != self.doc.generation {
            self.edit_seen = (self.doc.generation, Instant::now());
        }
        if self.live_apply && self.doc.is_dirty() && self.pending.is_none() && self.trial.is_none() {
            let since = self.edit_seen.1.elapsed();
            if since >= LIVE_DEBOUNCE {
                if self.doc.validation().ok() {
                    self.save(false);
                }
            } else {
                ctx.request_repaint_after(LIVE_DEBOUNCE - since);
            }
        }

        // Edits made outside the app.
        if self.disk_check.elapsed() > Duration::from_millis(1500) && self.pending.is_none() {
            self.disk_check = Instant::now();
            let changed = self.doc.changed_on_disk();
            if !changed.is_empty() && !self.doc.is_dirty() {
                match self.doc.reload() {
                    Ok(()) => self.toasts.info("The config changed on disk; reloaded."),
                    Err(e) => self.toasts.error(format!("Reloading failed: {e:#}")),
                }
                self.disk_changed.clear();
            } else {
                self.disk_changed = changed;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(1500));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let text_focused = ctx.memory(|m| m.focused().is_some()) && self.page == Page::Raw;
        ctx.input_mut(|i| {
            if i.consume_shortcut(&cmd(Key::S)) {
                self.pending_shortcut = Some(Shortcut::Save);
            }
            if i.consume_shortcut(&cmd(Key::K)) || i.consume_shortcut(&cmd(Key::F)) && self.page != Page::Raw {
                self.pending_shortcut = Some(Shortcut::Search);
            }
            if !text_focused {
                if i.consume_shortcut(&cmd_shift(Key::Z)) || i.consume_shortcut(&cmd(Key::Y)) {
                    self.pending_shortcut = Some(Shortcut::Redo);
                } else if i.consume_shortcut(&cmd(Key::Z)) {
                    self.pending_shortcut = Some(Shortcut::Undo);
                }
            }
        });
        match self.pending_shortcut.take() {
            Some(Shortcut::Save) => self.save(false),
            Some(Shortcut::Search) => self.open_search(),
            Some(Shortcut::Undo) => self.doc.undo(),
            Some(Shortcut::Redo) => self.doc.redo(),
            None => {}
        }
    }

    fn open_search(&mut self) {
        self.search_open = true;
        self.search.clear();
        self.search_sel = 0;
    }

    /// The window's title bar, drawn by the app: brand, the file being edited, search, status,
    /// actions and (when the app decorates itself) window controls. Empty space drags the window;
    /// double-clicking it toggles maximize.
    fn titlebar(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        let ctx = ui.ctx().clone();
        let (maximized, focused) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().focused.unwrap_or(true)));

        // Added first so every widget drawn on top takes its clicks.
        let bar = ui.interact(ui.max_rect(), egui::Id::new("titlebar-drag"), Sense::click_and_drag());
        if bar.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if bar.double_clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            brand(ui, &p, focused);
            // Line the file name up with the page content below.
            let content_left = ui.max_rect().left() + NAV_WIDTH + CONTENT_MARGIN - TITLEBAR_MARGIN;
            ui.add_space((content_left - ui.cursor().left()).max(12.0));
            let fg = if focused { p.muted } else { p.faint };
            ui.label(RichText::new(icons::FILE_TEXT).size(15.0).color(p.faint));
            let main = &self.doc.main().path;
            let mut label = main.display().to_string();
            if let Some(rest) = label.strip_prefix(&doc::home().display().to_string()) {
                label = format!("~{rest}");
            }
            if self.doc.files.len() > 1 {
                label += &format!("  +{} included", self.doc.files.len() - 1);
            }
            ui.add(egui::Label::new(RichText::new(label).size(12.5).color(fg)).selectable(false).truncate());
            if self.doc.is_dirty() {
                ui.label(RichText::new("●").size(9.0).color(p.warn)).on_hover_text("Unsaved changes");
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if self.theme.custom_titlebar {
                    window_controls(ui, &p, maximized);
                    ui.add_space(4.0);
                    ui.add(egui::Separator::default().vertical().spacing(8.0));
                }
                let dirty = self.doc.is_dirty();
                let save_label = if dirty { format!("{}  Save", icons::FLOPPY_DISK) } else { format!("{}  Saved", icons::CHECK) };
                if widgets::primary_button(ui, dirty, save_label)
                    .on_hover_text("Write the config; niri reloads it automatically (Ctrl+S)")
                    .clicked()
                {
                    self.save(false);
                }
                self.sandbox_button(ui);
                let live = ui
                    .add(egui::Button::selectable(self.live_apply, format!("{} Live", icons::LIGHTNING)))
                    .on_hover_text("Apply every valid change to the running niri as you make it");
                if live.clicked() {
                    self.live_apply = !self.live_apply;
                }
                ui.add_space(2.0);
                ui.add_enabled_ui(self.doc.can_redo(), |ui| {
                    if widgets::icon_button(ui, icons::ARROW_CLOCKWISE, "Redo (Ctrl+Shift+Z)").clicked() {
                        self.doc.redo();
                    }
                });
                ui.add_enabled_ui(self.doc.can_undo(), |ui| {
                    if widgets::icon_button(ui, icons::ARROW_COUNTER_CLOCKWISE, "Undo (Ctrl+Z)").clicked() {
                        self.doc.undo();
                    }
                });
                ui.add_space(4.0);
                self.status_chips(ui);
                ui.add_space(4.0);
                self.search_field(ui);
            });
        });
    }

    /// Looks like a text field; opens the search palette.
    fn search_field(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        let width = (ui.available_width() - 8.0).clamp(120.0, 260.0);
        let (rect, r) = ui.allocate_exact_size(egui::vec2(width, 30.0), Sense::click());
        let fill = if r.hovered() { p.control_hover() } else { p.control };
        ui.painter().rect(rect, ui.visuals().widgets.inactive.corner_radius, fill, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
        let left = rect.left_center() + egui::vec2(10.0, 0.0);
        ui.painter().text(left, Align2::LEFT_CENTER, icons::MAGNIFYING_GLASS, egui::FontId::proportional(14.0), p.muted);
        ui.painter().text(left + egui::vec2(22.0, 0.0), Align2::LEFT_CENTER, "Search settings", egui::FontId::proportional(13.0), p.faint);
        let kbd = egui::Rect::from_center_size(rect.right_center() - egui::vec2(28.0, 0.0), egui::vec2(42.0, 18.0));
        ui.painter().rect(kbd, 4.0, p.bg, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
        ui.painter().text(kbd.center(), Align2::CENTER_CENTER, "Ctrl K", egui::FontId::proportional(10.5), p.muted);
        if r.clicked() {
            self.open_search();
        }
    }

    fn sandbox_button(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        let running = self.sandbox.as_ref().is_some_and(Sandbox::running);
        let text = if running { format!("{} Sandbox", icons::FLASK) } else { format!("{} Sandbox", icons::FLASK) };
        let r = ui
            .add(egui::Button::selectable(running, RichText::new(text).color(if running { p.accent_fg() } else { p.text })))
            .on_hover_text(if running {
                "A nested niri is previewing your unsaved edits. Click to stop it."
            } else {
                "Open a nested niri window running your edited config. Nothing touches your session until you save."
            });
        if r.clicked() {
            if running {
                self.sandbox = None;
            } else {
                self.start_sandbox();
            }
        }
    }

    fn start_sandbox(&mut self) {
        if !self.doc.validation().ok() {
            self.toasts.error("Fix the config errors first; niri would refuse to start with it.");
            return;
        }
        match Sandbox::start(&mut self.doc, self.sandbox_startup) {
            Ok(s) => {
                self.sandbox = Some(s);
                self.toasts.info("Starting a nested niri…");
            }
            Err(e) => self.toasts.error(format!("Couldn't start the sandbox: {e:#}")),
        }
    }

    fn poll_sandbox(&mut self, ctx: &egui::Context) {
        let Some(sb) = &mut self.sandbox else { return };
        let was_ready = sb.ready();
        if let Some(msg) = sb.poll(&mut self.doc) {
            if sb.running() {
                self.toasts.error(msg);
            } else {
                self.toasts.info(msg);
                self.sandbox = None;
                return;
            }
        }
        if sb.ready() && !was_ready {
            self.toasts.ok("Sandbox ready. Edits show up there as you make them.");
        }
        ctx.request_repaint_after(Duration::from_millis(if sb.ready() { 300 } else { 100 }));
    }

    fn sandbox_banner(&mut self, ui: &mut egui::Ui) {
        let Some(sb) = &mut self.sandbox else { return };
        let p = theme::pal(ui.ctx());
        let mut stop = false;
        let mut resync = false;
        let mut spawn = None;
        banner(ui, p.accent_fg(), icons::FLASK, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(if sb.ready() { "Sandbox running" } else { "Sandbox starting…" }).strong().color(p.text));
                ui.label(
                    RichText::new(
                        "Your edits are mirrored into the nested niri window as you make them. The running session and your \
                         config file stay untouched until you save. Inside the window, Mod is Alt.",
                    )
                    .color(p.muted)
                    .size(12.5),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let term = widgets::join_args(&sb.terminal);
                    if ui.add_enabled(sb.ready(), egui::Button::new(format!("{} Open {term}", icons::TERMINAL_WINDOW))).clicked() {
                        spawn = Some(sb.terminal.clone());
                    }
                    if ui
                        .checkbox(&mut sb.startup, "Run startup programs")
                        .on_hover_text("Also start spawn-at-startup programs in the sandbox. Bars and daemons may start twice.")
                        .changed()
                    {
                        self.sandbox_startup = sb.startup;
                        resync = true;
                    }
                    if ui.button(format!("{} Stop", icons::STOP)).clicked() {
                        stop = true;
                    }
                    ui.label(RichText::new(sb.config_path().display().to_string()).size(11.0).color(p.faint))
                        .on_hover_text("The sandbox's copy of your config");
                });
            });
        });
        if let Some(cmd) = spawn {
            if let Err(e) = sb.spawn(cmd) {
                self.toasts.error(format!("{e:#}"));
            }
        }
        if resync {
            if let Err(e) = sb.resync(&mut self.doc) {
                self.toasts.error(format!("{e:#}"));
            }
        }
        if stop {
            self.sandbox = None;
        }
    }

    fn status_chips(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        let s = self.live.state();
        if s.connected {
            let v = s.version.as_deref().unwrap_or("").split_whitespace().next().unwrap_or("");
            widgets::pill(ui, icons::PLUGS_CONNECTED, &format!("niri {v}"), p.ok)
                .on_hover_text(format!("Connected: {} windows, {} outputs", s.windows.len(), s.outputs.len()));
        } else {
            widgets::pill(ui, icons::PLUGS, "offline", p.faint)
                .on_hover_text(s.error.clone().unwrap_or_else(|| "NIRI_SOCKET is not set".into()));
        }
        drop(s);
        let errors = self.doc.validation().errors.len();
        let unsaved_includes = self.doc.validation().unsaved_includes;
        if errors == 0 {
            let r = widgets::pill(ui, icons::CHECK_CIRCLE, "valid", p.ok);
            if unsaved_includes {
                r.on_hover_text("Included files with unsaved edits are validated once saved.");
            } else {
                r.on_hover_text("niri accepts this config");
            }
        } else if widgets::pill(ui, icons::WARNING_CIRCLE, &format!("{errors} error{}", plural(errors)), p.error)
            .on_hover_text("Open the raw editor")
            .clicked()
        {
            self.page = Page::Raw;
        }
    }

    fn nav_panel(&mut self, ui: &mut egui::Ui) {
        const GROUPS: [(&str, &[Page]); 4] = [
            ("Desktop", &[Page::Displays, Page::Layout, Page::Appearance, Page::Animations]),
            ("Behaviour", &[Page::Keybinds, Page::Rules, Page::Input, Page::Gestures]),
            ("Session", &[Page::Startup, Page::Misc]),
            ("Advanced", &[Page::Raw, Page::History]),
        ];
        ui.add_space(4.0);
        egui::Panel::bottom("nav-foot").show_separator_line(true).frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 6, bottom: 0 })).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if let Some(wiki) = self.page.wiki() {
                if side_link(ui, icons::BOOK_OPEN, "niri docs for this page", false).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{}{wiki}", schema::WIKI)));
                }
            }
            if side_link(ui, icons::PALETTE, "Studio appearance", self.appearance_open).clicked() {
                self.appearance_open = !self.appearance_open;
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (gi, (title, pages)) in GROUPS.iter().enumerate() {
            if gi > 0 {
                ui.add_space(12.0);
            }
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                widgets::caption(ui, title);
            });
            ui.add_space(2.0);
            for &page in *pages {
                if nav_item(ui, page, self.page == page, self.page_badge(page)).clicked() {
                    self.page = page;
                }
            }
        }
        });
    }

    /// A small count shown next to a page name, e.g. errors on the raw page.
    fn page_badge(&mut self, page: Page) -> Option<(String, Color32)> {
        let p = self.applied.map_or_else(|| theme::Preset::StudioDark.palette(egui::Theme::Dark), |a| a.0);
        match page {
            Page::Raw => {
                let n = self.doc.validation().errors.len();
                (n > 0).then(|| (n.to_string(), p.error))
            }
            Page::History if self.doc.is_dirty() => Some(("•".into(), p.warn)),
            _ => None,
        }
    }

    fn error_panel(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        let errors = self.doc.validation().errors.clone();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icons::WARNING_CIRCLE).size(16.0).color(p.error));
            ui.label(RichText::new("niri would reject this config").strong().color(p.error));
            ui.label(RichText::new("It can't be saved until these are fixed.").color(p.muted));
        });
        ui.add_space(2.0);
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            for e in errors {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(24.0);
                    if let (Some(file), Some(line)) = (e.file, e.line) {
                        let name = self.doc.files[file].path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        if ui.link(RichText::new(format!("{name}:{line}")).monospace()).clicked() {
                            self.nav = Some(Nav::Raw { file, line });
                        }
                    }
                    ui.label(&e.message);
                    if let Some(l) = &e.label {
                        ui.label(RichText::new(format!("({l})")).color(p.muted));
                    }
                    if let Some(h) = &e.help {
                        ui.label(RichText::new(format!("hint: {h}")).color(p.muted));
                    }
                });
            }
        });
        ui.add_space(8.0);
    }

    fn banners(&mut self, ui: &mut egui::Ui) {
        let p = theme::pal(ui.ctx());
        if let Some(err) = self.load_error.clone() {
            banner(ui, p.warn, icons::WARNING, |ui| {
                ui.label(format!("Couldn't load the config ({err}). Saving will create it."));
                if ui.small_button("Dismiss").clicked() {
                    self.load_error = None;
                }
            });
        }
        if !self.disk_changed.is_empty() {
            banner(ui, p.warn, icons::WARNING, |ui| {
                ui.label("The config was changed by another program while you have unsaved edits.");
                if ui.button("Load theirs (undoable)").clicked() {
                    if let Err(e) = self.doc.reload() {
                        self.toasts.error(format!("{e:#}"));
                    }
                    self.disk_changed.clear();
                }
                if ui.button("Keep mine").clicked() {
                    for &i in &self.disk_changed.clone() {
                        if let Ok(t) = std::fs::read_to_string(&self.doc.files[i].path) {
                            self.doc.mark_saved_as(i, t);
                        }
                    }
                    self.disk_changed.clear();
                }
            });
        }
        if self.doc.files.iter().any(|f| f.kdl.is_err()) && self.page != Page::Raw {
            banner(ui, p.warn, icons::WARNING, |ui| {
                ui.label("The config has a syntax error, so visual editing is paused. Fix it in the raw editor.");
                if ui.button("Open raw editor").clicked() {
                    self.page = Page::Raw;
                }
            });
        }
        self.sandbox_banner(ui);
    }

    fn search_window(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let p = theme::pal(ctx);
        let mut open = true;
        let modal = egui::Modal::new(egui::Id::new("search"))
            .backdrop_color(Color32::from_black_alpha(if p.dark { 120 } else { 60 }))
            .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(10).corner_radius(ctx.global_style().visuals.window_corner_radius))
            .show(ctx, |ui| {
                ui.set_width(620.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(icons::MAGNIFYING_GLASS).size(18.0).color(p.muted));
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search settings and pages…")
                            .font(egui::FontId::proportional(16.0))
                            .frame(egui::Frame::NONE)
                            .desired_width(f32::INFINITY),
                    );
                    if r.changed() {
                        self.search_sel = 0;
                    }
                    r.request_focus();
                });
                ui.add_space(4.0);
                ui.separator();

                let q = self.search.to_lowercase();
                let mut hits: Vec<Hit> = Page::ALL
                    .into_iter()
                    .filter(|pg| !q.is_empty() && pg.title().to_lowercase().contains(&q))
                    .map(Hit::Page)
                    .collect();
                hits.extend(schema::search(&self.search).into_iter().map(Hit::Opt));

                let (down, up, enter, esc) = ui.input_mut(|i| {
                    (
                        i.consume_key(Modifiers::NONE, Key::ArrowDown),
                        i.consume_key(Modifiers::NONE, Key::ArrowUp),
                        i.key_pressed(Key::Enter),
                        i.key_pressed(Key::Escape),
                    )
                });
                if esc {
                    open = false;
                }
                if !hits.is_empty() {
                    if down {
                        self.search_sel = (self.search_sel + 1) % hits.len();
                    }
                    if up {
                        self.search_sel = (self.search_sel + hits.len() - 1) % hits.len();
                    }
                    self.search_sel = self.search_sel.min(hits.len() - 1);
                }

                let mut chosen = None;
                egui::ScrollArea::vertical().max_height(400.0).auto_shrink([false, true]).show(ui, |ui| {
                    if q.is_empty() {
                        ui.add_space(6.0);
                        widgets::caption(ui, "Jump to");
                        for pg in Page::ALL {
                            if search_row(ui, icons::page(pg), pg.title(), "", false).clicked() {
                                chosen = Some(Hit::Page(pg));
                            }
                        }
                    }
                    for (i, hit) in hits.iter().enumerate() {
                        let selected = i == self.search_sel;
                        let r = match hit {
                            Hit::Page(pg) => search_row(ui, icons::page(*pg), pg.title(), "Page", selected),
                            Hit::Opt(o) => {
                                let r = search_row(ui, icons::page(o.page), o.label, &format!("{} › {}", o.page.title(), o.section), selected);
                                if o.help.is_empty() { r } else { r.on_hover_text(o.help) }
                            }
                        };
                        if selected && (down || up) {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() || (enter && selected) {
                            chosen = Some(*hit);
                        }
                    }
                    if !q.is_empty() && hits.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new("No settings match. Anything else can be edited in the raw config.").color(p.muted));
                        ui.add_space(10.0);
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::kbd(ui, &format!("{}{}", icons::ARROW_UP, icons::ARROW_DOWN));
                    ui.label(RichText::new("navigate").size(11.5).color(p.faint));
                    ui.add_space(8.0);
                    widgets::kbd(ui, "Enter");
                    ui.label(RichText::new("open").size(11.5).color(p.faint));
                    ui.add_space(8.0);
                    widgets::kbd(ui, "Esc");
                    ui.label(RichText::new("close").size(11.5).color(p.faint));
                });
                chosen
            });
        if modal.should_close() {
            open = false;
        }
        match modal.inner {
            Some(Hit::Page(pg)) => {
                self.page = pg;
                open = false;
            }
            Some(Hit::Opt(o)) => {
                self.page = o.page;
                self.highlight = Some((o.path, Instant::now()));
                open = false;
            }
            None => {}
        }
        self.search_open = open;
    }

    fn appearance_window(&mut self, ctx: &egui::Context) {
        if !self.appearance_open {
            return;
        }
        let p = theme::pal(ctx);
        let before = self.theme;
        let mut open = true;
        egui::Window::new(RichText::new(format!("{}  Studio appearance", icons::PALETTE)).strong())
            .id(egui::Id::new("appearance"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::RIGHT_TOP, [-20.0, 64.0])
            .default_width(392.0)
            .show(ctx, |ui| {
                ui.set_width(392.0);
                ui.label(RichText::new("How niri studio itself looks. Saved for next time; your niri config isn't touched.").color(p.muted).size(12.5));
                ui.add_space(8.0);
                widgets::caption(ui, "Theme");
                ui.add_space(2.0);
                let system = ctx.system_theme().unwrap_or(egui::Theme::Dark);
                egui::Grid::new("presets").spacing([8.0, 8.0]).show(ui, |ui| {
                    for (i, preset) in theme::Preset::ALL.into_iter().enumerate() {
                        if theme_swatch(ui, preset, system, self.theme.preset == preset).clicked() {
                            self.theme.preset = preset;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(10.0);
                widgets::caption(ui, "Accent color");
                ui.horizontal(|ui| {
                    let focus = matches!(self.theme.accent, theme::Accent::FocusRing);
                    let preset = matches!(self.theme.accent, theme::Accent::Theme);
                    let custom = matches!(self.theme.accent, theme::Accent::Custom(_));
                    if ui.selectable_label(focus, "Focus ring").on_hover_text("Match niri's focus ring color").clicked() {
                        self.theme.accent = theme::Accent::FocusRing;
                    }
                    if ui.selectable_label(preset, "Theme").clicked() {
                        self.theme.accent = theme::Accent::Theme;
                    }
                    if ui.selectable_label(custom, "Custom").clicked() && !custom {
                        self.theme.accent = theme::Accent::Custom(p.accent);
                    }
                    if let theme::Accent::Custom(c) = &mut self.theme.accent {
                        egui::color_picker::color_edit_button_srgba(ui, c, egui::color_picker::Alpha::Opaque);
                    }
                });
                ui.add_space(8.0);
                widgets::caption(ui, "Density");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.theme.density, theme::Density::Comfortable, "Comfortable");
                    ui.selectable_value(&mut self.theme.density, theme::Density::Compact, "Compact");
                });
                ui.add_space(8.0);
                widgets::caption(ui, "Corner radius");
                ui.add(egui::Slider::new(&mut self.theme.radius, 0..=14).suffix(" px"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::switch(ui, &mut self.theme.system_font).changed() {
                        theme::install_fonts(ctx, self.theme.system_font);
                    }
                    ui.label("Use the desktop's font").on_hover_text("The sans-serif and monospace fonts fontconfig picks");
                });
                ui.horizontal(|ui| {
                    if widgets::switch(ui, &mut self.theme.custom_titlebar).changed() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.theme.custom_titlebar));
                    }
                    ui.label("Draw the app's own title bar").on_hover_text(
                        "With window controls, matching the theme. Off asks the compositor for decorations instead \
                         (niri draws none when prefer-no-csd is set).",
                    );
                });
            });
        self.appearance_open = open;
        if self.theme != before {
            if let Err(e) = self.theme.save() {
                self.toasts.error(format!("Saving appearance failed: {e}"));
            }
        }
    }

    fn trial_modal(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.trial else { return };
        let p = theme::pal(ctx);
        let total = TRIAL.as_secs_f32();
        let remaining = t.deadline.saturating_duration_since(Instant::now());
        let left = remaining.as_secs() + 1;
        let mut keep = false;
        let mut revert = false;
        egui::Modal::new(egui::Id::new("trial")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icons::MONITOR).size(22.0).color(p.accent_fg()));
                ui.label(RichText::new("Keep these display settings?").size(18.0).strong());
            });
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Reverting in {left} s unless you confirm.")).color(p.muted));
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(remaining.as_secs_f32() / total).desired_height(4.0).fill(p.accent));
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                keep = widgets::primary_button(ui, true, format!("{} Keep", icons::CHECK)).clicked();
                revert = ui.button(format!("{} Revert now", icons::ARROW_COUNTER_CLOCKWISE)).clicked();
            });
        });
        if keep {
            self.trial = None;
            self.toasts.ok("Display settings kept.");
        } else if revert {
            let t = self.trial.take().unwrap();
            self.revert_to(&t.backups);
        }
    }

    fn toasts_overlay(&mut self, ctx: &egui::Context) {
        const LIFE: f32 = 6.0;
        self.toasts.0.retain(|t| t.2.elapsed().as_secs_f32() < LIFE);
        if self.toasts.0.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(50));
        let p = theme::pal(ctx);
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, [-20.0, -20.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for (msg, level, at) in &self.toasts.0 {
                    let age = at.elapsed().as_secs_f32();
                    ui.set_opacity((age / 0.15).min(1.0).min((LIFE - age) / 0.4).clamp(0.0, 1.0));
                    let (icon, color) = level_style(&p, *level);
                    egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).size(17.0).color(color));
                            ui.add(egui::Label::new(RichText::new(msg).color(p.text)).wrap());
                        });
                    });
                }
            });
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        // By default the app's accent follows your focus ring.
        let ring = &self.cfg.layout.focus_ring;
        let ring = crate::color::from_niri(ring.active_gradient.map_or(ring.active_color, |g| g.from));
        let ring = Color32::from_rgb(ring.r(), ring.g(), ring.b());
        let system = ctx.system_theme().unwrap_or(egui::Theme::Dark);
        let palette = self.theme.resolve(system, ring);
        if self.applied != Some((palette, self.theme)) {
            theme::apply(ctx, &palette, &self.theme);
            self.applied = Some((palette, self.theme));
        }
    }
}

#[derive(Clone, Copy)]
enum Hit {
    Page(Page),
    Opt(&'static schema::Opt),
}

#[derive(Clone, Copy)]
enum Shortcut {
    Save,
    Search,
    Undo,
    Redo,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        pages::rules::set_context(&ctx);
        self.refresh_cfg();
        self.apply_theme(&ctx);
        self.shortcuts(&ctx);
        self.poll(&ctx);
        self.poll_sandbox(&ctx);

        if let Some(nav) = self.nav.take() {
            match nav {
                Nav::Page(p) => self.page = p,
                Nav::Raw { file, line } => {
                    self.page = Page::Raw;
                    self.pages.raw.jump(file, line);
                }
            }
        }

        let p = theme::pal(&ctx);
        egui::Panel::top("titlebar")
            .exact_size(TITLEBAR_HEIGHT)
            .resizable(false)
            .show_separator_line(true)
            .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(TITLEBAR_MARGIN as i8, 0)))
            .show(ui, |ui| self.titlebar(ui));
        egui::Panel::left("nav")
            .exact_size(NAV_WIDTH)
            .resizable(false)
            .show_separator_line(true)
            .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin { left: 10, right: 10, top: 10, bottom: 10 }))
            .show(ui, |ui| self.nav_panel(ui));
        if !self.doc.validation().ok() {
            egui::Panel::bottom("errors")
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(p.tint(p.error, 0.08))
                        .stroke(egui::Stroke::new(1.0, p.tint(p.error, 0.35)))
                        .inner_margin(egui::Margin::symmetric(24, 0)),
                )
                .show(ui, |ui| self.error_panel(ui));
        }

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin { left: CONTENT_MARGIN as i8, right: 24, top: 6, bottom: 0 }))
            .show(ui, |ui| {
                self.banners(ui);
                let highlight = self
                    .highlight
                    .filter(|(_, t)| t.elapsed() < Duration::from_secs(3))
                    .map(|(p, _)| p);
                let mut cx = Ctx {
                    doc: &mut self.doc,
                    live: &self.live,
                    toasts: &mut self.toasts,
                    cfg: &self.cfg,
                    highlight,
                    nav: &mut self.nav,
                    request_trial: &mut self.request_trial,
                    request_save: &mut self.request_save,
                };
                pages::show(self.page, ui, &mut cx, &mut self.pages);
            });

        if self.theme.custom_titlebar {
            resize_edges(&ctx);
        }
        self.search_window(&ctx);
        self.appearance_window(&ctx);
        self.trial_modal(&ctx);
        self.toasts_overlay(&ctx);

        if self.shot.as_mut().is_some_and(|s| s.tick(&ctx)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn level_style(p: &Palette, level: Level) -> (&'static str, Color32) {
    match level {
        Level::Info => (icons::INFO, p.accent_fg()),
        Level::Ok => (icons::CHECK_CIRCLE, p.ok),
        Level::Warn => (icons::WARNING, p.warn),
        Level::Error => (icons::X_CIRCLE, p.error),
    }
}

/// A tinted notice across the page with a leading icon.
fn banner(ui: &mut egui::Ui, color: Color32, icon: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = theme::pal(ui.ctx());
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(p.tint(color, 0.10))
        .stroke(egui::Stroke::new(1.0, p.tint(color, 0.40)))
        .corner_radius(ui.visuals().window_corner_radius)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width((ui.available_width()).min(1100.0 - 30.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(color));
                ui.horizontal_wrapped(add);
            });
        });
    ui.add_space(6.0);
}

/// The app's mark and name at the left of the title bar: three columns, the middle one
/// focused. Dims with the window, as title bars do.
fn brand(ui: &mut egui::Ui, p: &Palette, focused: bool) {
    let accent = if focused { p.accent } else { p.faint };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::hover());
    ui.painter().rect_filled(rect, 7.0, p.tint(accent, 0.22));
    let inner = rect.shrink(6.0);
    let w = inner.width() / 3.0;
    for i in 0..3 {
        let col = egui::Rect::from_min_size(inner.left_top() + egui::vec2(i as f32 * w + 1.0, 0.0), egui::vec2(w - 2.0, inner.height()));
        if i == 1 {
            ui.painter().rect_filled(col, 1.5, accent);
        } else {
            ui.painter().rect_stroke(col, 1.5, egui::Stroke::new(1.0, p.muted), egui::StrokeKind::Inside);
        }
    }
    ui.add_space(2.0);
    let color = if focused { p.text } else { p.muted };
    ui.add(egui::Label::new(RichText::new("niri studio").size(14.5).strong().color(color)).selectable(false));
}

/// Maximize and close, plus minimize where the compositor has such a thing (niri doesn't).
fn window_controls(ui: &mut egui::Ui, p: &Palette, maximized: bool) {
    let ctx = ui.ctx().clone();
    if control_button(ui, p, icons::X, "Close", true).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let (icon, tip) = if maximized { (icons::CORNERS_IN, "Restore") } else { (icons::CORNERS_OUT, "Maximize") };
    if control_button(ui, p, icon, tip, false).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    if std::env::var_os("NIRI_SOCKET").is_none() && control_button(ui, p, icons::MINUS, "Minimize", false).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// A round title bar button; the close button turns red on hover.
fn control_button(ui: &mut egui::Ui, p: &Palette, icon: &str, tip: &str, danger: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::click());
    let hover = ui.ctx().animate_bool_responsive(r.id, r.hovered());
    let (fill, fg) = if danger {
        (theme::mix(p.control, p.error, hover), theme::mix(p.muted, Color32::WHITE, hover))
    } else {
        (theme::mix(p.control, p.control_active(), hover), theme::mix(p.muted, p.text, hover))
    };
    ui.painter().circle_filled(rect.center(), 12.0, fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, egui::FontId::proportional(13.0), fg);
    r.on_hover_text(tip)
}

/// Without compositor decorations the window has no resize borders of its own; give it thin
/// ones along its edges (only when it isn't maximized, and only where nothing else is).
fn resize_edges(ctx: &egui::Context) {
    use egui::ResizeDirection as D;
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let (edge, corner) = (5.0, 12.0);
    let zones = [
        (D::NorthWest, egui::Rect::from_min_size(screen.left_top(), egui::vec2(corner, corner)), egui::CursorIcon::ResizeNorthWest),
        (D::NorthEast, egui::Rect::from_min_size(screen.right_top() - egui::vec2(corner, 0.0), egui::vec2(corner, corner)), egui::CursorIcon::ResizeNorthEast),
        (D::SouthWest, egui::Rect::from_min_size(screen.left_bottom() - egui::vec2(0.0, corner), egui::vec2(corner, corner)), egui::CursorIcon::ResizeSouthWest),
        (D::SouthEast, egui::Rect::from_min_size(screen.right_bottom() - egui::vec2(corner, corner), egui::vec2(corner, corner)), egui::CursorIcon::ResizeSouthEast),
        (D::North, egui::Rect::from_min_max(screen.left_top(), egui::pos2(screen.right(), screen.top() + edge)), egui::CursorIcon::ResizeNorth),
        (D::South, egui::Rect::from_min_max(egui::pos2(screen.left(), screen.bottom() - edge), screen.right_bottom()), egui::CursorIcon::ResizeSouth),
        (D::West, egui::Rect::from_min_max(screen.left_top(), egui::pos2(screen.left() + edge, screen.bottom())), egui::CursorIcon::ResizeWest),
        (D::East, egui::Rect::from_min_max(egui::pos2(screen.right() - edge, screen.top()), screen.right_bottom()), egui::CursorIcon::ResizeEast),
    ];
    egui::Area::new(egui::Id::new("resize-edges")).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        for (i, (dir, rect, cursor)) in zones.into_iter().enumerate() {
            let r = ui.interact(rect, egui::Id::new("resize-edge").with(i), Sense::drag());
            if r.hovered() || r.dragged() {
                ctx.set_cursor_icon(cursor);
            }
            if r.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    });
}

/// A sidebar entry: icon and label, with an accent bar and tinted fill when selected.
fn nav_item(ui: &mut egui::Ui, page: Page, selected: bool, badge: Option<(String, Color32)>) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let height = 32.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
    let rect = rect.shrink2(egui::vec2(0.0, 1.0));
    let painter = ui.painter();
    let hover = ui.ctx().animate_bool_responsive(resp.id.with("hover"), resp.hovered());
    let radius = ui.visuals().widgets.inactive.corner_radius;
    if selected {
        painter.rect_filled(rect, radius, p.tint(p.accent, if p.dark { 0.20 } else { 0.14 }));
        let bar = egui::Rect::from_min_size(rect.left_top() + egui::vec2(0.0, 9.0), egui::vec2(3.0, rect.height() - 18.0));
        painter.rect_filled(bar, 2.0, p.accent);
    } else if hover > 0.0 {
        painter.rect_filled(rect, radius, theme::mix(p.chrome, p.text, 0.06 * hover));
    }
    let fg = if selected { p.accent_fg() } else { theme::mix(p.muted, p.text, hover) };
    painter.text(egui::pos2(rect.left() + 20.0, rect.center().y), Align2::CENTER_CENTER, icons::page(page), egui::FontId::proportional(17.0), fg);
    painter.text(
        egui::pos2(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        page.title(),
        egui::FontId::proportional(14.0),
        if selected { p.text } else { fg },
    );
    if let Some((text, color)) = badge {
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(11.0), p.on_accent());
        let w = (galley.size().x + 10.0).max(18.0);
        let pill = egui::Rect::from_center_size(egui::pos2(rect.right() - 8.0 - w / 2.0, rect.center().y), egui::vec2(w, 18.0));
        painter.rect_filled(pill, 9.0, color);
        painter.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);
    }
    resp
}

/// A quiet link-style row at the bottom of the sidebar.
fn side_link(ui: &mut egui::Ui, icon: &str, text: &str, active: bool) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), Sense::click());
    if r.hovered() || active {
        ui.painter().rect_filled(rect, ui.visuals().widgets.inactive.corner_radius, theme::mix(p.chrome, p.text, 0.06));
    }
    let fg = if r.hovered() || active { p.text } else { p.muted };
    ui.painter().text(rect.left_center() + egui::vec2(20.0, 0.0), Align2::CENTER_CENTER, icon, egui::FontId::proportional(15.0), fg);
    ui.painter().text(rect.left_center() + egui::vec2(38.0, 0.0), Align2::LEFT_CENTER, text, egui::FontId::proportional(12.5), fg);
    r
}

/// One entry in the search palette: icon, title and a muted location.
fn search_row(ui: &mut egui::Ui, icon: &str, title: &str, sub: &str, selected: bool) -> egui::Response {
    let p = theme::pal(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), Sense::click());
    if selected || r.hovered() {
        let fill = if selected { p.tint(p.accent, 0.18) } else { theme::mix(p.surface, p.text, 0.05) };
        ui.painter().rect_filled(rect, ui.visuals().widgets.inactive.corner_radius, fill);
    }
    let fg = if selected { p.accent_fg() } else { p.muted };
    ui.painter().text(rect.left_center() + egui::vec2(18.0, 0.0), Align2::CENTER_CENTER, icon, egui::FontId::proportional(16.0), fg);
    ui.painter().text(rect.left_center() + egui::vec2(38.0, 0.0), Align2::LEFT_CENTER, title, egui::FontId::proportional(14.0), p.text);
    ui.painter().text(rect.right_center() - egui::vec2(12.0, 0.0), Align2::RIGHT_CENTER, sub, egui::FontId::proportional(12.0), p.faint);
    r
}

/// A miniature of a theme: sidebar, page and a card with its accent.
fn theme_swatch(ui: &mut egui::Ui, preset: theme::Preset, system: egui::Theme, selected: bool) -> egui::Response {
    let cur = theme::pal(ui.ctx());
    let p = preset.palette(system);
    let (rect, r) = ui.allocate_exact_size(egui::vec2(120.0, 84.0), Sense::click());
    let painter = ui.painter();
    let preview = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 58.0));
    painter.rect_filled(preview, 8.0, p.bg);
    let side = egui::Rect::from_min_size(preview.min, egui::vec2(32.0, preview.height()));
    painter.rect_filled(side, egui::CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, p.chrome);
    for i in 0..3 {
        let y = side.top() + 12.0 + i as f32 * 10.0;
        let c = if i == 0 { p.accent } else { p.faint };
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(side.left() + 7.0, y), egui::vec2(18.0, 4.0)), 2.0, c);
    }
    let card = egui::Rect::from_min_max(egui::pos2(side.right() + 8.0, preview.top() + 10.0), preview.right_bottom() - egui::vec2(8.0, 10.0));
    painter.rect(card, 5.0, p.surface, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    painter.rect_filled(egui::Rect::from_min_size(card.min + egui::vec2(7.0, 8.0), egui::vec2(40.0, 4.0)), 2.0, p.text);
    painter.rect_filled(egui::Rect::from_min_size(card.min + egui::vec2(7.0, 18.0), egui::vec2(28.0, 4.0)), 2.0, p.muted);
    let sw = egui::Rect::from_min_size(egui::pos2(card.right() - 26.0, card.bottom() - 16.0), egui::vec2(18.0, 10.0));
    painter.rect_filled(sw, 5.0, p.accent);
    if preset == theme::Preset::System {
        // Half light, half dark.
        let other = if p.dark { theme::Preset::StudioLight } else { theme::Preset::StudioDark }.palette(system);
        let half = egui::Rect::from_min_max(egui::pos2(preview.center().x + 18.0, preview.top()), preview.right_bottom());
        painter.rect_filled(half, egui::CornerRadius { ne: 8, se: 8, nw: 0, sw: 0 }, other.bg);
    }
    let stroke = if selected {
        egui::Stroke::new(2.0, cur.accent)
    } else if r.hovered() {
        egui::Stroke::new(1.0, cur.border_strong())
    } else {
        egui::Stroke::new(1.0, cur.border)
    };
    painter.rect_stroke(preview, 8.0, stroke, egui::StrokeKind::Outside);
    let name_color = if selected { cur.text } else { cur.muted };
    painter.text(
        egui::pos2(rect.left() + 2.0, preview.bottom() + 14.0),
        Align2::LEFT_CENTER,
        preset.name(),
        egui::FontId::proportional(12.0),
        name_color,
    );
    r.on_hover_text(preset.name())
}
