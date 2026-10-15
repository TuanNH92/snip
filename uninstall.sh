#!/usr/bin/env bash
set -euo pipefail
SCHEMA=org.gnome.settings-daemon.plugins.media-keys
KPATH=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/snip/
pkill -x snip 2>/dev/null || true
rm -f "$HOME/.local/bin/snip" "$HOME/.local/share/applications/io.github.tuannh92.Snip.desktop"
current=$(gsettings get $SCHEMA custom-keybindings)
new=$(echo "$current" | sed -e "s|, '$KPATH'||; s|'$KPATH', ||; s|'$KPATH'||")
[[ "$new" == "[]" ]] && new="@as []"
gsettings set $SCHEMA custom-keybindings "$new"
gsettings reset-recursively $SCHEMA.custom-keybinding:$KPATH
echo "Đã gỡ Snip."
