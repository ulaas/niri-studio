# niri studio

[![CI](https://github.com/ulaas/niri-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/ulaas/niri-studio/actions/workflows/ci.yml)

A Wayland-native visual editor for [niri](https://github.com/niri-wm/niri)'s `config.kdl`,
written in Rust with egui. **[See the tour →](https://ulaas.github.io/niri-studio/)**

![niri studio's Layout page](site/assets/shots/hero.webp)

- **Keeps your file intact.** Edits are spliced into the text; comments, ordering and
  formatting you wrote are never rewritten. Everything written is KDL v1, which niri reads.
- **niri is the judge.** Every change is re-parsed with niri's own `niri-config` crate (pinned
  to a niri release commit), so errors are exactly what niri would report, with line links.
- **Safe apply.** Saving is atomic and backs up the previous file. If niri reports that the
  reload failed, the backup is restored automatically. Display changes revert after 15 seconds
  unless you confirm them.

## Pages

| Page | What it does |
|---|---|
| Displays | Drag monitors with edge snapping; mode, scale, rotation, VRR, with a timed revert |
| Layout | Live workspace preview drawn from niri's parsed layout; gaps, struts, column/height presets |
| Appearance | Focus ring, border (solid or gradient with angle dial), shadow, tabs, cursor, blur |
| Animations | Spring and easing editors with niri's curve maths, draggable cubic-bézier handles, live demo |
| Key bindings | Search, duplicate-combination warnings, key recorder, action picker, ▶ run now |
| Window rules | Quick rules: click a button, then click a window. Rename, enable/disable (`/-`), reorder, duplicate, delete; a live list of the windows each rule matches |
| Input, Gestures, Misc | Forms generated from the option schema |
| Startup | `spawn-at-startup`, `spawn-sh-at-startup`, environment variables |
| Raw config | Highlighted editor with niri's diagnostics, synced with the visual pages |
| History | Review unsaved changes, diff and restore earlier saved versions |

Ctrl+K searches every setting; Ctrl+S saves; Ctrl+Z / Ctrl+Shift+Z undo and redo across all
pages. **Live** applies each valid change to niri as you make it.

**Sandbox** tries your edits before they touch anything: it opens niri nested as a window,
running a private copy of the config in `$XDG_RUNTIME_DIR`, and mirrors every valid edit into
it as you make it (unsaved includes too). Your config file and the running session stay
untouched until you save. Startup programs are left out unless you ask for them, a couple of
terminals open so layout and decorations have something to show, and inside the window Mod is
Alt. The nested niri exits with the app.

**Studio appearance** (sidebar) picks the app's own theme: follow the system, Studio
Dark/Light, Catppuccin, Tokyo Night, Rosé Pine, Nord or Gruvbox; an accent that follows your
focus ring, the theme's or a custom color; density; corner radius; the desktop's font; and
whether the app draws its own title bar (drag to move, double-click to maximize, themed window
controls) or asks the compositor for decorations.
Saved to `~/.config/niri-studio/studio.conf`.

A rule's name is a `// comment` line directly above it; disabling a rule comments it out
with KDL's `/-`, so it stays in the file.

## Installing a release

Each [release](https://github.com/ulaas/niri-studio/releases) has a prebuilt x86_64 Linux
archive, built by GitHub Actions from the tagged commit. It needs only `libxkbcommon` at run time.

```sh
tar xzf niri-studio-v*-x86_64-linux.tar.gz
cd niri-studio-v*-x86_64-linux
./install.sh   # → ~/.local/bin/niri-studio + a desktop entry
```

To check the download came from this repository's release workflow:
`gh attestation verify niri-studio-v*-x86_64-linux.tar.gz --repo ulaas/niri-studio`.

## Building

You need Rust 1.87 or newer, a C toolchain, `pkg-config` and the development files for libinput,
libudev, Wayland and libxkbcommon (niri's config crate builds against them). Cargo fetches
niri's config and IPC crates from [niri's repository](https://github.com/niri-wm/niri) at the
commit pinned in `Cargo.toml`; no niri checkout is needed.

**Fedora**

```sh
sudo dnf install cargo rust gcc pkgconf-pkg-config libinput-devel systemd-devel wayland-devel libxkbcommon-devel
```

**Arch Linux**

```sh
sudo pacman -S --needed base-devel rust pkgconf libinput systemd-libs wayland libxkbcommon
```

**Debian / Ubuntu.** Their packaged Rust is usually older than 1.87, so install it with
[rustup](https://rustup.rs) and take the rest from apt:

```sh
sudo apt install build-essential pkg-config libinput-dev libudev-dev libwayland-dev libxkbcommon-dev
```

**openSUSE Tumbleweed**

```sh
sudo zypper install cargo gcc pkgconf-pkg-config libinput-devel systemd-devel wayland-devel libxkbcommon-devel
```

Then:

```sh
git clone https://github.com/ulaas/niri-studio
cd niri-studio
./install.sh            # release build → ~/.local/bin/niri-studio + a desktop entry
cargo test --workspace  # round-trips configs; checks every option against niri's parser
```

At run time niri studio talks to the niri you're running over `$NIRI_SOCKET`. The sandbox needs
the `niri` binary, and a terminal for its sample windows (foot, alacritty, kitty, ghostty,
wezterm, ptyxis, gnome-terminal or konsole). `fc-match` (fontconfig) is used to pick up your
desktop's font, when present.

To follow a newer niri, change the `rev` of `niri-config` and `niri-ipc` in `Cargo.toml`, run
`cargo update -p niri-config -p niri-ipc` and `cargo test`, and refresh
`crates/doc/tests/fixtures/niri-default-config.kdl` from that commit's
`resources/default-config.kdl`.

## Layout

- `crates/doc`: format-preserving document model (splices, undo, includes, labels, `/-`), save/backup, validation
- `crates/schema`: option catalogue, verified against niri's parser in tests
- `crates/live`: niri IPC (event stream, outputs, window picker, running actions)
- `crates/app`: the egui application (`theme.rs` holds palettes and the style built from
  them; widgets take colors from `theme::pal` rather than hard-coding them)

For UI work, `NIRI_STUDIO_SHOT=<Page>=/tmp/x.png niri-studio <config>` renders a page to a PNG
and exits (`NIRI_STUDIO_SELECT=<n>` preselects an item, `NIRI_STUDIO_SEARCH=<query>` opens
the search palette, `NIRI_STUDIO_APPEARANCE=1` the appearance panel). `NIRI_STUDIO_SANDBOX=1`
starts with the sandbox running.

`site/` is the project page, published to GitHub Pages by `.github/workflows/pages.yml`;
`site/shots.sh` regenerates its screenshots from niri's default config.

## License

GPL-3.0-or-later, like niri. niri studio is an independent project, not affiliated with niri.
Security issues: see [SECURITY.md](SECURITY.md).

## Making a release

Bump `version` in `Cargo.toml`, commit, then tag and push the tag:

```sh
git tag -a v0.2.0 -m "niri studio 0.2.0"
git push origin v0.2.0
```

`.github/workflows/release.yml` checks the tag matches `Cargo.toml`, runs the tests, builds the
binary on Ubuntu 22.04 (for an older glibc baseline), attests it and publishes the release with
generated notes.
