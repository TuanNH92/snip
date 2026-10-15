mod capture;
mod editor;
mod output;
mod overlay;
mod shortcut;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.tuannh92.Snip";

fn main() -> glib::ExitCode {
    // Single instance: lần chạy `snip` thứ hai chỉ gửi "activate" tới tiến trình đang chạy,
    // tiến trình đó vẫn sống để giữ nội dung clipboard (Wayland mất clipboard khi app thoát).
    let app = gtk::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|app| {
        std::mem::forget(app.hold());
        output::install_css();

        let edit = gio::SimpleAction::new("edit", Some(glib::VariantTy::STRING));
        let a = app.clone();
        edit.connect_activate(move |_, param| {
            if let Some(path) = param.and_then(|p| p.get::<String>()) {
                editor::open(&a, &PathBuf::from(path));
            }
        });
        app.add_action(&edit);

        let folder = gio::SimpleAction::new("open-folder", Some(glib::VariantTy::STRING));
        folder.connect_activate(|_, param| {
            if let Some(path) = param.and_then(|p| p.get::<String>()) {
                output::open_folder(&PathBuf::from(path));
            }
        });
        app.add_action(&folder);

        let quit = gio::SimpleAction::new("quit", None);
        let a = app.clone();
        quit.connect_activate(move |_, _| a.quit());
        app.add_action(&quit);

        if shortcut::ensure_first_run() {
            output::notify_info(app, "Snip đã sẵn sàng", "Nhấn Super+Shift+S để chụp màn hình bất cứ lúc nào.");
        }
    });

    app.connect_activate(snip);
    app.run()
}

fn snip(app: &gtk::Application) {
    if overlay::is_active() {
        return;
    }
    match capture::capture() {
        Ok(full) => {
            let a = app.clone();
            overlay::start(app, full, move |pb| output::handle(&a, &pb));
        }
        Err(e) => output::notify_error(app, &format!("Không chụp được màn hình: {e}")),
    }
}
