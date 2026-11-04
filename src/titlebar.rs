//! Our own window chrome, Discord style: a slim bar with the app name on the
//! left and window controls on the right, replacing the OS title bar.
//!
//! On Windows and Linux the window is undecorated, so this bar also has to do
//! the dragging, double-click maximize and edge resizing the OS did before.
//! macOS keeps its traffic lights (the content just extends under them), so
//! there we only leave room for them and skip our own buttons. Android is
//! always fullscreen with the system bars around it, so none of this applies.

use crate::theme::semibold;
use crate::widgets as w;
use egui::{Color32, CornerRadius, Frame, Pos2, Rect, ResizeDirection, RichText, Sense, Stroke, Ui, Vec2, ViewportCommand};

pub const HEIGHT: f32 = if cfg!(target_os = "android") { 0.0 } else { 30.0 };

/// Width of the invisible grab strip along the window edges.
const EDGE: f32 = 5.0;

pub fn configure(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    if cfg!(target_os = "macos") {
        viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false)
    } else {
        viewport.with_decorations(false)
    }
}

pub fn show(ui: &mut Ui) {
    if cfg!(target_os = "android") {
        return;
    }
    let pal = w::p(ui);
    let ctx = ui.ctx().clone();
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    egui::Panel::top("rivet-titlebar").exact_size(HEIGHT).show_separator_line(false).frame(Frame::new().fill(pal.titlebar_bg)).show(
        ui,
        |ui| {
            let bar = ui.max_rect();
            // registered first so the buttons added after it win the clicks
            let drag = ui.interact(bar, ui.scope_id().with("titlebar-drag"), Sense::click_and_drag());
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }

            let left_pad = if cfg!(target_os = "macos") { 76.0 } else { 10.0 };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.add_space(left_pad);
            child.add(egui::Image::new(egui::include_image!("../assets/rivet.png")).fit_to_exact_size(Vec2::splat(16.0)));
            child.add_space(2.0);
            child.label(RichText::new("Rivet").font(semibold(12.0)).color(pal.fg_muted));

            if cfg!(target_os = "macos") {
                return;
            }
            let btn = Vec2::new(46.0, HEIGHT);
            let close_rect = Rect::from_min_size(Pos2::new(bar.right() - btn.x, bar.top()), btn);
            let max_rect = close_rect.translate(Vec2::new(-btn.x, 0.0));
            let min_rect = max_rect.translate(Vec2::new(-btn.x, 0.0));
            if control(ui, min_rect, "minimize", false, |p, c, color| {
                p.hline(c.x - 5.0..=c.x + 5.0, c.y, Stroke::new(1.0, color));
            })
            .on_hover_text("Minimize")
            .clicked()
            {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            let tip = if maximized { "Restore" } else { "Maximize" };
            if control(ui, max_rect, "maximize", false, |p, c, color| {
                let s = Stroke::new(1.0, color);
                if maximized {
                    // two overlapping squares, the usual restore glyph
                    p.rect_stroke(Rect::from_center_size(c + Vec2::new(-1.5, 1.5), Vec2::splat(8.0)), 1.0, s, egui::StrokeKind::Inside);
                    p.line_segment([c + Vec2::new(-2.5, -4.5), c + Vec2::new(5.5, -4.5)], s);
                    p.line_segment([c + Vec2::new(5.5, -4.5), c + Vec2::new(5.5, 3.5)], s);
                } else {
                    p.rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0)), 1.0, s, egui::StrokeKind::Inside);
                }
            })
            .on_hover_text(tip)
            .clicked()
            {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if control(ui, close_rect, "close", true, |p, c, color| {
                let s = Stroke::new(1.0, color);
                p.line_segment([c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, 5.0)], s);
                p.line_segment([c + Vec2::new(5.0, -5.0), c + Vec2::new(-5.0, 5.0)], s);
            })
            .on_hover_text("Close")
            .clicked()
            {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        },
    );
}

/// One window control button; glyphs are painted so we don't depend on font coverage.
fn control(ui: &mut Ui, rect: Rect, id: &str, danger: bool, glyph: impl FnOnce(&egui::Painter, Pos2, Color32)) -> egui::Response {
    let pal = w::p(ui);
    let resp = ui.interact(rect, ui.scope_id().with(("titlebar-btn", id)), Sense::click());
    let (bg, fg) = match (resp.hovered(), danger) {
        (true, true) => (pal.danger_emphasis, Color32::WHITE),
        (true, false) => (pal.control_hover, pal.fg),
        _ => (Color32::TRANSPARENT, pal.fg_muted),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, bg);
    glyph(painter, rect.center(), fg);
    resp
}

/// Lets the undecorated window be resized from its edges and corners.
pub fn resize_edges(ctx: &egui::Context) {
    if cfg!(any(target_os = "macos", target_os = "android")) {
        return;
    }
    let (maximized, screen, pos, pressed) =
        ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.content_rect(), i.pointer.hover_pos(), i.pointer.primary_pressed()));
    if maximized {
        return;
    }
    let Some(p) = pos else { return };
    let (l, r) = (p.x - screen.left() <= EDGE, screen.right() - p.x <= EDGE);
    let (t, b) = (p.y - screen.top() <= EDGE, screen.bottom() - p.y <= EDGE);
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some((ResizeDirection::NorthWest, egui::CursorIcon::ResizeNorthWest)),
        (_, true, true, _) => Some((ResizeDirection::NorthEast, egui::CursorIcon::ResizeNorthEast)),
        (true, _, _, true) => Some((ResizeDirection::SouthWest, egui::CursorIcon::ResizeSouthWest)),
        (_, true, _, true) => Some((ResizeDirection::SouthEast, egui::CursorIcon::ResizeSouthEast)),
        (true, ..) => Some((ResizeDirection::West, egui::CursorIcon::ResizeWest)),
        (_, true, ..) => Some((ResizeDirection::East, egui::CursorIcon::ResizeEast)),
        (_, _, true, _) => Some((ResizeDirection::North, egui::CursorIcon::ResizeNorth)),
        (_, _, _, true) => Some((ResizeDirection::South, egui::CursorIcon::ResizeSouth)),
        _ => None,
    };
    if let Some((d, cursor)) = dir {
        ctx.set_cursor_icon(cursor);
        if pressed {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(d));
        }
    }
}

/// Without OS chrome the window needs its own outline so it doesn't blend into
/// whatever is behind it.
pub fn outline(ctx: &egui::Context) {
    if cfg!(any(target_os = "macos", target_os = "android")) || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let pal = crate::theme::pal(ctx);
    let rect = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("window-outline"))).rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, pal.border),
        egui::StrokeKind::Inside,
    );
}
