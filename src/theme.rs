//! Primer color tokens and the egui style that goes with them.
//!
//! Values follow GitHub's current functional tokens (light / dark default), so
//! when something looks off compared to github.com, this is the place to check.

use egui::{Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2};
use std::sync::Arc;

pub const SEMIBOLD: &str = "semibold";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub canvas: Color32,
    pub canvas_subtle: Color32,
    pub canvas_inset: Color32,
    pub header_bg: Color32,
    pub titlebar_bg: Color32,
    pub border: Color32,
    pub border_muted: Color32,
    pub fg: Color32,
    pub fg_muted: Color32,
    pub fg_subtle: Color32,
    pub fg_on_emphasis: Color32,
    pub accent: Color32,
    pub accent_emphasis: Color32,
    pub accent_subtle: Color32,
    pub success: Color32,
    pub success_emphasis: Color32,
    pub success_hover: Color32,
    pub success_subtle: Color32,
    pub danger: Color32,
    pub danger_emphasis: Color32,
    pub danger_subtle: Color32,
    pub done: Color32,
    pub done_emphasis: Color32,
    pub attention: Color32,
    pub attention_subtle: Color32,
    pub neutral_muted: Color32,
    pub neutral_emphasis: Color32,
    pub btn_bg: Color32,
    pub btn_hover: Color32,
    pub btn_active: Color32,
    pub btn_border: Color32,
    pub control_hover: Color32,
    pub underline_active: Color32,
    pub diff_add_bg: Color32,
    pub diff_add_num: Color32,
    pub diff_del_bg: Color32,
    pub diff_del_num: Color32,
    pub diff_hunk_bg: Color32,
    pub overlay_shadow: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const fn hexa(rgb: u32, a: u8) -> Color32 {
    // hmm, from_rgba_unmultiplied isn't const, so premultiply by hand
    let r = ((rgb >> 16) & 0xff) * a as u32 / 255;
    let g = ((rgb >> 8) & 0xff) * a as u32 / 255;
    let b = (rgb & 0xff) * a as u32 / 255;
    Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, a)
}

pub const LIGHT: Palette = Palette {
    dark: false,
    canvas: hex(0xffffff),
    canvas_subtle: hex(0xf6f8fa),
    canvas_inset: hex(0xf6f8fa),
    header_bg: hex(0xf6f8fa),
    titlebar_bg: hex(0xe6eaef),
    border: hex(0xd1d9e0),
    border_muted: hex(0xdee4ea),
    fg: hex(0x1f2328),
    fg_muted: hex(0x59636e),
    fg_subtle: hex(0x818b98),
    fg_on_emphasis: hex(0xffffff),
    accent: hex(0x0969da),
    accent_emphasis: hex(0x0969da),
    accent_subtle: hex(0xddf4ff),
    success: hex(0x1a7f37),
    success_emphasis: hex(0x1f883d),
    success_hover: hex(0x1c8139),
    success_subtle: hex(0xdafbe1),
    danger: hex(0xd1242f),
    danger_emphasis: hex(0xcf222e),
    danger_subtle: hex(0xffebe9),
    done: hex(0x8250df),
    done_emphasis: hex(0x8250df),
    attention: hex(0x9a6700),
    attention_subtle: hex(0xfff8c5),
    neutral_muted: hex(0xe6eaef),
    neutral_emphasis: hex(0x59636e),
    btn_bg: hex(0xf6f8fa),
    btn_hover: hex(0xeff2f5),
    btn_active: hex(0xe6eaef),
    btn_border: hex(0xd1d9e0),
    control_hover: hex(0xeef1f4),
    underline_active: hex(0xfd8c73),
    diff_add_bg: hex(0xdafbe1),
    diff_add_num: hex(0xaceebb),
    diff_del_bg: hex(0xffebe9),
    diff_del_num: hex(0xffcecb),
    diff_hunk_bg: hex(0xddf4ff),
    overlay_shadow: hexa(0x25292e, 40),
};

pub const DARK: Palette = Palette {
    dark: true,
    canvas: hex(0x0d1117),
    canvas_subtle: hex(0x151b23),
    canvas_inset: hex(0x010409),
    header_bg: hex(0x010409),
    titlebar_bg: hex(0x000000),
    border: hex(0x3d444d),
    border_muted: hex(0x2f353d),
    fg: hex(0xf0f6fc),
    fg_muted: hex(0x9198a1),
    fg_subtle: hex(0x656c76),
    fg_on_emphasis: hex(0xffffff),
    accent: hex(0x4493f8),
    accent_emphasis: hex(0x1f6feb),
    accent_subtle: hex(0x0e2240),
    success: hex(0x3fb950),
    success_emphasis: hex(0x238636),
    success_hover: hex(0x29903b),
    success_subtle: hex(0x12261e),
    danger: hex(0xf85149),
    danger_emphasis: hex(0xda3633),
    danger_subtle: hex(0x25171c),
    done: hex(0xab7df8),
    done_emphasis: hex(0x8957e5),
    attention: hex(0xd29922),
    attention_subtle: hex(0x272115),
    neutral_muted: hex(0x2a313b),
    neutral_emphasis: hex(0x656c76),
    btn_bg: hex(0x212830),
    btn_hover: hex(0x262c36),
    btn_active: hex(0x2a313c),
    btn_border: hex(0x3d444d),
    control_hover: hex(0x1f252d),
    underline_active: hex(0xf78166),
    diff_add_bg: hex(0x12261e),
    diff_add_num: hex(0x1b4721),
    diff_del_bg: hex(0x25171c),
    diff_del_num: hex(0x542426),
    diff_hunk_bg: hex(0x0e2240),
    overlay_shadow: hexa(0x010409, 200),
};

/// Palette for whatever theme egui is currently rendering with.
pub fn pal(ctx: &Context) -> &'static Palette {
    if ctx.global_style().visuals.dark_mode { &DARK } else { &LIGHT }
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Loads the system UI font when we can find it (GitHub itself uses the system
/// stack), falling back to the bundled Noto Sans.
pub fn install_fonts(ctx: &Context) {
    let mut defs = FontDefinitions::default();

    let regular = first_existing(REGULAR_CANDIDATES).unwrap_or_else(|| include_bytes!("../assets/fonts/NotoSans-Regular.ttf").to_vec());
    let bold = first_existing(SEMIBOLD_CANDIDATES).unwrap_or_else(|| include_bytes!("../assets/fonts/NotoSans-SemiBold.ttf").to_vec());
    let mono = first_existing(MONO_CANDIDATES).unwrap_or_else(|| include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf").to_vec());

    defs.font_data.insert("rivet-regular".into(), Arc::new(FontData::from_owned(regular)));
    defs.font_data.insert("rivet-semibold".into(), Arc::new(FontData::from_owned(bold)));
    defs.font_data.insert("rivet-mono".into(), Arc::new(FontData::from_owned(mono)));
    // monochrome emoji so READMEs with 👋 don't render as boxes; egui can't do color glyphs
    defs.font_data.insert("rivet-emoji".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf"))));

    // keep egui's defaults at the end of each chain, they carry emoji + symbols
    let fallbacks = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = vec!["rivet-regular".to_string(), "rivet-emoji".to_string()];
    prop.extend(fallbacks.iter().cloned());
    let mut semi = vec!["rivet-semibold".to_string(), "rivet-emoji".to_string()];
    semi.extend(fallbacks.iter().cloned());
    let mut monos = vec!["rivet-mono".to_string()];
    monos.extend(defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default());

    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(FontFamily::Name(SEMIBOLD.into()), semi);
    defs.families.insert(FontFamily::Monospace, monos);
    ctx.set_fonts(defs);
}

#[cfg(target_os = "windows")]
const REGULAR_CANDIDATES: &[&str] = &["C:\\Windows\\Fonts\\segoeui.ttf"];
#[cfg(target_os = "windows")]
const SEMIBOLD_CANDIDATES: &[&str] = &["C:\\Windows\\Fonts\\seguisb.ttf"];
#[cfg(target_os = "windows")]
const MONO_CANDIDATES: &[&str] = &["C:\\Windows\\Fonts\\CascadiaMono.ttf", "C:\\Windows\\Fonts\\consola.ttf"];

// yeah, SF is a variable font and we can't pick weights, so macOS gets the
// bundled Noto for semibold to keep headings looking right
#[cfg(target_os = "macos")]
const REGULAR_CANDIDATES: &[&str] = &[];
#[cfg(target_os = "macos")]
const SEMIBOLD_CANDIDATES: &[&str] = &[];
#[cfg(target_os = "macos")]
const MONO_CANDIDATES: &[&str] = &["/System/Library/Fonts/Menlo.ttc"];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const REGULAR_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SEMIBOLD_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/google-noto/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
    "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const MONO_CANDIDATES: &[&str] =
    &["/usr/share/fonts/google-noto/NotoSansMono-Regular.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"];

fn first_existing(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// Pushes the Primer look into both of egui's theme slots.
pub fn install_style(ctx: &Context) {
    ctx.set_style_of(egui::Theme::Light, style_for(&LIGHT));
    ctx.set_style_of(egui::Theme::Dark, style_for(&DARK));
}

pub fn apply_pref(ctx: &Context, pref: ThemePref) {
    ctx.options_mut(|o| {
        o.theme_preference = match pref {
            ThemePref::System => egui::ThemePreference::System,
            ThemePref::Light => egui::ThemePreference::Light,
            ThemePref::Dark => egui::ThemePreference::Dark,
        }
    });
}

fn style_for(p: &Palette) -> egui::Style {
    let mut style = egui::Style::default();
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, semibold(20.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 6.0);
    s.button_padding = Vec2::new(12.0, 5.0);
    // buttons draw their own 32px height; this only sizes checkboxes, labels and drags
    s.interact_size = Vec2::new(20.0, 20.0);
    s.menu_margin = Margin::same(8);
    s.window_margin = Margin::same(16);
    s.scroll.bar_width = 8.0;

    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.override_text_color = Some(p.fg);
    v.weak_text_color = Some(p.fg_muted);
    v.hyperlink_color = p.accent;
    v.panel_fill = p.canvas;
    v.window_fill = p.canvas;
    v.faint_bg_color = p.canvas_subtle;
    v.extreme_bg_color = p.canvas;
    v.text_edit_bg_color = Some(p.canvas);
    v.code_bg_color = p.neutral_muted;
    v.error_fg_color = p.danger;
    v.warn_fg_color = p.attention;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: p.overlay_shadow };
    v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: p.overlay_shadow };
    v.selection.bg_fill = if p.dark { hexa(0x1f6feb, 110) } else { hexa(0x0969da, 60) };
    v.selection.stroke = Stroke::new(2.0, p.accent_emphasis);
    v.striped = false;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;

    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.canvas;
    w.noninteractive.weak_bg_fill = p.canvas;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
    w.noninteractive.corner_radius = r;

    w.inactive.bg_fill = p.btn_bg;
    w.inactive.weak_bg_fill = p.btn_bg;
    w.inactive.bg_stroke = Stroke::new(1.0, p.btn_border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    w.inactive.corner_radius = r;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.btn_hover;
    w.hovered.weak_bg_fill = p.btn_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.btn_border);
    w.hovered.fg_stroke = Stroke::new(1.0, p.fg);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.btn_active;
    w.active.weak_bg_fill = p.btn_active;
    w.active.bg_stroke = Stroke::new(1.0, p.btn_border);
    w.active.fg_stroke = Stroke::new(1.0, p.fg);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;

    w.open = w.active;
    style.visuals = v;
    style
}
