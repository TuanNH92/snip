#!/usr/bin/env bash
# Cài Snip vào ~/.local và gán phím tắt Super+Shift+S.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

APP_ID=io.github.tuannh92.Snip
BIN="$HOME/.local/bin/snip"
KEY="${SNIP_SHORTCUT:-<Shift><Super>s}"

cargo build --release
pkill -x snip 2>/dev/null || true
install -Dm755 target/release/snip "$BIN"
install -Dm644 "data/$APP_ID.desktop" "$HOME/.local/share/applications/$APP_ID.desktop"
sed -i "s|^Exec=.*|Exec=$BIN|" "$HOME/.local/share/applications/$APP_ID.desktop"

# Custom keybinding của GNOME
SCHEMA=org.gnome.settings-daemon.plugins.media-keys
KPATH=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/snip/
current=$(gsettings get $SCHEMA custom-keybindings)
if [[ "$current" != *"$KPATH"* ]]; then
    if [[ "$current" == "@as []" || "$current" == "[]" ]]; then
        new="['$KPATH']"
    else
        new="${current%]}, '$KPATH']"
    fi
    gsettings set $SCHEMA custom-keybindings "$new"
fi
gsettings set $SCHEMA.custom-keybinding:$KPATH name 'Snip'
gsettings set $SCHEMA.custom-keybinding:$KPATH command "$BIN"
gsettings set $SCHEMA.custom-keybinding:$KPATH binding "$KEY"

echo "Đã cài: $BIN"
echo "Phím tắt: $KEY"
