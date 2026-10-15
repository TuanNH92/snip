//! Clipboard, lưu file và thông báo sau khi chụp.

use crate::APP_ID;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib::{self, ToVariant};
use gtk::prelude::*;
use gtk::{gdk, gio};
use std::path::{Path, PathBuf};

const CSS: &str = r#"
.snip-toolbar {
    background-color: rgba(32, 32, 32, 0.96);
    border-radius: 10px;
    padding: 6px;
    border: 1px solid rgba(255, 255, 255, 0.12);
}
.snip-toolbar button, .snip-toolbar radiobutton {
    color: #ffffff;
    background: none;
    border: none;
    box-shadow: none;
    text-shadow: none;
    -gtk-icon-shadow: none;
    padding: 6px 12px;
    border-radius: 6px;
    min-height: 0;
}
.snip-toolbar button:hover, .snip-toolbar radiobutton:hover {
    background-color: rgba(255, 255, 255, 0.12);
}
.snip-toolbar button:checked, .snip-toolbar radiobutton:checked {
    background-color: #3d7eff;
}
"#;

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(CSS.as_bytes()).is_ok() {
        if let Some(screen) = gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(&screen, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }
}

pub fn copy_to_clipboard(pb: &Pixbuf) {
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_image(pb);
}

pub fn screenshots_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| glib::home_dir().join("Pictures"))
        .join("Screenshots")
}

fn save(pb: &Pixbuf) -> Result<PathBuf, String> {
    let dir = screenshots_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = glib::DateTime::now_local()
        .and_then(|d| d.format("%Y-%m-%d_%H-%M-%S"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "snip".into());
    let mut path = dir.join(format!("Snip_{stamp}.png"));
    let mut i = 1;
    while path.exists() {
        path = dir.join(format!("Snip_{stamp}_{i}.png"));
        i += 1;
    }
    pb.savev(&path, "png", &[]).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Gọi sau khi overlay đã đóng (clipboard đã được set trong overlay).
pub fn handle(app: &gtk::Application, pb: &Pixbuf) {
    match save(pb) {
        Ok(path) => notify_saved(app, &path),
        Err(e) => notify_error(app, &format!("Đã sao chép vào clipboard nhưng không lưu được file: {e}")),
    }
}

/// GNOME Shell chỉ hiển thị GNotification khi có file .desktop trùng app id.
fn has_desktop_file() -> bool {
    let name = format!("applications/{APP_ID}.desktop");
    std::iter::once(glib::user_data_dir())
        .chain(glib::system_data_dirs())
        .any(|d| d.join(&name).exists())
}

fn notify_saved(app: &gtk::Application, path: &Path) {
    let title = "Đã sao chép ảnh chụp vào clipboard";
    let body = format!("Đã lưu: {}\nBấm để chỉnh sửa.", path.display());
    if !has_desktop_file() {
        let _ = std::process::Command::new("notify-send")
            .args(["-a", "Snip", "-i", &path.to_string_lossy(), title, &body])
            .spawn();
        return;
    }
    let p = path.to_string_lossy().to_string();
    let n = gio::Notification::new(title);
    n.set_body(Some(&body));
    n.set_icon(&gio::FileIcon::new(&gio::File::for_path(path)));
    n.set_default_action_and_target_value("app.edit", Some(&p.to_variant()));
    n.add_button_with_target_value("Chỉnh sửa", "app.edit", Some(&p.to_variant()));
    n.add_button_with_target_value("Mở thư mục", "app.open-folder", Some(&p.to_variant()));
    app.send_notification(Some("snip"), &n);
}

pub fn notify_error(app: &gtk::Application, msg: &str) {
    eprintln!("snip: {msg}");
    notify_info(app, "Snip", msg);
}

pub fn notify_info(app: &gtk::Application, title: &str, body: &str) {
    if has_desktop_file() {
        let n = gio::Notification::new(title);
        n.set_body(Some(body));
        app.send_notification(None, &n);
    } else {
        let _ = std::process::Command::new("notify-send").args(["-a", "Snip", title, body]).spawn();
    }
}

pub fn open_folder(file: &Path) {
    let dir = file.parent().unwrap_or(file);
    let uri = gio::File::for_path(dir).uri();
    let _ = gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE);
}
