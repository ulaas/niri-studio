#!/usr/bin/env bash
# Installs a niri-studio release archive for the current user: the binary to ~/.local/bin and
# a desktop entry. Run it from the unpacked archive.
set -euo pipefail
cd "$(dirname "$0")"
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
install -Dm755 niri-studio "$bin/niri-studio.new"
mv -f "$bin/niri-studio.new" "$bin/niri-studio"
# Absolute Exec: launchers run with the session's PATH, which may not include ~/.local/bin.
mkdir -p "$apps"
sed "s|^Exec=niri-studio|Exec=$bin/niri-studio|" niri-studio.desktop >"$apps/niri-studio.desktop"
echo "installed $bin/niri-studio"
