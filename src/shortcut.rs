//! Tự gán phím tắt Super+Shift+S trong GNOME ở lần chạy đầu tiên.
//!
//! Gói .deb cài cho cả hệ thống nên không đặt được gsettings của từng người dùng;
//! vì vậy app tự làm việc này khi được mở lần đầu. Chỉ làm một lần (có file đánh dấu),
//! để nếu người dùng xoá/đổi phím tắt thì app không ghi đè lại.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/snip/";
pub const BINDING: &str = "<Shift><Super>s";

/// Trả về `true` nếu vừa gán phím tắt (để báo cho người dùng).
pub fn ensure_first_run() -> bool {
    let marker = glib::user_config_dir().join("snip").join("shortcut-registered");
    if marker.exists() {
        return false;
    }
    let ok = register();
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, BINDING);
    ok
}

fn register() -> bool {
    let Some(src) = gio::SettingsSchemaSource::default() else { return false };
    if src.lookup(MEDIA_KEYS, true).is_none() || src.lookup(CUSTOM, true).is_none() {
        return false; // không phải GNOME
    }
    let command = std::env::current_exe()
        .ok()
        .filter(|p| p.starts_with("/usr") || p.starts_with(glib::home_dir().join(".local")))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "snip".into());

    let mk = gio::Settings::new(MEDIA_KEYS);
    let mut list: Vec<String> = mk.strv("custom-keybindings").iter().map(|s| s.to_string()).collect();
    if !list.iter().any(|p| p == PATH) {
        list.push(PATH.into());
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        if mk.set_strv("custom-keybindings", refs.as_slice()).is_err() {
            return false;
        }
    }
    let kb = gio::Settings::with_path(CUSTOM, PATH);
    let ok = kb.set_string("name", "Snip").is_ok()
        && kb.set_string("command", &command).is_ok()
        && kb.set_string("binding", BINDING).is_ok();
    gio::Settings::sync();
    ok
}
