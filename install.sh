#!/usr/bin/env bash
# Build niri-studio in release mode and install it for the current user.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --locked -p niri-studio
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
install -Dm755 target/release/niri-studio "$bin/niri-studio.new"
mv -f "$bin/niri-studio.new" "$bin/niri-studio"
install -Dm644 resources/niri-studio.desktop "$apps/niri-studio.desktop"
echo "installed $bin/niri-studio"
