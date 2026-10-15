//! Cửa sổ chỉnh sửa đơn giản (mở khi bấm vào thông báo): bút, bút dạ quang,
//! hoàn tác, sao chép, lưu.

use crate::output;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pen,
    Highlighter,
}

struct Stroke {
    tool: Tool,
    rgb: (f64, f64, f64),
    points: Vec<(f64, f64)>,
}

struct State {
    img: Pixbuf,
    scale: f64,
    tool: Cell<Tool>,
    rgb: Cell<(f64, f64, f64)>,
    strokes: RefCell<Vec<Stroke>>,
    path: PathBuf,
}

pub fn open(app: &gtk::Application, path: &Path) {
    let img = match Pixbuf::from_file(path) {
        Ok(p) => p,
        Err(e) => return output::notify_error(app, &format!("Không mở được ảnh: {e}")),
    };
    let (w, h) = (img.width() as f64, img.height() as f64);
    let scale = (1400.0 / w).min(850.0 / h).min(1.0);
    let st = Rc::new(State {
        img,
        scale,
        tool: Cell::new(Tool::Pen),
        rgb: Cell::new((0.9, 0.1, 0.1)),
        strokes: RefCell::default(),
        path: path.to_path_buf(),
    });

    let win = gtk::ApplicationWindow::new(app);
    let header = gtk::HeaderBar::new();
    header.set_show_close_button(true);
    header.set_title(Some("Snip"));
    header.set_subtitle(path.file_name().and_then(|n| n.to_str()));

    let pen = gtk::RadioButton::with_label("✎ Bút");
    let hl = gtk::RadioButton::with_label_from_widget(&pen, "▮ Dạ quang");
    for (b, t) in [(&pen, Tool::Pen), (&hl, Tool::Highlighter)] {
        b.set_mode(false);
        let st = st.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                st.tool.set(t);
            }
        });
    }
    let color = gtk::ColorButton::with_rgba(&gdk::RGBA::new(0.9, 0.1, 0.1, 1.0));
    color.set_tooltip_text(Some("Màu"));
    {
        let st = st.clone();
        color.connect_color_set(move |c| {
            let c = c.rgba();
            st.rgb.set((c.red(), c.green(), c.blue()));
        });
    }
    let undo = gtk::Button::from_icon_name(Some("edit-undo-symbolic"), gtk::IconSize::Button);
    undo.set_tooltip_text(Some("Hoàn tác (Ctrl+Z)"));
    let copy = gtk::Button::with_label("Sao chép");
    copy.set_tooltip_text(Some("Ctrl+C"));
    let save = gtk::Button::with_label("Lưu…");
    save.set_tooltip_text(Some("Ctrl+S"));
    save.style_context().add_class("suggested-action");

    let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tools.style_context().add_class("linked");
    tools.pack_start(&pen, false, false, 0);
    tools.pack_start(&hl, false, false, 0);
    header.pack_start(&tools);
    header.pack_start(&color);
    header.pack_start(&undo);
    header.pack_end(&save);
    header.pack_end(&copy);
    win.set_titlebar(Some(&header));

    let area = gtk::DrawingArea::new();
    area.set_size_request((w * scale) as i32, (h * scale) as i32);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.add_events(
        gdk::EventMask::BUTTON_PRESS_MASK | gdk::EventMask::BUTTON_RELEASE_MASK | gdk::EventMask::BUTTON1_MOTION_MASK,
    );
    let scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    scroll.add(&area);
    win.add(&scroll);
    win.set_default_size((w * scale) as i32 + 48, (h * scale) as i32 + 48);

    {
        let st = st.clone();
        area.connect_draw(move |_, cr| {
            cr.scale(st.scale, st.scale);
            let _ = paint(cr, &st);
            glib::Propagation::Stop
        });
    }
    {
        let st = st.clone();
        area.connect_button_press_event(move |a, ev| {
            if ev.button() == 1 {
                let (x, y) = ev.position();
                st.strokes.borrow_mut().push(Stroke {
                    tool: st.tool.get(),
                    rgb: st.rgb.get(),
                    points: vec![(x / st.scale, y / st.scale)],
                });
                a.queue_draw();
            }
            glib::Propagation::Stop
        });
    }
    {
        let st = st.clone();
        area.connect_motion_notify_event(move |a, ev| {
            let (x, y) = ev.position();
            if let Some(s) = st.strokes.borrow_mut().last_mut() {
                s.points.push((x / st.scale, y / st.scale));
            }
            a.queue_draw();
            glib::Propagation::Stop
        });
    }

    let do_undo = {
        let st = st.clone();
        let area = area.clone();
        move || {
            st.strokes.borrow_mut().pop();
            area.queue_draw();
        }
    };
    let do_copy = {
        let st = st.clone();
        move || {
            if let Some(pb) = render(&st) {
                output::copy_to_clipboard(&pb);
            }
        }
    };
    let do_save = {
        let st = st.clone();
        let win = win.clone();
        move || save_as(&win, &st)
    };

    {
        let f = do_undo.clone();
        undo.connect_clicked(move |_| f());
    }
    {
        let f = do_copy.clone();
        copy.connect_clicked(move |_| f());
    }
    {
        let f = do_save.clone();
        save.connect_clicked(move |_| f());
    }
    win.connect_key_press_event(move |_, ev| {
        if !ev.state().contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        use gdk::keys::constants as k;
        let key = ev.keyval();
        if key == k::z || key == k::Z {
            do_undo();
        } else if key == k::c || key == k::C {
            do_copy();
        } else if key == k::s || key == k::S {
            do_save();
        } else {
            return glib::Propagation::Proceed;
        }
        glib::Propagation::Stop
    });

    win.show_all();
    win.present();
}

fn paint(cr: &cairo::Context, st: &State) -> Result<(), cairo::Error> {
    cr.set_source_pixbuf(&st.img, 0.0, 0.0);
    cr.paint()?;
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    for s in st.strokes.borrow().iter() {
        let (r, g, b) = s.rgb;
        match s.tool {
            Tool::Pen => {
                cr.set_source_rgb(r, g, b);
                cr.set_line_width(3.0);
            }
            Tool::Highlighter => {
                cr.set_source_rgba(r, g, b, 0.35);
                cr.set_line_width(18.0);
            }
        }
        cr.new_path();
        let first = s.points[0];
        cr.move_to(first.0, first.1);
        if s.points.len() == 1 {
            cr.line_to(first.0 + 0.01, first.1);
        }
        for &(x, y) in &s.points[1..] {
            cr.line_to(x, y);
        }
        cr.stroke()?;
    }
    Ok(())
}

fn render(st: &State) -> Option<Pixbuf> {
    let (w, h) = (st.img.width(), st.img.height());
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        paint(&cr, st).ok()?;
    }
    gdk::pixbuf_get_from_surface(&surface, 0, 0, w, h)
}

fn save_as(win: &gtk::ApplicationWindow, st: &State) {
    let dialog = gtk::FileChooserNative::new(
        Some("Lưu ảnh chụp"),
        Some(win),
        gtk::FileChooserAction::Save,
        Some("Lưu"),
        Some("Hủy"),
    );
    dialog.set_do_overwrite_confirmation(true);
    if let Some(dir) = st.path.parent() {
        let _ = dialog.set_current_folder(dir);
    }
    if let Some(name) = st.path.file_name().and_then(|n| n.to_str()) {
        dialog.set_current_name(name);
    }
    if dialog.run() != gtk::ResponseType::Accept {
        return;
    }
    let Some(mut path) = dialog.filename() else { return };
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    let format = match ext.as_deref() {
        Some("jpg") | Some("jpeg") => "jpeg",
        Some("bmp") => "bmp",
        Some("png") => "png",
        _ => {
            path.set_extension("png");
            "png"
        }
    };
    let Some(mut pb) = render(st) else { return };
    if format != "png" {
        // JPEG/BMP không có alpha
        if let Some(flat) = pb.composite_color_simple(pb.width(), pb.height(), gtk::gdk_pixbuf::InterpType::Nearest, 255, 1, 0xffffff, 0xffffff) {
            pb = flat;
        }
    }
    if let Err(e) = pb.savev(&path, format, &[]) {
        let msg = gtk::MessageDialog::new(
            Some(win),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Error,
            gtk::ButtonsType::Close,
            &format!("Không lưu được: {e}"),
        );
        msg.run();
        // SAFETY: dialog không còn được dùng sau khi destroy.
        unsafe { msg.destroy() };
    }
}
