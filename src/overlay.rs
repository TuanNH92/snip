//! Lớp phủ toàn màn hình kiểu Win+Shift+S: màn hình bị "đóng băng" và làm tối,
//! người dùng kéo chuột để chọn vùng (chữ nhật / tự do) hoặc click để chụp cả màn hình.
//!
//! Wayland không cho một cửa sổ trải qua nhiều màn hình, nên mỗi monitor có một cửa sổ
//! fullscreen riêng, hiển thị phần ảnh chụp tương ứng.

use crate::output;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Rect,
    Free,
    Screen,
}

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Session>>> = const { RefCell::new(None) };
    static LAST_MODE: Cell<Mode> = const { Cell::new(Mode::Rect) };
}

pub fn is_active() -> bool {
    ACTIVE.with(|a| a.borrow().is_some())
}

type DoneFn = Box<dyn FnOnce(Pixbuf)>;

struct Session {
    full: Pixbuf,
    /// pixel ảnh / đơn vị logical của GDK
    ratio: f64,
    /// góc trên-trái của toàn bộ desktop (logical)
    origin: (i32, i32),
    windows: RefCell<Vec<gtk::Window>>,
    on_done: RefCell<Option<DoneFn>>,
}

/// Widget dùng chung giữa các monitor để đồng bộ trạng thái.
#[derive(Default)]
struct Ui {
    areas: RefCell<Vec<gtk::DrawingArea>>,
    buttons: RefCell<Vec<(Mode, gtk::RadioButton)>>,
}

#[derive(Default)]
struct Drag {
    active: bool,
    start: (f64, f64),
    cur: (f64, f64),
    points: Vec<(f64, f64)>,
    hover: bool,
}

pub fn start(app: &gtk::Application, full: Pixbuf, on_done: impl FnOnce(Pixbuf) + 'static) {
    let Some(display) = gdk::Display::default() else { return };
    let geoms: Vec<gdk::Rectangle> =
        (0..display.n_monitors()).filter_map(|i| display.monitor(i)).map(|m| m.geometry()).collect();
    if geoms.is_empty() {
        return;
    }
    let minx = geoms.iter().map(|g| g.x()).min().unwrap();
    let miny = geoms.iter().map(|g| g.y()).min().unwrap();
    let maxx = geoms.iter().map(|g| g.x() + g.width()).max().unwrap();
    let ratio = full.width() as f64 / (maxx - minx).max(1) as f64;

    let session = Rc::new(Session {
        full,
        ratio,
        origin: (minx, miny),
        windows: RefCell::new(Vec::new()),
        on_done: RefCell::new(Some(Box::new(on_done))),
    });
    ACTIVE.with(|a| *a.borrow_mut() = Some(session.clone()));

    let mode = Rc::new(Cell::new(LAST_MODE.with(|m| m.get())));
    let ui = Rc::new(Ui::default());
    for (i, g) in geoms.iter().enumerate() {
        let win = build_window(app, &session, i as i32, g, &mode, &ui);
        session.windows.borrow_mut().push(win);
    }
}

fn build_window(
    app: &gtk::Application,
    session: &Rc<Session>,
    monitor: i32,
    geom: &gdk::Rectangle,
    mode: &Rc<Cell<Mode>>,
    ui: &Rc<Ui>,
) -> gtk::Window {
    let r = session.ratio;
    // phần ảnh chụp thuộc monitor này
    let ix = (((geom.x() - session.origin.0) as f64) * r).round() as i32;
    let iy = (((geom.y() - session.origin.1) as f64) * r).round() as i32;
    let ix = ix.clamp(0, session.full.width() - 1);
    let iy = iy.clamp(0, session.full.height() - 1);
    let iw = ((geom.width() as f64 * r).round() as i32).min(session.full.width() - ix).max(1);
    let ih = ((geom.height() as f64 * r).round() as i32).min(session.full.height() - iy).max(1);
    let sub = session.full.new_subpixbuf(ix, iy, iw, ih);

    let win = gtk::Window::new(gtk::WindowType::Toplevel);
    win.set_application(Some(app));
    win.set_title("Snip");
    win.set_decorated(false);
    win.set_keep_above(true);
    win.set_default_size(geom.width(), geom.height());

    let area = gtk::DrawingArea::new();
    area.add_events(
        gdk::EventMask::BUTTON_PRESS_MASK
            | gdk::EventMask::BUTTON_RELEASE_MASK
            | gdk::EventMask::POINTER_MOTION_MASK
            | gdk::EventMask::ENTER_NOTIFY_MASK
            | gdk::EventMask::LEAVE_NOTIFY_MASK,
    );
    ui.areas.borrow_mut().push(area.clone());

    let overlay = gtk::Overlay::new();
    overlay.add(&area);
    overlay.add_overlay(&build_toolbar(session, mode, ui));
    win.add(&overlay);

    let drag = Rc::new(RefCell::new(Drag::default()));

    // --- vẽ ---
    {
        let drag = drag.clone();
        let mode = mode.clone();
        area.connect_draw(move |_, cr| {
            let _ = draw(cr, &sub, r, &drag.borrow(), mode.get());
            glib::Propagation::Stop
        });
    }

    // --- chuột ---
    {
        let drag = drag.clone();
        let mode = mode.clone();
        let s = session.clone();
        let sub_rect = (ix, iy, iw, ih);
        area.connect_button_press_event(move |a, ev| {
            if ev.button() == 3 {
                s.cancel();
                return glib::Propagation::Stop;
            }
            if ev.button() != 1 {
                return glib::Propagation::Proceed;
            }
            if mode.get() == Mode::Screen {
                let (x, y, w, h) = sub_rect;
                let pb = s.full.new_subpixbuf(x, y, w, h).copy();
                if let Some(pb) = pb {
                    s.finish(pb);
                }
                return glib::Propagation::Stop;
            }
            let p = ev.position();
            let mut d = drag.borrow_mut();
            d.active = true;
            d.start = p;
            d.cur = p;
            d.points = vec![p];
            a.queue_draw();
            glib::Propagation::Stop
        });
    }
    {
        let drag = drag.clone();
        let mode = mode.clone();
        area.connect_motion_notify_event(move |a, ev| {
            let mut d = drag.borrow_mut();
            d.hover = true;
            if d.active {
                let p = ev.position();
                d.cur = p;
                if mode.get() == Mode::Free {
                    d.points.push(p);
                }
            }
            a.queue_draw();
            glib::Propagation::Stop
        });
    }
    {
        let drag = drag.clone();
        area.connect_enter_notify_event(move |a, _| {
            drag.borrow_mut().hover = true;
            a.queue_draw();
            glib::Propagation::Proceed
        });
    }
    {
        let drag = drag.clone();
        area.connect_leave_notify_event(move |a, _| {
            drag.borrow_mut().hover = false;
            a.queue_draw();
            glib::Propagation::Proceed
        });
    }
    {
        let drag = drag.clone();
        let mode = mode.clone();
        let s = session.clone();
        let offset = (geom.x() - session.origin.0, geom.y() - session.origin.1);
        area.connect_button_release_event(move |a, ev| {
            if ev.button() != 1 || !drag.borrow().active {
                return glib::Propagation::Proceed;
            }
            let result = {
                let mut d = drag.borrow_mut();
                d.active = false;
                d.cur = ev.position();
                match mode.get() {
                    Mode::Rect => crop_rect(&s, offset, d.start, d.cur),
                    Mode::Free => crop_free(&s, offset, &d.points),
                    Mode::Screen => None,
                }
            };
            match result {
                Some(pb) => s.finish(pb),
                None => a.queue_draw(), // click không kéo: bỏ qua
            }
            glib::Propagation::Stop
        });
    }

    // --- bàn phím ---
    {
        let s = session.clone();
        win.connect_key_press_event(move |_, ev| {
            if ev.keyval() == gdk::keys::constants::Escape {
                s.cancel();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    {
        let s = session.clone();
        win.connect_delete_event(move |_, _| {
            s.cancel();
            glib::Propagation::Stop
        });
    }

    win.connect_realize(|w| {
        if let (Some(gw), Some(display)) = (w.window(), gdk::Display::default()) {
            gw.set_cursor(gdk::Cursor::for_display(&display, gdk::CursorType::Crosshair).as_ref());
        }
    });

    if let Some(screen) = gdk::Screen::default() {
        win.fullscreen_on_monitor(&screen, monitor);
    }
    win.show_all();
    win.present();
    win
}

fn build_toolbar(session: &Rc<Session>, mode: &Rc<Cell<Mode>>, ui: &Rc<Ui>) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    bar.style_context().add_class("snip-toolbar");
    bar.set_halign(gtk::Align::Center);
    bar.set_valign(gtk::Align::Start);
    bar.set_margin_top(16);

    let items = [
        (Mode::Rect, "▭  Chữ nhật", "Chụp vùng hình chữ nhật"),
        (Mode::Free, "✎  Tự do", "Chụp vùng vẽ tự do"),
        (Mode::Screen, "⛶  Toàn màn hình", "Click vào màn hình cần chụp"),
    ];
    let mut group: Option<gtk::RadioButton> = None;
    for (m, label, tip) in items {
        let btn = match group.as_ref() {
            Some(g) => gtk::RadioButton::with_label_from_widget(g, label),
            None => gtk::RadioButton::with_label(label),
        };
        if group.is_none() {
            group = Some(btn.clone());
        }
        btn.set_mode(false);
        btn.set_tooltip_text(Some(tip));
        btn.set_can_focus(false);
        if mode.get() == m {
            btn.set_active(true);
        }
        let mode = mode.clone();
        let ui_ = ui.clone();
        btn.connect_toggled(move |b| {
            if !b.is_active() || mode.get() == m {
                return;
            }
            mode.set(m);
            LAST_MODE.with(|l| l.set(m));
            // đồng bộ thanh công cụ ở các monitor khác
            let others: Vec<_> =
                ui_.buttons.borrow().iter().filter(|(bm, _)| *bm == m).map(|(_, b)| b.clone()).collect();
            for o in others {
                o.set_active(true);
            }
            for a in ui_.areas.borrow().iter() {
                a.queue_draw();
            }
        });
        ui.buttons.borrow_mut().push((m, btn.clone()));
        bar.pack_start(&btn, false, false, 0);
    }

    bar.pack_start(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, 4);
    let close = gtk::Button::with_label("✕");
    close.set_tooltip_text(Some("Hủy (Esc)"));
    close.set_can_focus(false);
    let s = session.clone();
    close.connect_clicked(move |_| s.cancel());
    bar.pack_start(&close, false, false, 0);
    bar
}

fn draw(cr: &cairo::Context, sub: &Pixbuf, r: f64, d: &Drag, mode: Mode) -> Result<(), cairo::Error> {
    let paint_image = |cr: &cairo::Context| -> Result<(), cairo::Error> {
        cr.save()?;
        cr.scale(1.0 / r, 1.0 / r);
        cr.set_source_pixbuf(sub, 0.0, 0.0);
        cr.paint()?;
        cr.restore()
    };

    paint_image(cr)?;
    if mode == Mode::Screen && d.hover {
        // màn hình đang trỏ chuột sẽ sáng, viền xanh
        let w = sub.width() as f64 / r;
        let h = sub.height() as f64 / r;
        cr.set_source_rgb(0.24, 0.49, 1.0);
        cr.set_line_width(6.0);
        cr.rectangle(3.0, 3.0, w - 6.0, h - 6.0);
        return cr.stroke();
    }
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
    cr.paint()?;

    if !d.active {
        return Ok(());
    }
    match mode {
        Mode::Rect => {
            let (x, y, w, h) = norm(d.start, d.cur);
            cr.save()?;
            cr.rectangle(x, y, w, h);
            cr.clip();
            paint_image(cr)?;
            cr.restore()?;

            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(1.0);
            cr.rectangle(x.floor() + 0.5, y.floor() + 0.5, w, h);
            cr.stroke()?;

            // kích thước (pixel thật)
            let text = format!("{} × {}", (w * r).round(), (h * r).round());
            cr.set_font_size(13.0);
            let ext = cr.text_extents(&text)?;
            let ty = if y > 28.0 { y - 8.0 } else { y + h + 20.0 };
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.75);
            cr.rectangle(x, ty - ext.height() - 6.0, ext.width() + 12.0, ext.height() + 10.0);
            cr.fill()?;
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.move_to(x + 6.0, ty);
            cr.show_text(&text)?;
        }
        Mode::Free if d.points.len() > 1 => {
            trace(cr, &d.points);
            cr.save()?;
            cr.clip_preserve();
            paint_image(cr)?;
            cr.restore()?;
            trace(cr, &d.points);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(2.0);
            cr.stroke()?;
        }
        _ => {}
    }
    Ok(())
}

fn trace(cr: &cairo::Context, pts: &[(f64, f64)]) {
    cr.new_path();
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 {
            cr.move_to(x, y);
        } else {
            cr.line_to(x, y);
        }
    }
    cr.close_path();
}

fn norm(a: (f64, f64), b: (f64, f64)) -> (f64, f64, f64, f64) {
    (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs())
}

/// Điểm (logical, trong monitor) -> pixel trên ảnh chụp toàn desktop.
fn to_image(s: &Session, offset: (i32, i32), p: (f64, f64)) -> (f64, f64) {
    ((p.0 + offset.0 as f64) * s.ratio, (p.1 + offset.1 as f64) * s.ratio)
}

fn clamp_rect(s: &Session, x0: f64, y0: f64, x1: f64, y1: f64) -> Option<(i32, i32, i32, i32)> {
    let x0 = (x0.floor() as i32).clamp(0, s.full.width());
    let y0 = (y0.floor() as i32).clamp(0, s.full.height());
    let x1 = (x1.ceil() as i32).clamp(0, s.full.width());
    let y1 = (y1.ceil() as i32).clamp(0, s.full.height());
    let (w, h) = (x1 - x0, y1 - y0);
    (w >= 4 && h >= 4).then_some((x0, y0, w, h))
}

fn crop_rect(s: &Session, offset: (i32, i32), a: (f64, f64), b: (f64, f64)) -> Option<Pixbuf> {
    let a = to_image(s, offset, a);
    let b = to_image(s, offset, b);
    let (x, y, w, h) = clamp_rect(s, a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))?;
    s.full.new_subpixbuf(x, y, w, h).copy()
}

/// Vùng tự do: cắt theo bounding box, phần ngoài đường vẽ trong suốt.
fn crop_free(s: &Session, offset: (i32, i32), pts: &[(f64, f64)]) -> Option<Pixbuf> {
    if pts.len() < 3 {
        return None;
    }
    let pts: Vec<_> = pts.iter().map(|&p| to_image(s, offset, p)).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in &pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (x, y, w, h) = clamp_rect(s, x0, y0, x1, y1)?;
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        cr.translate(-x as f64, -y as f64);
        trace(&cr, &pts);
        cr.clip();
        cr.set_source_pixbuf(&s.full, 0.0, 0.0);
        cr.paint().ok()?;
    }
    gdk::pixbuf_get_from_surface(&surface, 0, 0, w, h)
}

impl Session {
    fn close(&self) {
        for w in self.windows.borrow_mut().drain(..) {
            // SAFETY: cửa sổ do ta tạo, không còn được dùng sau khi destroy.
            unsafe { w.destroy() };
        }
        ACTIVE.with(|a| a.borrow_mut().take());
    }

    fn cancel(&self) {
        self.on_done.borrow_mut().take();
        self.close();
    }

    fn finish(&self, pb: Pixbuf) {
        // Wayland yêu cầu set clipboard khi cửa sổ còn focus (có serial của sự kiện chuột).
        output::copy_to_clipboard(&pb);
        let done = self.on_done.borrow_mut().take();
        self.close();
        if let Some(done) = done {
            // để overlay biến mất ngay rồi mới lưu file
            glib::idle_add_local_once(move || done(pb));
        }
    }
}
