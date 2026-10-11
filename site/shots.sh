#!/usr/bin/env bash
# Regenerates site/assets/shots/*.webp from niri's default config (the copy kept as a test
# fixture), so no personal config ends up in the images. Needs a running niri session, kitty,
# jq, ImageMagick and a release build.
# A nested niri window and the app open briefly while it runs.
set -euo pipefail
cd "$(dirname "$0")/.."
default_config=crates/doc/tests/fixtures/niri-default-config.kdl
work=$(mktemp -d)
trap 'kill "${npid:-}" 2>/dev/null || true; rm -rf "$work"' EXIT

cargo build --release -p niri-studio
mkdir -p "$work/home/.config/niri" "$work/raw" "$work/nested" site/assets/shots
cp "$default_config" "$work/home/.config/niri/config.kdl"
sed '/^spawn-at-startup\|^spawn-sh-at-startup/d' "$default_config" >"$work/nested/config.kdl"

# A nested niri with a couple of windows stands in for the live session.
env -u NIRI_SOCKET HOME="$work/home" niri --config "$work/nested/config.kdl" >/dev/null 2>&1 &
npid=$!
for _ in $(seq 50); do
  sock=$(ls "$XDG_RUNTIME_DIR"/niri.*."$npid".sock 2>/dev/null || true)
  [ -n "$sock" ] && break
  sleep 0.2
done
[ -n "$sock" ] || { echo "nested niri didn't start" >&2; exit 1; }
# The nested output is as big as its window; float that window at 16:9 so it looks like a monitor.
nid=
for _ in $(seq 50); do
  nid=$(niri msg --json windows | jq -r --argjson pid "$npid" '.[] | select(.pid == $pid) | .id' | head -1)
  [ -n "$nid" ] && break
  sleep 0.2
done
if [ -n "$nid" ]; then
  niri msg action move-window-to-floating --id "$nid" >/dev/null
  niri msg action set-window-width --id "$nid" 1600 >/dev/null
  niri msg action set-window-height --id "$nid" 900 >/dev/null
  sleep 0.8
fi
for title in Editor Browser; do NIRI_SOCKET=$sock niri msg action spawn -- kitty -d /tmp --title "$title" >/dev/null || true; done
sleep 1.5

shot() { # name theme accent page [VAR=value...]
  local name=$1 theme=$2 accent=$3 page=$4
  shift 4
  mkdir -p "$work/home/.config/niri-studio"
  printf 'theme = %s\naccent = %s\n' "$theme" "$accent" >"$work/home/.config/niri-studio/studio.conf"
  env "$@" HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" NIRI_SOCKET="$sock" \
    NIRI_STUDIO_SCALE=1.5 NIRI_STUDIO_SHOT="$page=$work/raw/$name.png" timeout 40 target/release/niri-studio >/dev/null 2>&1
  magick "$work/raw/$name.png" -strip -quality 80 "site/assets/shots/$name.webp"
  echo "$name"
}
shot hero studio-dark focus-ring Layout
shot themes catppuccin-mocha theme Appearance NIRI_STUDIO_APPEARANCE=1
shot keybinds tokyo-night theme Keybinds NIRI_STUDIO_SELECT=4
shot rules rose-pine theme Rules NIRI_STUDIO_SELECT=0
shot animations nord theme Animations
shot raw gruvbox-dark theme Raw
shot search studio-light theme Layout NIRI_STUDIO_SEARCH=focus
shot sandbox studio-dark focus-ring Appearance NIRI_STUDIO_SANDBOX=1
shot light catppuccin-latte theme Layout
shot input studio-dark theme Input
magick "$work/raw/hero.png" -strip -resize 1200x -gravity north -crop 1200x630+0+0 +repage -quality 85 site/assets/og.png
