//! Chụp toàn bộ màn hình thành một Pixbuf.
//!
//! Trên Wayland, ứng dụng thường không được đọc framebuffer. GNOME Shell chỉ cho phép
//! gọi `org.gnome.Shell.Screenshot` từ một số tên D-Bus trong allowlist (trong đó có
//! `org.gnome.Screenshot`), nên ta tạm giữ tên đó rồi gọi trực tiếp — không flash, không
//! hộp thoại. Nếu thất bại thì dùng `gnome-screenshot` làm phương án dự phòng.

use gtk::gdk_pixbuf::Pixbuf;
use gtk::gio;
use gtk::glib::{self, ToVariant, VariantTy};
use std::path::Path;
use std::process::Command;

pub fn capture() -> Result<Pixbuf, String> {
    let path = glib::user_runtime_dir().join(format!("snip-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&path);

    shell_dbus(&path).or_else(|e1| gnome_screenshot_cli(&path).map_err(|e2| format!("{e1}; {e2}")))?;

    let pb = Pixbuf::from_file(&path).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&path);
    pb
}

fn dbus_call(
    conn: &gio::DBusConnection,
    dest: &str,
    object: &str,
    iface: &str,
    method: &str,
    args: glib::Variant,
    reply: &str,
) -> Result<glib::Variant, String> {
    conn.call_sync(
        Some(dest),
        object,
        iface,
        method,
        Some(&args),
        Some(VariantTy::new(reply).unwrap()),
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
    )
    .map_err(|e| e.to_string())
}

fn shell_dbus(path: &Path) -> Result<(), String> {
    const NAME: &str = "org.gnome.Screenshot";
    let conn = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).map_err(|e| e.to_string())?;
    let bus = |method: &str, args: glib::Variant| {
        dbus_call(&conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", method, args, "(u)")
    };

    // 4 = DBUS_NAME_FLAG_DO_NOT_QUEUE; reply 1 = primary owner, 4 = already owner
    let (code,): (u32,) = bus("RequestName", (NAME, 4u32).to_variant())?.get().ok_or("RequestName: bad reply")?;
    if code != 1 && code != 4 {
        return Err(format!("{NAME} đang bị chiếm"));
    }

    let res = dbus_call(
        &conn,
        "org.gnome.Shell.Screenshot",
        "/org/gnome/Shell/Screenshot",
        "org.gnome.Shell.Screenshot",
        "Screenshot",
        (false, false, path.to_string_lossy().as_ref()).to_variant(),
        "(bs)",
    );
    let _ = bus("ReleaseName", (NAME,).to_variant());

    let (ok, _): (bool, String) = res?.get().ok_or("Screenshot: bad reply")?;
    if ok && path.exists() {
        Ok(())
    } else {
        Err("GNOME Shell từ chối chụp".into())
    }
}

fn gnome_screenshot_cli(path: &Path) -> Result<(), String> {
    let status = Command::new("gnome-screenshot")
        .arg("-f")
        .arg(path)
        .status()
        .map_err(|e| format!("gnome-screenshot: {e}"))?;
    if status.success() && path.exists() {
        Ok(())
    } else {
        Err("gnome-screenshot thất bại".into())
    }
}
