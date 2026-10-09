//! A catalogue of niri's scalar options, used to generate settings forms and power search.
//!
//! Each entry is checked against niri's real parser in `tests`, so a typo or an option niri
//! dropped fails the build's tests rather than producing a broken config.

/// How an option is written in KDL.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Presence means on: `prefer-no-csd`.
    Flag,
    /// An explicit boolean argument: `open-floating true`.
    Bool,
    Int { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Str,
    /// One string out of a fixed set.
    Enum(&'static [&'static str]),
    /// A CSS-style color string.
    Color,
    /// A file path string; `~` is expanded by niri.
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Input,
    Layout,
    Appearance,
    Animations,
    Displays,
    Keybinds,
    Rules,
    Startup,
    Gestures,
    Misc,
    Raw,
    History,
}

impl Page {
    pub const ALL: [Page; 12] = [
        Page::Displays,
        Page::Layout,
        Page::Appearance,
        Page::Animations,
        Page::Keybinds,
        Page::Rules,
        Page::Input,
        Page::Gestures,
        Page::Startup,
        Page::Misc,
        Page::Raw,
        Page::History,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Input => "Input",
            Page::Layout => "Layout",
            Page::Appearance => "Appearance",
            Page::Animations => "Animations",
            Page::Displays => "Displays",
            Page::Keybinds => "Key bindings",
            Page::Rules => "Window rules",
            Page::Startup => "Startup",
            Page::Gestures => "Gestures",
            Page::Misc => "Miscellaneous",
            Page::Raw => "Raw config",
            Page::History => "History",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Page::Input => "⌨",
            Page::Layout => "▥",
            Page::Appearance => "🎨",
            Page::Animations => "〰",
            Page::Displays => "🖵",
            Page::Keybinds => "⌘",
            Page::Rules => "☰",
            Page::Startup => "▶",
            Page::Gestures => "☝",
            Page::Misc => "⚙",
            Page::Raw => "{}",
            Page::History => "⟲",
        }
    }

    /// The niri wiki page documenting it.
    pub fn wiki(self) -> Option<&'static str> {
        let page = match self {
            Page::Input => "Configuration:-Input",
            Page::Layout | Page::Appearance => "Configuration:-Layout",
            Page::Animations => "Configuration:-Animations",
            Page::Displays => "Configuration:-Outputs",
            Page::Keybinds => "Configuration:-Key-Bindings",
            Page::Rules => "Configuration:-Window-Rules",
            Page::Gestures => "Configuration:-Gestures",
            Page::Startup | Page::Misc => "Configuration:-Miscellaneous",
            Page::Raw | Page::History => return None,
        };
        Some(page)
    }
}

pub const WIKI: &str = "https://github.com/niri-wm/niri/wiki/";

#[derive(Debug, Clone, Copy)]
pub struct Opt {
    /// `/`-separated node path, e.g. `layout/focus-ring/width`.
    pub path: &'static str,
    pub kind: Kind,
    pub label: &'static str,
    pub help: &'static str,
    pub page: Page,
    /// Group heading within the page.
    pub section: &'static str,
}

impl Opt {
    /// A sensible value to start from when the user turns the option on.
    pub fn sample(&self) -> String {
        match self.kind {
            Kind::Flag => String::new(),
            Kind::Bool => "true".into(),
            Kind::Int { min, max } => (if min <= 0 && max >= 0 { 0.max(min) } else { min }).to_string(),
            Kind::Float { min, max } => {
                let v = if (min..=max).contains(&1.0) { 1.0 } else { min };
                format!("{v:.1}")
            }
            Kind::Str => "\"\"".into(),
            Kind::Enum(v) => format!("\"{}\"", v[0]),
            Kind::Color => "\"#7fc8ff\"".into(),
            Kind::Path => "\"\"".into(),
        }
    }
}

const ACCEL: &[&str] = &["adaptive", "flat"];
const SCROLL_METHOD: &[&str] = &["no-scroll", "two-finger", "edge", "on-button-down"];

macro_rules! opts {
    ($( $page:ident $section:literal { $( $path:literal $kind:expr, $label:literal, $help:literal; )* } )*) => {
        pub static OPTIONS: &[Opt] = &[
            $( $( Opt { path: $path, kind: $kind, label: $label, help: $help, page: Page::$page, section: $section }, )* )*
        ];
    };
}

use Kind::*;

opts! {
    Input "Keyboard" {
        "input/keyboard/xkb/layout" Str, "Layouts", "Comma-separated XKB layouts, e.g. \"us,tr\".";
        "input/keyboard/xkb/variant" Str, "Variants", "XKB variants matching the layouts.";
        "input/keyboard/xkb/options" Str, "Options", "XKB options, e.g. \"grp:win_space_toggle,compose:ralt\".";
        "input/keyboard/xkb/model" Str, "Model", "XKB keyboard model.";
        "input/keyboard/xkb/rules" Str, "Rules", "XKB rules file.";
        "input/keyboard/xkb/file" Path, "Keymap file", "Load a compiled .xkb keymap instead of the settings above.";
        "input/keyboard/repeat-delay" Int { min: 100, max: 2000 }, "Repeat delay (ms)", "How long a key is held before it repeats.";
        "input/keyboard/repeat-rate" Int { min: 1, max: 100 }, "Repeat rate", "Characters per second while a key repeats.";
        "input/keyboard/track-layout" Enum(&["global", "window"]), "Track layout", "Keep one layout for all windows, or remember it per window.";
        "input/keyboard/numlock" Flag, "Num Lock on at startup", "";
    }
    Input "Touchpad" {
        "input/touchpad/off" Flag, "Disable touchpad", "";
        "input/touchpad/tap" Flag, "Tap to click", "";
        "input/touchpad/dwt" Flag, "Disable while typing", "";
        "input/touchpad/dwtp" Flag, "Disable while trackpointing", "";
        "input/touchpad/drag" Bool, "Tap and drag", "";
        "input/touchpad/drag-lock" Flag, "Drag lock", "Lifting the finger briefly doesn't end a drag.";
        "input/touchpad/natural-scroll" Flag, "Natural scrolling", "";
        "input/touchpad/accel-speed" Float { min: -1.0, max: 1.0 }, "Pointer speed", "From -1 (slowest) to 1 (fastest).";
        "input/touchpad/accel-profile" Enum(ACCEL), "Acceleration profile", "";
        "input/touchpad/scroll-factor" Float { min: 0.0, max: 10.0 }, "Scroll speed", "Multiplier for scroll distance.";
        "input/touchpad/scroll-method" Enum(SCROLL_METHOD), "Scroll method", "";
        "input/touchpad/tap-button-map" Enum(&["left-right-middle", "left-middle-right"]), "Tap button map", "Which buttons one-, two- and three-finger taps press.";
        "input/touchpad/click-method" Enum(&["button-areas", "clickfinger"]), "Click method", "";
        "input/touchpad/left-handed" Flag, "Left-handed", "";
        "input/touchpad/disabled-on-external-mouse" Flag, "Disable when a mouse is connected", "";
        "input/touchpad/middle-emulation" Flag, "Middle-click emulation", "";
        "input/touchpad/pinch-sensitivity" Float { min: 0.1, max: 5.0 }, "Pinch sensitivity", "";
    }
    Input "Mouse" {
        "input/mouse/off" Flag, "Disable mouse", "";
        "input/mouse/natural-scroll" Flag, "Natural scrolling", "";
        "input/mouse/accel-speed" Float { min: -1.0, max: 1.0 }, "Pointer speed", "";
        "input/mouse/accel-profile" Enum(ACCEL), "Acceleration profile", "";
        "input/mouse/scroll-factor" Float { min: 0.0, max: 10.0 }, "Scroll speed", "";
        "input/mouse/scroll-method" Enum(SCROLL_METHOD), "Scroll method", "";
        "input/mouse/left-handed" Flag, "Left-handed", "";
        "input/mouse/middle-emulation" Flag, "Middle-click emulation", "";
    }
    Input "Trackpoint" {
        "input/trackpoint/off" Flag, "Disable trackpoint", "";
        "input/trackpoint/natural-scroll" Flag, "Natural scrolling", "";
        "input/trackpoint/accel-speed" Float { min: -1.0, max: 1.0 }, "Pointer speed", "";
        "input/trackpoint/accel-profile" Enum(ACCEL), "Acceleration profile", "";
        "input/trackpoint/scroll-method" Enum(SCROLL_METHOD), "Scroll method", "";
        "input/trackpoint/middle-emulation" Flag, "Middle-click emulation", "";
    }
    Input "Tablet and touch" {
        "input/tablet/off" Flag, "Disable tablet", "";
        "input/tablet/map-to-output" Str, "Map tablet to output", "Output name, e.g. \"eDP-1\".";
        "input/tablet/left-handed" Flag, "Left-handed tablet", "";
        "input/touch/off" Flag, "Disable touchscreen", "";
        "input/touch/map-to-output" Str, "Map touchscreen to output", "";
    }
    Input "Focus and behaviour" {
        "input/warp-mouse-to-focus" Flag, "Warp mouse to focus", "Move the pointer to newly focused windows.";
        "input/focus-follows-mouse" Flag, "Focus follows mouse", "";
        "input/workspace-auto-back-and-forth" Flag, "Workspace back-and-forth", "Switching to the current workspace goes back to the previous one.";
        "input/disable-power-key-handling" Flag, "Ignore the power key", "Let logind or another program handle it.";
        "input/mod-key" Enum(&["Super", "Alt", "Ctrl", "Shift", "Mod3", "Mod5", "ISO_Level3_Shift", "ISO_Level5_Shift"]), "Mod key", "What Mod means in binds.";
        "input/mod-key-nested" Enum(&["Alt", "Super", "Ctrl", "Shift", "Mod3", "Mod5", "ISO_Level3_Shift", "ISO_Level5_Shift"]), "Mod key when nested", "Mod when niri runs as a window.";
    }

    Layout "Spacing" {
        "layout/gaps" Float { min: 0.0, max: 128.0 }, "Gaps", "Space between windows and around the edges, in logical pixels.";
        "layout/struts/left" Float { min: 0.0, max: 512.0 }, "Strut left", "Extra outer space on the left.";
        "layout/struts/right" Float { min: 0.0, max: 512.0 }, "Strut right", "";
        "layout/struts/top" Float { min: 0.0, max: 512.0 }, "Strut top", "";
        "layout/struts/bottom" Float { min: 0.0, max: 512.0 }, "Strut bottom", "";
    }
    Layout "Columns" {
        "layout/center-focused-column" Enum(&["never", "always", "on-overflow"]), "Center focused column", "";
        "layout/always-center-single-column" Flag, "Center a lone column", "";
        "layout/empty-workspace-above-first" Flag, "Empty workspace above the first", "";
        "layout/default-column-display" Enum(&["normal", "tabbed"]), "New column display", "";
        "layout/background-color" Color, "Background color", "Shown behind windows where no wallpaper covers.";
    }

    Appearance "Tab indicator" {
        "layout/tab-indicator/off" Flag, "Hide tab indicator", "";
        "layout/tab-indicator/hide-when-single-tab" Flag, "Hide with a single tab", "";
        "layout/tab-indicator/place-within-column" Flag, "Place within column", "";
        "layout/tab-indicator/gap" Float { min: -64.0, max: 64.0 }, "Gap", "";
        "layout/tab-indicator/width" Float { min: 0.0, max: 64.0 }, "Width", "";
        "layout/tab-indicator/position" Enum(&["left", "right", "top", "bottom"]), "Position", "";
        "layout/tab-indicator/gaps-between-tabs" Float { min: 0.0, max: 64.0 }, "Gaps between tabs", "";
        "layout/tab-indicator/corner-radius" Float { min: 0.0, max: 64.0 }, "Corner radius", "";
        "layout/tab-indicator/active-color" Color, "Active color", "";
        "layout/tab-indicator/inactive-color" Color, "Inactive color", "";
        "layout/tab-indicator/urgent-color" Color, "Urgent color", "";
    }
    Appearance "Insert hint" {
        "layout/insert-hint/off" Flag, "Hide insert hint", "";
        "layout/insert-hint/color" Color, "Color", "Where a dragged window will land.";
    }
    Appearance "Cursor" {
        "cursor/xcursor-theme" Str, "Cursor theme", "";
        "cursor/xcursor-size" Int { min: 8, max: 255 }, "Cursor size", "";
        "cursor/hide-when-typing" Flag, "Hide while typing", "";
        "cursor/hide-after-inactive-ms" Int { min: 0, max: 60000 }, "Hide after inactivity (ms)", "";
    }
    Appearance "Overview" {
        "overview/zoom" Float { min: 0.0001, max: 0.75 }, "Overview zoom", "";
        "overview/backdrop-color" Color, "Overview backdrop", "";
    }
    Appearance "Blur" {
        "blur/off" Flag, "Disable blur", "";
        "blur/passes" Int { min: 0, max: 8 }, "Passes", "";
        "blur/offset" Float { min: 0.0, max: 16.0 }, "Offset", "";
        "blur/noise" Float { min: 0.0, max: 1.0 }, "Noise", "";
        "blur/saturation" Float { min: 0.0, max: 4.0 }, "Saturation", "";
    }

    Animations "Global" {
        "animations/off" Flag, "Disable all animations", "";
        "animations/slowdown" Float { min: 0.1, max: 10.0 }, "Slowdown", "Above 1 slows every animation down; below 1 speeds them up.";
    }

    Gestures "Drag and drop: scroll the view" {
        "gestures/dnd-edge-view-scroll/trigger-width" Float { min: 0.0, max: 400.0 }, "Trigger width", "";
        "gestures/dnd-edge-view-scroll/delay-ms" Int { min: 0, max: 5000 }, "Delay (ms)", "";
        "gestures/dnd-edge-view-scroll/max-speed" Float { min: 0.0, max: 10000.0 }, "Max speed", "";
    }
    Gestures "Drag and drop: switch workspace" {
        "gestures/dnd-edge-workspace-switch/trigger-height" Float { min: 0.0, max: 400.0 }, "Trigger height", "";
        "gestures/dnd-edge-workspace-switch/delay-ms" Int { min: 0, max: 5000 }, "Delay (ms)", "";
        "gestures/dnd-edge-workspace-switch/max-speed" Float { min: 0.0, max: 10000.0 }, "Max speed", "";
    }
    Gestures "Hot corners" {
        "gestures/hot-corners/off" Flag, "Disable hot corners", "";
        "gestures/hot-corners/top-left" Flag, "Top left", "";
        "gestures/hot-corners/top-right" Flag, "Top right", "";
        "gestures/hot-corners/bottom-left" Flag, "Bottom left", "";
        "gestures/hot-corners/bottom-right" Flag, "Bottom right", "";
    }
    Gestures "Recent windows switcher" {
        "recent-windows/off" Flag, "Disable the switcher", "";
        "recent-windows/debounce-ms" Int { min: 0, max: 5000 }, "Debounce (ms)", "";
        "recent-windows/open-delay-ms" Int { min: 0, max: 5000 }, "Open delay (ms)", "";
        "recent-windows/highlight/active-color" Color, "Highlight color", "";
        "recent-windows/highlight/urgent-color" Color, "Urgent highlight color", "";
        "recent-windows/highlight/padding" Float { min: 0.0, max: 200.0 }, "Highlight padding", "";
        "recent-windows/highlight/corner-radius" Float { min: 0.0, max: 100.0 }, "Highlight corner radius", "";
        "recent-windows/previews/max-height" Float { min: 1.0, max: 2000.0 }, "Preview max height", "";
        "recent-windows/previews/max-scale" Float { min: 0.01, max: 1.0 }, "Preview max scale", "";
    }

    Startup "Session" {
        "prefer-no-csd" Flag, "Prefer server-side decorations", "Ask apps not to draw their own title bars.";
        "screenshot-path" Path, "Screenshot path", "strftime-style template; empty to not save screenshots to disk.";
        "xwayland-satellite/off" Flag, "Disable xwayland-satellite", "";
        "xwayland-satellite/path" Path, "xwayland-satellite binary", "";
    }

    Misc "Notifications and overlays" {
        "hotkey-overlay/skip-at-startup" Flag, "Don't show hotkey overlay at startup", "";
        "hotkey-overlay/hide-not-bound" Flag, "Hide unbound actions in the overlay", "";
        "config-notification/disable-failed" Flag, "Don't notify about config errors", "";
        "clipboard/disable-primary" Flag, "Disable primary selection", "Middle-click paste.";
    }
}

/// Options whose label, help or path contain every word of `query`.
pub fn search(query: &str) -> Vec<&'static Opt> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Vec::new();
    }
    OPTIONS
        .iter()
        .filter(|o| {
            let hay = format!("{} {} {} {} {}", o.label, o.help, o.path, o.section, o.page.title()).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

/// The options in a page, grouped by section in declaration order.
pub fn sections(page: Page) -> Vec<(&'static str, Vec<&'static Opt>)> {
    let mut out: Vec<(&'static str, Vec<&'static Opt>)> = Vec::new();
    for o in OPTIONS.iter().filter(|o| o.page == page) {
        match out.iter_mut().find(|(s, _)| *s == o.section) {
            Some((_, v)) => v.push(o),
            None => out.push((o.section, vec![o])),
        }
    }
    out
}

/// Bind actions niri accepts, for the bind editor's picker: (name, argument hint).
pub static ACTIONS: &[(&str, &str)] = &[
    ("spawn", "program and arguments"),
    ("spawn-sh", "shell command"),
    ("close-window", ""),
    ("quit", ""),
    ("power-off-monitors", ""),
    ("power-on-monitors", ""),
    ("toggle-keyboard-shortcuts-inhibit", ""),
    ("show-hotkey-overlay", ""),
    ("toggle-overview", ""),
    ("open-overview", ""),
    ("close-overview", ""),
    ("screenshot", ""),
    ("screenshot-screen", ""),
    ("screenshot-window", ""),
    ("focus-column-left", ""),
    ("focus-column-right", ""),
    ("focus-column-first", ""),
    ("focus-column-last", ""),
    ("focus-column-or-monitor-left", ""),
    ("focus-column-or-monitor-right", ""),
    ("focus-window-up", ""),
    ("focus-window-down", ""),
    ("focus-window-or-workspace-up", ""),
    ("focus-window-or-workspace-down", ""),
    ("focus-window-previous", ""),
    ("move-column-left", ""),
    ("move-column-right", ""),
    ("move-column-to-first", ""),
    ("move-column-to-last", ""),
    ("move-column-left-or-to-monitor-left", ""),
    ("move-column-right-or-to-monitor-right", ""),
    ("move-window-up", ""),
    ("move-window-down", ""),
    ("move-window-up-or-to-workspace-up", ""),
    ("move-window-down-or-to-workspace-down", ""),
    ("consume-or-expel-window-left", ""),
    ("consume-or-expel-window-right", ""),
    ("consume-window-into-column", ""),
    ("expel-window-from-column", ""),
    ("swap-window-left", ""),
    ("swap-window-right", ""),
    ("toggle-column-tabbed-display", ""),
    ("center-column", ""),
    ("center-visible-columns", ""),
    ("focus-workspace-down", ""),
    ("focus-workspace-up", ""),
    ("focus-workspace", "index or name"),
    ("focus-workspace-previous", ""),
    ("move-window-to-workspace-down", ""),
    ("move-window-to-workspace-up", ""),
    ("move-window-to-workspace", "index or name"),
    ("move-column-to-workspace-down", ""),
    ("move-column-to-workspace-up", ""),
    ("move-column-to-workspace", "index or name"),
    ("move-workspace-down", ""),
    ("move-workspace-up", ""),
    ("focus-monitor-left", ""),
    ("focus-monitor-right", ""),
    ("focus-monitor-up", ""),
    ("focus-monitor-down", ""),
    ("focus-monitor-next", ""),
    ("focus-monitor-previous", ""),
    ("move-column-to-monitor-left", ""),
    ("move-column-to-monitor-right", ""),
    ("move-column-to-monitor-up", ""),
    ("move-column-to-monitor-down", ""),
    ("move-workspace-to-monitor-left", ""),
    ("move-workspace-to-monitor-right", ""),
    ("set-window-height", "e.g. \"-10%\" or \"+10\""),
    ("reset-window-height", ""),
    ("switch-preset-column-width", ""),
    ("switch-preset-column-width-back", ""),
    ("switch-preset-window-height", ""),
    ("maximize-column", ""),
    ("maximize-window-to-edges", ""),
    ("fullscreen-window", ""),
    ("toggle-windowed-fullscreen", ""),
    ("set-column-width", "e.g. \"-10%\" or \"+10\""),
    ("expand-column-to-available-width", ""),
    ("toggle-window-floating", ""),
    ("switch-focus-between-floating-and-tiling", ""),
    ("toggle-window-rule-opacity", ""),
    ("switch-layout", "\"next\", \"prev\" or index"),
    ("set-dynamic-cast-window", ""),
    ("set-dynamic-cast-monitor", ""),
    ("clear-dynamic-cast-target", ""),
    ("do-screen-transition", ""),
    ("next-window", ""),
    ("previous-window", ""),
];

#[cfg(test)]
mod tests;
