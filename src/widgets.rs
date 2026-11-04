//! Primer components rebuilt on top of egui.
//!
//! Each helper mirrors a Primer React component (Button, CounterLabel, Label,
//! StateLabel, Box, UnderlineNav, Blankslate...) closely enough that screens can
//! be composed the same way GitHub's are.

use crate::icons::{Icon, icon, icon_sized};
use crate::task::Load;
use crate::theme::{Palette, pal, semibold};
use chrono::{DateTime, Utc};
use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, Response, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
    WidgetText,
};

pub fn p(ui: &Ui) -> &'static Palette {
    pal(ui.ctx())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Default,
    Primary,
    Danger,
    Invisible,
}

/// A Primer button. `icon` goes on the left like `leadingVisual`.
pub fn button(ui: &mut Ui, variant: Variant, icon: Option<Icon>, label: &str) -> Response {
    button_ex(ui, variant, icon, label, None, true)
}

pub fn button_ex(ui: &mut Ui, variant: Variant, leading: Option<Icon>, label: &str, trailing: Option<Icon>, enabled: bool) -> Response {
    let pal = p(ui);
    let font = if variant == Variant::Invisible { FontId::proportional(14.0) } else { semibold(14.0) };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
    let icon_w = |i: Option<Icon>| if i.is_some() { 16.0 + 8.0 } else { 0.0 };
    let pad_x = 12.0;
    let width =
        pad_x * 2.0 + galley.size().x + icon_w(leading) + icon_w(trailing) - if label.is_empty() && leading.is_some() { 8.0 } else { 0.0 };
    let content_w = width.max(32.0);
    // justified parents (vertical_centered_justified) get a full width button like Primer's `block`
    let full = ui.layout().horizontal_justify();
    let size = Vec2::new(if full { ui.available_width().max(content_w) } else { content_w }, 32.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let (fill, stroke, fg) = match variant {
            Variant::Default => (
                if pressed {
                    pal.btn_active
                } else if hovered {
                    pal.btn_hover
                } else {
                    pal.btn_bg
                },
                pal.btn_border,
                pal.fg,
            ),
            Variant::Primary => {
                (if hovered { pal.success_hover } else { pal.success_emphasis }, Color32::from_black_alpha(40), pal.fg_on_emphasis)
            }
            Variant::Danger => {
                if hovered {
                    (pal.danger_emphasis, pal.danger_emphasis, pal.fg_on_emphasis)
                } else {
                    (pal.btn_bg, pal.btn_border, pal.danger)
                }
            }
            Variant::Invisible => (
                if pressed {
                    pal.btn_active
                } else if hovered {
                    pal.control_hover
                } else {
                    Color32::TRANSPARENT
                },
                Color32::TRANSPARENT,
                if leading.is_some() && label.is_empty() { pal.fg_muted } else { pal.accent },
            ),
        };
        let alpha = if enabled { 1.0 } else { 0.5 };
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(6),
            fill.gamma_multiply(alpha),
            Stroke::new(1.0, stroke.gamma_multiply(alpha)),
            StrokeKind::Inside,
        );
        let mut x = rect.left() + pad_x + (rect.width() - content_w).max(0.0) / 2.0;
        if label.is_empty() && leading.is_some() {
            x = rect.center().x - 8.0;
        }
        let fg = fg.gamma_multiply(alpha);
        let icon_fg = if variant == Variant::Default || variant == Variant::Invisible && !label.is_empty() {
            pal.fg_muted.gamma_multiply(alpha)
        } else {
            fg
        };
        if let Some(i) = leading {
            let r = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 8.0), Vec2::splat(16.0));
            i.image(16.0, icon_fg).paint_at(ui, r);
            x += 24.0;
        }
        if !label.is_empty() {
            painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley.clone(), fg);
            x += galley.size().x + 8.0;
        }
        if let Some(i) = trailing {
            let r = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 8.0), Vec2::splat(16.0));
            i.image(16.0, icon_fg).paint_at(ui, r);
        }
    }
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Button with a counter pill after the label, like Watch / Fork / Star on repo pages.
pub fn button_counter(ui: &mut Ui, leading: Icon, label: &str, count: u64, caret: bool, enabled: bool) -> Response {
    let pal = p(ui);
    let font = semibold(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, pal.fg);
    let num = format_count(count);
    let num_g = ui.painter().layout_no_wrap(num, FontId::proportional(12.0), pal.fg);
    let pill_w = num_g.size().x + 12.0;
    let width = 12.0 + 16.0 + 8.0 + galley.size().x + 8.0 + pill_w + if caret { 8.0 + 16.0 } else { 0.0 } + 12.0;
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), sense);
    if ui.is_rect_visible(rect) {
        let fill = if enabled && resp.is_pointer_button_down_on() {
            pal.btn_active
        } else if enabled && resp.hovered() {
            pal.btn_hover
        } else {
            pal.btn_bg
        };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(6), fill, Stroke::new(1.0, pal.btn_border), StrokeKind::Inside);
        let mut x = rect.left() + 12.0;
        let cy = rect.center().y;
        leading.image(16.0, pal.fg_muted).paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, cy - 8.0), Vec2::splat(16.0)));
        x += 24.0;
        painter.galley(egui::pos2(x, cy - galley.size().y / 2.0), galley.clone(), pal.fg);
        x += galley.size().x + 8.0;
        let pill = egui::Rect::from_min_size(egui::pos2(x, cy - 9.0), Vec2::new(pill_w, 18.0));
        painter.rect_filled(pill, CornerRadius::same(255), pal.neutral_muted);
        painter.galley(pill.center() - num_g.size() / 2.0, num_g, pal.fg);
        x += pill_w + 8.0;
        if caret {
            Icon::TriangleDown
                .image(16.0, pal.fg_muted)
                .paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, cy - 8.0), Vec2::splat(16.0)));
        }
    }
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Square icon only button (IconButton in Primer).
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    button(ui, Variant::Default, Some(icon), "").on_hover_text(tooltip)
}

pub fn icon_button_invisible(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    button(ui, Variant::Invisible, Some(icon), "").on_hover_text(tooltip)
}

/// Painted pill: fixed height, measured up front, so it never stretches with
/// the row and wrapping layouts can break before it like any other widget.
pub struct Pill<'a> {
    pub text: &'a str,
    pub font: FontId,
    pub fg: Color32,
    pub fill: Color32,
    pub stroke: Color32,
    pub pad_x: f32,
    pub height: f32,
    pub radius: u8,
}

pub fn pill(ui: &mut Ui, p: Pill) -> Response {
    let galley = ui.painter().layout_no_wrap(p.text.to_owned(), p.font, p.fg);
    let size = Vec2::new(galley.size().x + p.pad_x * 2.0, p.height);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(p.radius), p.fill, Stroke::new(1.0, p.stroke), StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, p.fg);
    }
    resp
}

/// The grey pill with a number, like the ones next to tab names.
pub fn counter(ui: &mut Ui, n: u64) {
    let pal = p(ui);
    let text = format_count(n);
    pill(
        ui,
        Pill {
            text: &text,
            font: FontId::proportional(12.0),
            fg: pal.fg,
            fill: pal.neutral_muted,
            stroke: Color32::TRANSPARENT,
            pad_x: 6.0,
            height: 18.0,
            radius: 255,
        },
    );
}

pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0).replace(".0m", "m")
    } else if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0).replace(".0k", "k")
    } else {
        n.to_string()
    }
}

/// An issue label, colored the way GitHub does it in each theme.
pub fn issue_label(ui: &mut Ui, name: &str, hex: &str) -> Response {
    let pal = p(ui);
    let base = parse_hex(hex).unwrap_or(pal.neutral_emphasis);
    let (fill, stroke, fg) = if pal.dark {
        // github dark lightens the text and tints the background, close enough
        let fg = lighten(base, 0.35);
        (base.gamma_multiply(0.18), base.gamma_multiply(0.45), fg)
    } else {
        let lum = 0.299 * base.r() as f32 + 0.587 * base.g() as f32 + 0.114 * base.b() as f32;
        let fg = if lum > 150.0 { Color32::from_rgb(0x1f, 0x23, 0x28) } else { Color32::WHITE };
        (base, Color32::TRANSPARENT, fg)
    };
    pill(ui, Pill { text: name, font: semibold(12.0), fg, fill, stroke, pad_x: 8.0, height: 20.0, radius: 255 })
}

/// Bordered outline label (Primer `Label`), e.g. "Public", "Private", "Draft".
pub fn outline_label(ui: &mut Ui, text: &str, color: Option<Color32>) {
    let pal = p(ui);
    let c = color.unwrap_or(pal.fg_muted);
    let stroke = if color.is_some() { c } else { pal.border };
    pill(ui, Pill { text, font: semibold(12.0), fg: c, fill: Color32::TRANSPARENT, stroke, pad_x: 8.0, height: 20.0, radius: 255 });
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemState {
    Open,
    Closed,
    ClosedNotPlanned,
    Merged,
    Draft,
    PrClosed,
}

impl ItemState {
    pub fn colors(self, pal: &Palette) -> (Color32, Icon, &'static str) {
        match self {
            ItemState::Open => (pal.success_emphasis, Icon::IssueOpened, "Open"),
            ItemState::Closed => (pal.done_emphasis, Icon::IssueClosed, "Closed"),
            ItemState::ClosedNotPlanned => (pal.neutral_emphasis, Icon::Skip, "Closed"),
            ItemState::Merged => (pal.done_emphasis, Icon::GitMerge, "Merged"),
            ItemState::Draft => (pal.neutral_emphasis, Icon::GitPullRequestDraft, "Draft"),
            ItemState::PrClosed => (pal.danger_emphasis, Icon::GitPullRequestClosed, "Closed"),
        }
    }

    /// The small icon used in lists, colored with the fg variant.
    pub fn list_icon(self, pal: &Palette, is_pr: bool) -> (Icon, Color32) {
        match self {
            ItemState::Open if is_pr => (Icon::GitPullRequest, pal.success),
            ItemState::Open => (Icon::IssueOpened, pal.success),
            ItemState::Closed => (Icon::IssueClosed, pal.done),
            ItemState::ClosedNotPlanned => (Icon::Skip, pal.fg_muted),
            ItemState::Merged => (Icon::GitMerge, pal.done),
            ItemState::Draft => (Icon::GitPullRequestDraft, pal.fg_muted),
            ItemState::PrClosed => (Icon::GitPullRequestClosed, pal.danger),
        }
    }
}

/// The big rounded "Open" / "Merged" badge on issue and PR pages.
pub fn state_label(ui: &mut Ui, state: ItemState, is_pr: bool) {
    let pal = p(ui);
    let (fill, mut ic, text) = state.colors(pal);
    if is_pr && state == ItemState::Open {
        ic = Icon::GitPullRequest;
    }
    Frame::new().fill(fill).corner_radius(CornerRadius::same(255)).inner_margin(Margin::symmetric(12, 5)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.horizontal(|ui| {
            icon(ui, ic, pal.fg_on_emphasis);
            ui.label(RichText::new(text).font(semibold(14.0)).color(pal.fg_on_emphasis));
        });
    });
}

/// Primer `Box` with a border, rounded corners and optional header row.
pub fn bordered<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = p(ui);
    Frame::new()
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .fill(pal.canvas)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Grey header strip on top of a bordered box.
pub fn box_header<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = p(ui);
    let r = Frame::new()
        .fill(pal.canvas_subtle)
        .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| add(ui)).inner
        })
        .inner;
    divider(ui);
    r
}

/// One row inside a bordered list, highlights on hover and is clickable.
pub fn list_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let inner = Frame::new()
        .fill(if hovered { pal.canvas_subtle } else { Color32::TRANSPARENT })
        .inner_margin(Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.interact(inner.response.rect, id, Sense::click())
}

pub fn divider(ui: &mut Ui) {
    let pal = p(ui);
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().top();
    ui.painter().hline(rect.x_range(), y, Stroke::new(1.0, pal.border));
    ui.add_space(1.0);
}

/// Underline tabs like the repo nav (Code, Issues, Pull requests...).
/// Returns the index of a clicked tab.
pub fn underline_nav(ui: &mut Ui, items: &[(Icon, &str, Option<u64>)], selected: usize) -> Option<usize> {
    let pal = p(ui);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (i, (ic, name, count)) in items.iter().enumerate() {
            let is_sel = i == selected;
            let font = if is_sel { semibold(14.0) } else { FontId::proportional(14.0) };
            let galley = ui.painter().layout_no_wrap(name.to_string(), font, pal.fg);
            let count_text = count.filter(|c| *c > 0).map(format_count);
            let count_w = count_text
                .as_ref()
                .map(|t| ui.painter().layout_no_wrap(t.clone(), FontId::proportional(12.0), pal.fg).size().x + 12.0 + 8.0)
                .unwrap_or(0.0);
            let w = 8.0 + 16.0 + 8.0 + galley.size().x + count_w + 8.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 48.0), Sense::click());
            let inner = egui::Rect::from_center_size(rect.center(), Vec2::new(w, 32.0));
            if resp.hovered() {
                ui.painter().rect_filled(inner, CornerRadius::same(6), pal.control_hover);
            }
            let mut x = inner.left() + 8.0;
            ic.image(16.0, pal.fg_muted).paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, inner.center().y - 8.0), Vec2::splat(16.0)));
            x += 24.0;
            ui.painter().galley(egui::pos2(x, inner.center().y - galley.size().y / 2.0), galley.clone(), pal.fg);
            x += galley.size().x + 8.0;
            if let Some(t) = count_text {
                let g = ui.painter().layout_no_wrap(t, FontId::proportional(12.0), pal.fg);
                let pill = egui::Rect::from_min_size(egui::pos2(x, inner.center().y - 9.0), Vec2::new(g.size().x + 12.0, 18.0));
                ui.painter().rect_filled(pill, CornerRadius::same(255), pal.neutral_muted);
                ui.painter().galley(pill.center() - g.size() / 2.0, g, pal.fg);
            }
            if is_sel {
                let bar = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 2.0), egui::pos2(rect.right(), rect.bottom()));
                ui.painter().rect_filled(bar, CornerRadius::same(6), pal.underline_active);
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// Segmented "Open / Closed" style toggle used above issue lists.
pub fn tab_link(ui: &mut Ui, icon_: Icon, text: &str, selected: bool) -> Response {
    let pal = p(ui);
    let color = if selected { pal.fg } else { pal.fg_muted };
    let font = if selected { semibold(14.0) } else { FontId::proportional(14.0) };
    let r = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            icon(ui, icon_, color);
            ui.label(RichText::new(text).font(font).color(color));
        })
        .response;
    ui.interact(r.rect, ui.scope_id().with(("tablink", text)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Accent colored link that underlines on hover.
pub fn link(ui: &mut Ui, text: impl Into<String>) -> Response {
    link_styled(ui, text.into(), None, false)
}

pub fn link_strong(ui: &mut Ui, text: impl Into<String>, size: f32) -> Response {
    link_styled(ui, text.into(), Some(size), true)
}

/// Regular weight link in the default text color, accent + underline on hover.
/// GitHub uses this for file names and sidebar repo lists.
pub fn plain_link(ui: &mut Ui, text: &str) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let mut rt = RichText::new(text).size(14.0).color(if hovered { pal.accent } else { pal.fg });
    if hovered {
        rt = rt.underline();
    }
    let r = ui.add(egui::Label::new(rt).sense(Sense::click()).truncate());
    ui.interact(r.rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Default-colored text that turns into an accent link on hover, like titles in lists.
pub fn title_link(ui: &mut Ui, text: &str, size: f32) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let mut rt = RichText::new(text).font(semibold(size)).color(if hovered { pal.accent } else { pal.fg });
    if hovered {
        rt = rt.underline();
    }
    let r = ui.add(egui::Label::new(rt).sense(Sense::click()).truncate());
    ui.interact(r.rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn link_styled(ui: &mut Ui, text: String, size: Option<f32>, strong: bool) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let size = size.unwrap_or(14.0);
    let font = if strong { semibold(size) } else { FontId::proportional(size) };
    let mut rt = RichText::new(text).font(font).color(pal.accent);
    if hovered {
        rt = rt.underline();
    }
    let r = ui.add(egui::Label::new(rt).sense(Sense::click()));
    ui.interact(r.rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) -> Response {
    let pal = p(ui);
    ui.label(RichText::new(text.into()).color(pal.fg_muted))
}

pub fn muted_small(ui: &mut Ui, text: impl Into<String>) -> Response {
    let pal = p(ui);
    ui.label(RichText::new(text.into()).size(12.0).color(pal.fg_muted))
}

pub fn heading(ui: &mut Ui, text: impl Into<String>, size: f32) -> Response {
    ui.label(RichText::new(text.into()).font(semibold(size)))
}

/// Primer TextInput: 32px tall, 6px radius, blue focus ring.
pub fn text_input(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    text_input_ex(ui, value, hint, width, None)
}

pub fn text_input_ex(ui: &mut Ui, value: &mut String, hint: &str, width: f32, leading: Option<Icon>) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let focused = ui.ctx().memory(|m| m.has_focus(id.with("te")));
    let stroke = if focused { Stroke::new(2.0, pal.accent_emphasis) } else { Stroke::new(1.0, pal.border) };
    let out = Frame::new()
        .fill(pal.canvas)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 0 })
        .show(ui, |ui| {
            // the stroke sits outside the margin, leaving it out grows resizable panels every frame
            ui.set_width(width - 24.0 - stroke.width * 2.0);
            ui.set_height(30.0);
            ui.horizontal_centered(|ui| {
                if let Some(i) = leading {
                    icon(ui, i, pal.fg_muted);
                }
                ui.add(
                    TextEdit::singleline(value)
                        .id(id.with("te"))
                        .hint_text(RichText::new(hint).color(pal.fg_subtle))
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .align(egui::Align2::LEFT_CENTER),
                )
            })
            .inner
        });
    out.inner
}

/// Multiline variant used by comment boxes and editors.
pub fn text_area(ui: &mut Ui, value: &mut String, hint: &str, rows: usize, mono: bool) -> Response {
    let pal = p(ui);
    let id = ui.next_auto_id();
    let focused = ui.ctx().memory(|m| m.has_focus(id.with("ta")));
    let stroke = if focused { Stroke::new(2.0, pal.accent_emphasis) } else { Stroke::new(1.0, pal.border) };
    Frame::new()
        .fill(pal.canvas)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            let mut te = TextEdit::multiline(value)
                .id(id.with("ta"))
                .hint_text(RichText::new(hint).color(pal.fg_subtle))
                .frame(egui::Frame::NONE)
                .desired_rows(rows)
                .desired_width(f32::INFINITY);
            if mono {
                te = te.code_editor();
            }
            ui.add(te)
        })
        .inner
}

/// Form label above an input, GitHub style (semibold 14px).
pub fn form_label(ui: &mut Ui, text: &str, required: bool) {
    let pal = p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(RichText::new(text).font(semibold(14.0)));
        if required {
            ui.label(RichText::new("*").color(pal.danger));
        }
    });
}

/// Round avatar loaded from GitHub's CDN.
pub fn avatar(ui: &mut Ui, url: &str, size: f32) -> Response {
    let pal = p(ui);
    if url.is_empty() {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
        ui.painter().circle_filled(rect.center(), size / 2.0, pal.neutral_muted);
        return resp;
    }
    let sized = avatar_url(url, size);
    let resp = ui.add(
        egui::Image::new(sized)
            .fit_to_exact_size(Vec2::splat(size))
            .corner_radius(CornerRadius::same((size / 2.0).min(255.0) as u8))
            .sense(Sense::click()),
    );
    ui.painter().circle_stroke(resp.rect.center(), size / 2.0, Stroke::new(1.0, pal.border_muted));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Square-ish avatar for organizations (GitHub rounds orgs at 6px).
pub fn org_avatar(ui: &mut Ui, url: &str, size: f32) -> Response {
    ui.add(
        egui::Image::new(avatar_url(url, size))
            .fit_to_exact_size(Vec2::splat(size))
            .corner_radius(CornerRadius::same(6))
            .sense(Sense::click()),
    )
}

fn avatar_url(url: &str, size: f32) -> String {
    // ask for 2x so it stays crisp on hidpi screens
    let px = (size * 2.0) as u32;
    if url.contains('?') { format!("{url}&s={px}") } else { format!("{url}?s={px}") }
}

pub fn spinner(ui: &mut Ui) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.add(egui::Spinner::new().size(32.0).color(p(ui).fg_muted));
    });
    ui.add_space(24.0);
}

/// Empty state with an icon, title and short description.
pub fn blankslate(ui: &mut Ui, icon_: Icon, title: &str, desc: &str) {
    let pal = p(ui);
    ui.add_space(32.0);
    ui.vertical_centered(|ui| {
        icon_sized(ui, icon_, 24.0, pal.fg_muted);
        ui.add_space(8.0);
        ui.label(RichText::new(title).font(semibold(20.0)));
        if !desc.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(desc).color(pal.fg_muted));
        }
    });
    ui.add_space(32.0);
}

#[derive(Clone, Copy)]
pub enum FlashKind {
    Info,
    Warning,
    Danger,
    Success,
}

/// Primer Flash banner.
pub fn flash(ui: &mut Ui, kind: FlashKind, text: &str) {
    let pal = p(ui);
    let (bg, border, ic, fg) = match kind {
        FlashKind::Info => (pal.accent_subtle, pal.accent.gamma_multiply(0.4), Icon::Info, pal.accent),
        FlashKind::Warning => (pal.attention_subtle, pal.attention.gamma_multiply(0.4), Icon::Alert, pal.attention),
        FlashKind::Danger => (pal.danger_subtle, pal.danger.gamma_multiply(0.4), Icon::Alert, pal.danger),
        FlashKind::Success => (pal.success_subtle, pal.success.gamma_multiply(0.4), Icon::CheckCircle, pal.success),
    };
    Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                icon(ui, ic, fg);
                ui.label(RichText::new(text).color(pal.fg));
            });
        });
}

/// Renders a `Load` the usual way: spinner while loading, flash on error.
pub fn show_load<T: Send + 'static, R>(ui: &mut Ui, load: &mut Load<T>, add: impl FnOnce(&mut Ui, &mut T) -> R) -> Option<R> {
    match load.poll() {
        Load::Idle | Load::Loading(_) => {
            spinner(ui);
            None
        }
        Load::Failed(e) => {
            let e = e.clone();
            flash(ui, FlashKind::Danger, &e);
            None
        }
        Load::Ready(v) => Some(add(ui, v)),
    }
}

/// "3 hours ago", same buckets as GitHub's relative-time element.
pub fn rel_time(t: &DateTime<Utc>) -> String {
    let secs = (Utc::now() - *t).num_seconds();
    let (n, unit) = match secs {
        s if s < 45 => return "now".into(),
        s if s < 90 => return "1 minute ago".into(),
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86_400 => (s / 3600, "hour"),
        s if s < 86_400 * 2 => return "yesterday".into(),
        s if s < 86_400 * 30 => (s / 86_400, "day"),
        _ => return format!("on {}", t.format("%b %-d, %Y")),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

pub fn parse_hex(hex: &str) -> Option<Color32> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn lighten(c: Color32, t: f32) -> Color32 {
    let f = |x: u8| (x as f32 + (255.0 - x as f32) * t) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// GitHub's language colors for the handful of languages people see daily.
pub fn language_color(lang: &str) -> Color32 {
    let hex = match lang {
        "Rust" => "dea584",
        "TypeScript" => "3178c6",
        "JavaScript" => "f1e05a",
        "Python" => "3572A5",
        "Go" => "00ADD8",
        "Java" => "b07219",
        "Kotlin" => "A97BFF",
        "C" => "555555",
        "C++" => "f34b7d",
        "C#" => "178600",
        "Ruby" => "701516",
        "PHP" => "4F5D95",
        "Swift" => "F05138",
        "Shell" => "89e051",
        "HTML" => "e34c26",
        "CSS" => "663399",
        "SCSS" => "c6538c",
        "Vue" => "41b883",
        "Dart" => "00B4AB",
        "Lua" => "000080",
        "Luau" => "00A2FF",
        "GDScript" => "355570",
        "Zig" => "ec915c",
        "Nix" => "7e7eff",
        "Dockerfile" => "384d54",
        "Makefile" => "427819",
        "Haskell" => "5e5086",
        "Elixir" => "6e4a7e",
        "Scala" => "c22d40",
        "Jupyter Notebook" => "DA5B0B",
        "MDX" => "fcb32c",
        "Svelte" => "ff3e00",
        "Astro" => "ff5a03",
        "Objective-C" => "438eff",
        "PowerShell" => "012456",
        "TeX" => "3D6117",
        _ => "8b949e",
    };
    parse_hex(hex).unwrap_or(Color32::GRAY)
}

pub fn language_dot(ui: &mut Ui, lang: &str) {
    let pal = p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 6.0, language_color(lang));
        ui.label(RichText::new(lang).size(12.0).color(pal.fg_muted));
    });
}

/// Small icon + muted text, the meta bits under repo names ("★ 120").
pub fn meta(ui: &mut Ui, icon_: Icon, text: impl Into<WidgetText>) -> Response {
    let pal = p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        icon(ui, icon_, pal.fg_muted);
        ui.label(text);
    })
    .response
}

/// Right-aligned area in a horizontal row.
pub fn right<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    // hmm, a plain right_to_left layout grabs all remaining height, which blows up
    // rows inside scroll areas, so cap it to what the row already uses
    let h = ui.min_rect().height().max(ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), h), Layout::right_to_left(Align::Center), add).inner
}

/// Centers page content at GitHub's max width (1280px container-xl).
pub fn page<R>(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let w = avail.min(max_width);
    let side = ((avail - w) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(side);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        })
        .inner
    })
    .inner
}

/// Popup menu item with optional icon, GitHub ActionList style.
pub fn menu_item(ui: &mut Ui, icon_: Option<Icon>, text: &str) -> Response {
    menu_item_ex(ui, icon_, text, false, false)
}

pub fn menu_item_ex(ui: &mut Ui, icon_: Option<Icon>, text: &str, selected: bool, danger: bool) -> Response {
    let pal = p(ui);
    let w = ui.available_width().max(180.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), if danger { pal.danger_subtle } else { pal.control_hover });
    }
    let fg = if danger { pal.danger } else { pal.fg };
    let mut x = rect.left() + 8.0;
    if selected {
        Icon::Check.image(16.0, fg).paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 8.0), Vec2::splat(16.0)));
        x += 24.0;
    }
    if let Some(i) = icon_ {
        i.image(16.0, if danger { pal.danger } else { pal.fg_muted })
            .paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 8.0), Vec2::splat(16.0)));
        x += 24.0;
    }
    let g = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(14.0), fg);
    ui.painter().galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Code-ish inline pill, used for branch names ("main").
pub fn branch_pill(ui: &mut Ui, name: &str) {
    let pal = p(ui);
    Frame::new()
        .fill(pal.accent_subtle)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| ui.add(egui::Label::new(RichText::new(name).monospace().size(12.0).color(pal.accent)).extend()));
}
