use std::path::PathBuf;
use std::time::{Duration, Instant};

use doc::{Backup, Document};
use egui::{Align2, Color32, Key, KeyboardShortcut, Modifiers, RichText};
use live::Live;
use schema::Page;

use crate::pages;

/// How long display changes stay applied before reverting unless confirmed.
const TRIAL: Duration = Duration::from_secs(15);
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
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        cc.egui_ctx.set_global_style(style);

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
        };
        if let Some(shot) = &app.shot {
            app.page = shot.page;
            if let Some(i) = std::env::var("NIRI_STUDIO_SELECT").ok().and_then(|s| s.parse().ok()) {
                app.pages.binds.selected = Some(i);
                app.pages.rules.preselect = Some(i);
            }
        }
        app.refresh_cfg();
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
            Some(Shortcut::Search) => {
                self.search_open = true;
                self.search.clear();
            }
            Some(Shortcut::Undo) => self.doc.undo(),
            Some(Shortcut::Redo) => self.doc.redo(),
            None => {}
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("niri studio").strong());
            let main = &self.doc.main().path;
            let mut label = main.display().to_string();
            if let Some(rest) = label.strip_prefix(&doc::home().display().to_string()) {
                label = format!("~{rest}");
            }
            if self.doc.files.len() > 1 {
                label += &format!("  +{} included", self.doc.files.len() - 1);
            }
            ui.weak(label);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dirty = self.doc.is_dirty();
                let save = ui
                    .add_enabled(dirty, egui::Button::new(if dirty { "💾 Save" } else { "Saved" }))
                    .on_hover_text("Write the config; niri reloads it automatically (Ctrl+S)");
                if save.clicked() {
                    self.save(false);
                }
                ui.toggle_value(&mut self.live_apply, "⚡ Live")
                    .on_hover_text("Apply every valid change to niri as you make it");
                if ui.add_enabled(self.doc.can_redo(), egui::Button::new("⟳ Redo")).on_hover_text("Ctrl+Shift+Z").clicked() {
                    self.doc.redo();
                }
                if ui.add_enabled(self.doc.can_undo(), egui::Button::new("⟲ Undo")).on_hover_text("Ctrl+Z").clicked() {
                    self.doc.undo();
                }
                ui.separator();
                self.status_chips(ui);
                ui.separator();
                if ui.button("🔍 Search  Ctrl+K").clicked() {
                    self.search_open = true;
                }
            });
        });
    }

    fn status_chips(&mut self, ui: &mut egui::Ui) {
        let errors = self.doc.validation().errors.len();
        let unsaved_includes = self.doc.validation().unsaved_includes;
        if errors == 0 {
            let r = ui.label(RichText::new("✔ valid").color(ok_color(ui)));
            if unsaved_includes {
                r.on_hover_text("Included files with unsaved edits are validated once saved.");
            }
        } else if ui
            .link(RichText::new(format!("✖ {errors} error{}", plural(errors))).color(ui.visuals().error_fg_color))
            .clicked()
        {
            self.page = Page::Raw;
        }
        let s = self.live.state();
        if s.connected {
            let v = s.version.as_deref().unwrap_or("").split_whitespace().next().unwrap_or("");
            ui.label(RichText::new(format!("niri {v}")).color(ok_color(ui)))
                .on_hover_text(format!("Connected: {} windows, {} outputs", s.windows.len(), s.outputs.len()));
            crate::widgets::dot(ui, true, ok_color(ui));
        } else {
            ui.weak("niri offline").on_hover_text(s.error.clone().unwrap_or_else(|| "NIRI_SOCKET is not set".into()));
            crate::widgets::dot(ui, false, ui.visuals().weak_text_color());
        }
    }

    fn nav_panel(&mut self, ui: &mut egui::Ui) {
        const GROUPS: [(&str, &[Page]); 4] = [
            ("DESKTOP", &[Page::Displays, Page::Layout, Page::Appearance, Page::Animations]),
            ("BEHAVIOUR", &[Page::Keybinds, Page::Rules, Page::Input, Page::Gestures]),
            ("SESSION", &[Page::Startup, Page::Misc]),
            ("ADVANCED", &[Page::Raw, Page::History]),
        ];
        ui.add_space(8.0);
        for (gi, (title, pages)) in GROUPS.iter().enumerate() {
            if gi > 0 {
                ui.add_space(10.0);
            }
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new(*title).small().strong().color(ui.visuals().weak_text_color()));
            });
            ui.add_space(2.0);
            for &page in *pages {
                if nav_item(ui, page, self.page == page, self.page_badge(page)).clicked() {
                    self.page = page;
                }
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.0);
            if let Some(wiki) = self.page.wiki() {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.hyperlink_to(RichText::new("📖 niri docs for this page").small(), format!("{}{wiki}", schema::WIKI));
                });
            }
        });
    }

    /// A small count shown next to a page name, e.g. errors on the raw page.
    fn page_badge(&mut self, page: Page) -> Option<(String, Color32)> {
        match page {
            Page::Raw => {
                let n = self.doc.validation().errors.len();
                (n > 0).then(|| (n.to_string(), Color32::from_rgb(220, 80, 80)))
            }
            Page::History if self.doc.is_dirty() => Some(("•".into(), Color32::from_rgb(230, 180, 80))),
            _ => None,
        }
    }

    fn error_panel(&mut self, ui: &mut egui::Ui) {
        let errors = self.doc.validation().errors.clone();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("niri would reject this config").strong().color(ui.visuals().error_fg_color));
            ui.weak("— it can't be saved until these are fixed.");
        });
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            for e in errors {
                ui.horizontal_wrapped(|ui| {
                    if let (Some(file), Some(line)) = (e.file, e.line) {
                        let name = self.doc.files[file].path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        if ui.link(format!("{name}:{line}")).clicked() {
                            self.nav = Some(Nav::Raw { file, line });
                        }
                    }
                    ui.label(&e.message);
                    if let Some(l) = &e.label {
                        ui.weak(format!("({l})"));
                    }
                    if let Some(h) = &e.help {
                        ui.weak(format!("hint: {h}"));
                    }
                });
            }
        });
        ui.add_space(4.0);
    }

    fn banners(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = self.load_error.clone() {
            banner(ui, Level::Warn, |ui| {
                ui.label(format!("Couldn't load the config ({err}). Saving will create it."));
                if ui.small_button("Dismiss").clicked() {
                    self.load_error = None;
                }
            });
        }
        if !self.disk_changed.is_empty() {
            banner(ui, Level::Warn, |ui| {
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
            banner(ui, Level::Warn, |ui| {
                ui.label("The config has a syntax error, so visual editing is paused. Fix it in the raw editor.");
                if ui.button("Open raw editor").clicked() {
                    self.page = Page::Raw;
                }
            });
        }
    }

    fn search_window(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let mut open = true;
        egui::Window::new("search")
            .title_bar(false)
            .anchor(Align2::CENTER_TOP, [0.0, 56.0])
            .fixed_size([520.0, 0.0])
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search settings…  (Esc to close)")
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    open = false;
                }
                let results = schema::search(&self.search);
                let pages: Vec<Page> = Page::ALL
                    .into_iter()
                    .filter(|p| !self.search.is_empty() && p.title().to_lowercase().contains(&self.search.to_lowercase()))
                    .collect();
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (i, p) in pages.iter().enumerate() {
                        if ui.selectable_label(false, format!("{}  {}", p.icon(), p.title())).clicked() || (enter && i == 0) {
                            self.page = *p;
                            open = false;
                        }
                    }
                    for (i, o) in results.iter().enumerate() {
                        let r = ui.selectable_label(
                            false,
                            RichText::new(format!("{} › {} › {}", o.page.title(), o.section, o.label)),
                        );
                        let r = if o.help.is_empty() { r } else { r.on_hover_text(o.help) };
                        if r.clicked() || (enter && pages.is_empty() && i == 0) {
                            self.page = o.page;
                            self.highlight = Some((o.path, Instant::now()));
                            open = false;
                        }
                    }
                    if !self.search.is_empty() && results.is_empty() && pages.is_empty() {
                        ui.weak("No settings match. Anything else can be edited in the raw config.");
                    }
                });
            });
        self.search_open = open;
    }

    fn trial_modal(&mut self, ctx: &egui::Context) {
        let Some(t) = &self.trial else { return };
        let left = t.deadline.saturating_duration_since(Instant::now()).as_secs() + 1;
        let mut keep = false;
        let mut revert = false;
        egui::Modal::new(egui::Id::new("trial")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Keep these display settings?");
            ui.label(format!("Reverting in {left} s unless you confirm."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                keep = ui.button("✔ Keep").clicked();
                revert = ui.button("⟲ Revert now").clicked();
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
        self.toasts.0.retain(|t| t.2.elapsed() < Duration::from_secs(6));
        if self.toasts.0.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .show(ctx, |ui| {
                for (msg, level, _) in &self.toasts.0 {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(360.0);
                        ui.label(RichText::new(msg).color(level_color(ui, *level)));
                    });
                }
            });
    }

    fn apply_accent(&self, ctx: &egui::Context) {
        // The app's accent follows your focus ring.
        let ring = &self.cfg.layout.focus_ring;
        let accent = crate::color::from_niri(ring.active_gradient.map_or(ring.active_color, |g| g.from));
        let accent = Color32::from_rgb(accent.r(), accent.g(), accent.b());
        ctx.global_style_mut(|s| {
            s.visuals.selection.bg_fill = accent.gamma_multiply(0.55);
            s.visuals.selection.stroke.color = accent;
            s.visuals.hyperlink_color = accent;
        });
    }
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
        self.apply_accent(&ctx);
        self.shortcuts(&ctx);
        self.poll(&ctx);

        if let Some(nav) = self.nav.take() {
            match nav {
                Nav::Page(p) => self.page = p,
                Nav::Raw { file, line } => {
                    self.page = Page::Raw;
                    self.pages.raw.jump(file, line);
                }
            }
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(2.0);
        });
        let nav_fill = if ui.visuals().dark_mode { Color32::from_gray(22) } else { Color32::from_gray(238) };
        egui::Panel::left("nav")
            .exact_size(212.0)
            .frame(egui::Frame::new().fill(nav_fill).inner_margin(egui::Margin::symmetric(0, 4)))
            .show(ui, |ui| self.nav_panel(ui));
        if !self.doc.validation().ok() {
            egui::Panel::bottom("errors").show(ui, |ui| self.error_panel(ui));
        }

        egui::CentralPanel::default_margins().show(ui, |ui| {
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

        self.search_window(&ctx);
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

pub fn ok_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(120, 210, 140) } else { Color32::from_rgb(30, 130, 60) }
}

pub fn level_color(ui: &egui::Ui, level: Level) -> Color32 {
    match level {
        Level::Info => ui.visuals().text_color(),
        Level::Ok => ok_color(ui),
        Level::Warn => ui.visuals().warn_fg_color,
        Level::Error => ui.visuals().error_fg_color,
    }
}

fn banner(ui: &mut egui::Ui, level: Level, add: impl FnOnce(&mut egui::Ui)) {
    let color = level_color(ui, level);
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(1.0, color))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(add);
        });
    ui.add_space(6.0);
}

/// A sidebar entry: left-aligned icon and label, an accent bar and fill when selected.
fn nav_item(ui: &mut egui::Ui, page: Page, selected: bool, badge: Option<(String, Color32)>) -> egui::Response {
    let height = 32.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let rect = rect.shrink2(egui::vec2(6.0, 1.0));
    let vis = ui.visuals();
    let accent = vis.selection.stroke.color;
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 6.0, accent.gamma_multiply(0.16));
        let bar = egui::Rect::from_min_size(rect.left_top() + egui::vec2(0.0, 7.0), egui::vec2(3.0, rect.height() - 14.0));
        painter.rect_filled(bar, 2.0, accent);
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, vis.widgets.hovered.weak_bg_fill.gamma_multiply(0.6));
    }
    let fg = if selected { accent } else if resp.hovered() { vis.strong_text_color() } else { vis.text_color() };
    painter.text(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        Align2::CENTER_CENTER,
        page.icon(),
        egui::FontId::proportional(15.0),
        fg,
    );
    painter.text(
        egui::pos2(rect.left() + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        page.title(),
        egui::FontId::proportional(14.5),
        fg,
    );
    if let Some((text, color)) = badge {
        let c = egui::pos2(rect.right() - 16.0, rect.center().y);
        painter.circle_filled(c, 9.0, color.gamma_multiply(0.85));
        painter.text(c, Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), Color32::WHITE);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}
