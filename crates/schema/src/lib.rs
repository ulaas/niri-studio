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
            Page::Layout => "📐",
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
        "input/touchpad/scroll-button" Int { min: 0, max: 1000 }, "Scroll button", "Button code for on-button-down scrolling, e.g. 273 (right button).";
        "input/touchpad/scroll-button-lock" Flag, "Scroll button lock", "The scroll button toggles scrolling instead of being held.";
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
        "input/mouse/scroll-button" Int { min: 0, max: 1000 }, "Scroll button", "Button code for on-button-down scrolling, e.g. 274 (middle button).";
        "input/mouse/scroll-button-lock" Flag, "Scroll button lock", "The scroll button toggles scrolling instead of being held.";
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
        "input/trackpoint/left-handed" Flag, "Left-handed", "";
        "input/trackpoint/scroll-button" Int { min: 0, max: 1000 }, "Scroll button", "Button code for on-button-down scrolling.";
        "input/trackpoint/scroll-button-lock" Flag, "Scroll button lock", "The scroll button toggles scrolling instead of being held.";
    }
    Input "Trackball" {
        "input/trackball/off" Flag, "Disable trackball", "";
        "input/trackball/natural-scroll" Flag, "Natural scrolling", "";
        "input/trackball/accel-speed" Float { min: -1.0, max: 1.0 }, "Pointer speed", "";
        "input/trackball/accel-profile" Enum(ACCEL), "Acceleration profile", "";
        "input/trackball/scroll-method" Enum(SCROLL_METHOD), "Scroll method", "";
        "input/trackball/scroll-button" Int { min: 0, max: 1000 }, "Scroll button", "Button code for on-button-down scrolling.";
        "input/trackball/scroll-button-lock" Flag, "Scroll button lock", "The scroll button toggles scrolling instead of being held.";
        "input/trackball/left-handed" Flag, "Left-handed", "";
        "input/trackball/middle-emulation" Flag, "Middle-click emulation", "";
    }
    Input "Tablet and touch" {
        "input/tablet/off" Flag, "Disable tablet", "";
        "input/tablet/map-to-output" Str, "Map tablet to output", "Output name, e.g. \"eDP-1\".";
        "input/tablet/left-handed" Flag, "Left-handed tablet", "";
        "input/tablet/map-to-focused-output" Flag, "Map tablet to focused output", "Follow whichever output has focus.";
        "input/tablet/map-to-focused-window" Flag, "Map tablet to focused window", "Map the tablet's area onto the focused window.";
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
        "overview/workspace-shadow/off" Flag, "Hide workspace shadow", "The shadow under each workspace in the overview.";
        "overview/workspace-shadow/softness" Float { min: 0.0, max: 1024.0 }, "Workspace shadow softness", "Blur radius, in logical pixels.";
        "overview/workspace-shadow/spread" Float { min: -1024.0, max: 1024.0 }, "Workspace shadow spread", "";
        "overview/workspace-shadow/color" Color, "Workspace shadow color", "";
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
        "gestures/hot-corners/top-left" Flag, "Top left", "niri uses the top-left corner when none is chosen here.";
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
    Misc "Debug" {
        "debug/preview-render" Enum(&["screencast", "screen-capture"]), "Preview render", "Render monitors as a screencast or screen capture would see them.";
        "debug/render-drm-device" Path, "Render DRM device", "e.g. /dev/dri/renderD129.";
        "debug/enable-overlay-planes" Flag, "Enable overlay planes", "";
        "debug/disable-cursor-plane" Flag, "Disable cursor plane", "";
        "debug/disable-direct-scanout" Flag, "Disable direct scanout", "";
        "debug/restrict-primary-scanout-to-matching-format" Flag, "Restrict primary scanout format", "";
        "debug/wait-for-frame-completion-before-queueing" Flag, "Wait for frame completion", "";
        "debug/disable-10bit-output" Flag, "Disable 10-bit output", "";
        "debug/force-disable-connectors-on-resume" Flag, "Force-disable connectors on resume", "";
        "debug/keep-laptop-panel-on-when-lid-is-closed" Flag, "Keep panel on with lid closed", "";
        "debug/disable-monitor-names" Flag, "Disable monitor names", "";
        "debug/emulate-zero-presentation-time" Flag, "Emulate zero presentation time", "";
        "debug/skip-cursor-only-updates-during-vrr" Flag, "Skip cursor-only updates during VRR", "";
        "debug/disable-resize-throttling" Flag, "Disable resize throttling", "";
        "debug/disable-transactions" Flag, "Disable transactions", "";
        "debug/strict-new-window-focus-policy" Flag, "Strict new-window focus policy", "Only focus new windows that ask with a valid activation token.";
        "debug/honor-xdg-activation-with-invalid-serial" Flag, "Honor activation with invalid serial", "";
        "debug/deactivate-unfocused-windows" Flag, "Deactivate unfocused windows", "";
        "debug/force-pipewire-invalid-modifier" Flag, "Force PipeWire invalid modifier", "";
        "debug/disable-pipewire-dmabuf" Flag, "Disable PipeWire dmabuf", "";
        "debug/dbus-interfaces-in-non-session-instances" Flag, "D-Bus in non-session instances", "";
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
    ("minimize-window", ""),
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

/// A property inside a rule node: (path, kind, label, help). The path is `/`-separated for
/// options nested in a block, e.g. `border/off`.
pub type RuleProp = (&'static str, Kind, &'static str, &'static str);

pub static WINDOW_RULE_PROPS: &[RuleProp] = &[
    ("open-floating", Kind::Bool, "Open floating", ""),
    ("open-maximized", Kind::Bool, "Open maximized", ""),
    ("pinch-sensitivity", Kind::Float { min: 0.0, max: 10.0 }, "Pinch sensitivity", "Multiplier for touchpad pinch gestures in this window."),
    ("open-maximized-to-edges", Kind::Bool, "Open maximized to edges", "Fill the screen, ignoring gaps and struts."),
    ("open-fullscreen", Kind::Bool, "Open fullscreen", ""),
    ("open-focused", Kind::Bool, "Focus when opened", "Off keeps new windows from stealing focus."),
    ("open-on-workspace", Kind::Str, "Open on workspace", "Name of a named workspace."),
    ("open-on-output", Kind::Str, "Open on output", "Connector or monitor name."),
    ("default-column-display", Kind::Enum(&["normal", "tabbed"]), "Column display", ""),
    ("opacity", Kind::Float { min: 0.0, max: 1.0 }, "Opacity", ""),
    ("block-out-from", Kind::Enum(&["screencast", "screen-capture"]), "Hide from", "Blacks the window out in screencasts or all screen captures."),
    ("geometry-corner-radius", Kind::Float { min: 0.0, max: 64.0 }, "Corner radius", "For borders, focus ring and shadow."),
    ("clip-to-geometry", Kind::Bool, "Clip to corners", "Cut the window to its rounded geometry."),
    ("draw-border-with-background", Kind::Bool, "Border with background", "Fill behind the window with the border color."),
    ("border/off", Kind::Flag, "Hide border", "No border on this window, focused or not."),
    ("border/on", Kind::Flag, "Show border", "Draw a border even when the layout turns it off."),
    ("border/width", Kind::Float { min: 0.0, max: 64.0 }, "Border width", ""),
    ("border/active-color", Kind::Color, "Border active color", ""),
    ("border/inactive-color", Kind::Color, "Border inactive color", ""),
    ("border/urgent-color", Kind::Color, "Border urgent color", ""),
    ("focus-ring/off", Kind::Flag, "Hide focus ring", "No focus ring on this window."),
    ("focus-ring/on", Kind::Flag, "Show focus ring", "Draw the focus ring even when the layout turns it off."),
    ("focus-ring/width", Kind::Float { min: 0.0, max: 64.0 }, "Focus ring width", ""),
    ("focus-ring/active-color", Kind::Color, "Focus ring active color", ""),
    ("focus-ring/inactive-color", Kind::Color, "Focus ring inactive color", ""),
    ("focus-ring/urgent-color", Kind::Color, "Focus ring urgent color", ""),
    ("shadow/off", Kind::Flag, "Hide shadow", ""),
    ("shadow/on", Kind::Flag, "Show shadow", "Draw a shadow even when the layout turns it off."),
    ("shadow/color", Kind::Color, "Shadow color", ""),
    ("variable-refresh-rate", Kind::Bool, "Variable refresh rate", "Enable VRR while this window is shown (needs on-demand VRR)."),
    ("tiled-state", Kind::Bool, "Tell the app it's tiled", ""),
    ("baba-is-float", Kind::Bool, "Baba is float", "Make the window bob gently."),
    ("scroll-factor", Kind::Float { min: 0.0, max: 10.0 }, "Scroll factor", ""),
    ("on-xdg-activate", Kind::Enum(&["focus", "set-urgent", "ignore"]), "When it asks for focus", ""),
    ("min-width", Kind::Int { min: 0, max: 65535 }, "Min width", ""),
    ("max-width", Kind::Int { min: 0, max: 65535 }, "Max width", ""),
    ("min-height", Kind::Int { min: 0, max: 65535 }, "Min height", ""),
    ("max-height", Kind::Int { min: 0, max: 65535 }, "Max height", ""),
];

pub static LAYER_RULE_PROPS: &[RuleProp] = &[
    ("opacity", Kind::Float { min: 0.0, max: 1.0 }, "Opacity", ""),
    ("block-out-from", Kind::Enum(&["screencast", "screen-capture"]), "Hide from", ""),
    ("geometry-corner-radius", Kind::Float { min: 0.0, max: 64.0 }, "Corner radius", ""),
    ("place-within-backdrop", Kind::Bool, "Place within overview backdrop", ""),
    ("shadow/off", Kind::Flag, "Hide shadow", ""),
    ("shadow/on", Kind::Flag, "Show shadow", "Draw a shadow even when the layout turns it off."),
    ("shadow/color", Kind::Color, "Shadow color", ""),
    ("baba-is-float", Kind::Bool, "Baba is float", ""),
];

/// Boolean matchers: (property, label).
pub static WINDOW_MATCH_FLAGS: &[(&str, &str)] = &[
    ("is-floating", "floating"),
    ("is-focused", "focused"),
    ("is-active", "active"),
    ("is-active-in-column", "active in column"),
    ("is-urgent", "urgent"),
    ("is-window-cast-target", "being cast"),
    ("at-startup", "at startup"),
];

/// One-click rules applied to a picked window: (button, label prefix, body lines).
pub static QUICK_RULES: &[(&str, &str, &[&str])] = &[
    ("Always float", "Float", &["open-floating true"]),
    ("Open maximized", "Maximize", &["open-maximized true"]),
    ("Open fullscreen", "Fullscreen", &["open-fullscreen true"]),
    ("Don't steal focus", "No focus steal", &["open-focused false"]),
    ("Slightly transparent", "Transparent", &["opacity 0.9"]),
    ("Hide from screencasts", "Private", &["block-out-from \"screencast\""]),
    ("Rounded corners", "Rounded", &["geometry-corner-radius 12", "clip-to-geometry true"]),
    ("Open tabbed", "Tabbed", &["default-column-display \"tabbed\""]),
    ("Empty rule", "Rule", &[]),
];

/// niri's default for an option, as KDL text. Each one is proven in tests: a config with
/// `option <default>` parses to exactly the same `Config` as one without it.
pub static DEFAULTS: &[(&str, &str)] = &[
    ("input/keyboard/repeat-delay", "600"),
    ("input/keyboard/repeat-rate", "25"),
    ("input/keyboard/track-layout", "\"global\""),
    ("layout/gaps", "16"),
    ("layout/struts/left", "0"),
    ("layout/struts/right", "0"),
    ("layout/struts/top", "0"),
    ("layout/struts/bottom", "0"),
    ("layout/center-focused-column", "\"never\""),
    ("layout/default-column-display", "\"normal\""),
    ("layout/background-color", "\"rgb(25%, 25%, 25%)\""),
    ("layout/tab-indicator/gap", "5"),
    ("layout/tab-indicator/width", "4"),
    ("layout/tab-indicator/position", "\"left\""),
    ("layout/tab-indicator/gaps-between-tabs", "0"),
    ("layout/tab-indicator/corner-radius", "0"),
    ("layout/insert-hint/color", "\"#7fc8ff80\""),
    ("layout/focus-ring/width", "4"),
    ("layout/focus-ring/active-color", "\"#7fc8ff\""),
    ("layout/focus-ring/inactive-color", "\"#505050\""),
    ("layout/focus-ring/urgent-color", "\"#9b0000\""),
    ("layout/border/width", "4"),
    ("layout/border/active-color", "\"#ffc87f\""),
    ("layout/border/inactive-color", "\"#505050\""),
    ("layout/border/urgent-color", "\"#9b0000\""),
    ("layout/shadow/softness", "30"),
    ("layout/shadow/spread", "5"),
    ("layout/shadow/color", "\"#00000077\""),
    ("layout/shadow/draw-behind-window", "false"),
    ("cursor/xcursor-theme", "\"default\""),
    ("cursor/xcursor-size", "24"),
    ("overview/zoom", "0.5"),
    ("overview/backdrop-color", "\"rgb(15%, 15%, 15%)\""),
    ("blur/passes", "3"),
    ("blur/offset", "3"),
    ("blur/noise", "0.02"),
    ("blur/saturation", "1.5"),
    ("animations/slowdown", "1"),
    ("gestures/dnd-edge-view-scroll/trigger-width", "30"),
    ("gestures/dnd-edge-view-scroll/delay-ms", "100"),
    ("gestures/dnd-edge-view-scroll/max-speed", "1500"),
    ("gestures/dnd-edge-workspace-switch/trigger-height", "50"),
    ("gestures/dnd-edge-workspace-switch/delay-ms", "100"),
    ("gestures/dnd-edge-workspace-switch/max-speed", "1500"),
    ("recent-windows/debounce-ms", "750"),
    ("recent-windows/open-delay-ms", "150"),
    ("recent-windows/highlight/active-color", "\"rgb(60%, 60%, 60%)\""),
    ("recent-windows/highlight/urgent-color", "\"rgb(100%, 60%, 60%)\""),
    ("recent-windows/highlight/padding", "30"),
    ("recent-windows/highlight/corner-radius", "0"),
    ("recent-windows/previews/max-height", "480"),
    ("recent-windows/previews/max-scale", "0.5"),
    ("screenshot-path", "\"~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png\""),
    ("xwayland-satellite/path", "\"xwayland-satellite\""),
];

/// Defaults that aren't niri settings but come from elsewhere when unset (libinput, the
/// session). Shown to the user, but not provable by comparing configs.
pub static NOMINAL_DEFAULTS: &[(&str, &str, &str)] = &[
    ("input/touchpad/accel-speed", "0", "libinput default"),
    ("input/mouse/accel-speed", "0", "libinput default"),
    ("input/trackpoint/accel-speed", "0", "libinput default"),
    ("input/touchpad/scroll-factor", "1", "no scaling"),
    ("input/mouse/scroll-factor", "1", "no scaling"),
    ("input/touchpad/pinch-sensitivity", "1", "no scaling"),
    ("input/touchpad/accel-profile", "\"adaptive\"", "libinput default"),
    ("input/mouse/accel-profile", "\"adaptive\"", "libinput default"),
    ("input/trackpoint/accel-profile", "\"adaptive\"", "libinput default"),
    ("input/mod-key", "\"Super\"", "Super, or Alt when nested"),
    ("input/mod-key-nested", "\"Alt\"", ""),
    ("cursor/hide-after-inactive-ms", "0", "never hides"),
    ("input/touchpad/drag", "true", "libinput default"),
];

/// The default to display for an option: (KDL text, note). The note is empty when the value
/// is niri's own proven default.
pub fn default_of(path: &str) -> Option<(&'static str, &'static str)> {
    DEFAULTS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, v)| (*v, ""))
        .or_else(|| NOMINAL_DEFAULTS.iter().find(|(p, ..)| *p == path).map(|(_, v, n)| (*v, *n)))
}
