//! The waydlp visual identity for egui: true AMOLED black with purple
//! highlights. Shared by the desktop app and the graphical installer so the
//! whole product looks like one thing.

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Response, RichText, Shadow, Stroke, TextStyle, Ui, Vec2,
};

pub mod palette {
    use egui::Color32;
    pub const BG_PRIMARY: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
    pub const BG_SECONDARY: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
    pub const BG_TERTIARY: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
    pub const SURFACE_PRIMARY: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
    pub const SURFACE_SECONDARY: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(0x9D, 0x4E, 0xDD);
    pub const ACCENT_SECONDARY: Color32 = Color32::from_rgb(0x7B, 0x2C, 0xBF);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xB5, 0x6B, 0xFF);
    pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x5A, 0x18, 0x9A);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xA1, 0xA1, 0xAA);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x71, 0x71, 0x7A);
    pub const BORDER_PRIMARY: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
    pub const BORDER_ACCENT: Color32 = Color32::from_rgb(0x7B, 0x2C, 0xBF);
    pub const SUCCESS: Color32 = Color32::from_rgb(0x22, 0xC5, 0x5E);
    pub const WARNING: Color32 = Color32::from_rgb(0xEA, 0xB3, 0x08);
    pub const ERROR: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
}

use palette::*;

/// Accent colours derived from the user's chosen accent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accent {
    pub primary: Color32,
    pub hover: Color32,
    pub secondary: Color32,
    pub dark: Color32,
}

impl Accent {
    pub fn from_rgb(rgb: [u8; 3]) -> Self {
        if rgb == [0x9D, 0x4E, 0xDD] {
            return Self::default();
        }
        let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        Self {
            primary: c,
            hover: mix(c, Color32::WHITE, 0.25),
            secondary: mix(c, Color32::BLACK, 0.2),
            dark: mix(c, Color32::BLACK, 0.45),
        }
    }
}

impl Default for Accent {
    fn default() -> Self {
        Self { primary: ACCENT_PRIMARY, hover: ACCENT_HOVER, secondary: ACCENT_SECONDARY, dark: ACCENT_DARK }
    }
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Pick black or white text for legibility on `bg`.
pub fn on_color(bg: Color32) -> Color32 {
    let lum = 0.2126 * bg.r() as f32 + 0.7152 * bg.g() as f32 + 0.0722 * bg.b() as f32;
    if lum > 150.0 { Color32::BLACK } else { Color32::WHITE }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeOptions {
    pub accent: Accent,
    pub compact: bool,
    pub reduced_motion: bool,
    pub animations: bool,
}

impl Default for ThemeOptions {
    fn default() -> Self {
        Self { accent: Accent::default(), compact: false, reduced_motion: false, animations: true }
    }
}

pub const RADIUS: u8 = 10;
pub const RADIUS_LG: u8 = 14;

/// Apply the waydlp theme to a context.
pub fn apply(ctx: &egui::Context, opts: ThemeOptions) {
    ctx.set_theme(egui::Theme::Dark);
    let a = opts.accent;
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.dark_mode = true;
        v.override_text_color = None;
        v.panel_fill = BG_PRIMARY;
        v.window_fill = BG_SECONDARY;
        v.window_stroke = Stroke::new(1.0, BORDER_PRIMARY);
        v.window_corner_radius = CornerRadius::same(RADIUS_LG);
        v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(160) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(140) };
        v.menu_corner_radius = CornerRadius::same(RADIUS);
        v.faint_bg_color = BG_SECONDARY;
        v.extreme_bg_color = BG_TERTIARY;
        v.text_edit_bg_color = Some(BG_TERTIARY);
        v.code_bg_color = SURFACE_PRIMARY;
        v.hyperlink_color = a.hover;
        v.warn_fg_color = WARNING;
        v.error_fg_color = ERROR;
        v.selection.bg_fill = a.dark;
        v.selection.stroke = Stroke::new(1.0, a.hover);

        let r = CornerRadius::same(RADIUS);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = SURFACE_PRIMARY;
        w.noninteractive.weak_bg_fill = SURFACE_PRIMARY;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_PRIMARY);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
        w.noninteractive.corner_radius = r;

        w.inactive.bg_fill = SURFACE_SECONDARY;
        w.inactive.weak_bg_fill = SURFACE_PRIMARY;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER_PRIMARY);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
        w.inactive.corner_radius = r;

        w.hovered.bg_fill = mix(SURFACE_SECONDARY, a.primary, 0.18);
        w.hovered.weak_bg_fill = mix(SURFACE_PRIMARY, a.primary, 0.14);
        w.hovered.bg_stroke = Stroke::new(1.0, a.secondary);
        w.hovered.fg_stroke = Stroke::new(1.5, TEXT_PRIMARY);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 1.0;

        w.active.bg_fill = a.secondary;
        w.active.weak_bg_fill = a.dark;
        w.active.bg_stroke = Stroke::new(1.0, a.hover);
        w.active.fg_stroke = Stroke::new(2.0, TEXT_PRIMARY);
        w.active.corner_radius = r;
        w.active.expansion = 1.0;

        w.open.bg_fill = SURFACE_SECONDARY;
        w.open.weak_bg_fill = SURFACE_SECONDARY;
        w.open.bg_stroke = Stroke::new(1.0, a.secondary);
        w.open.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
        w.open.corner_radius = r;

        let s = &mut style.spacing;
        if opts.compact {
            s.item_spacing = Vec2::new(8.0, 6.0);
            s.button_padding = Vec2::new(10.0, 4.0);
            s.interact_size = Vec2::new(36.0, 24.0);
        } else {
            s.item_spacing = Vec2::new(10.0, 9.0);
            s.button_padding = Vec2::new(14.0, 7.0);
            s.interact_size = Vec2::new(40.0, 30.0);
        }
        s.window_margin = Margin::same(16);
        s.menu_margin = Margin::same(8);
        s.combo_width = 180.0;
        s.text_edit_width = 280.0;
        s.slider_width = 180.0;

        style.animation_time = if opts.reduced_motion || !opts.animations { 0.0 } else { 0.12 };

        style.text_styles = [
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional)),
            (TextStyle::Name("title".into()), FontId::new(32.0, FontFamily::Proportional)),
            (TextStyle::Name("subheading".into()), FontId::new(17.0, FontFamily::Proportional)),
        ]
        .into();
    });
}

/// A rounded surface card.
pub fn card() -> Frame {
    Frame::new()
        .fill(SURFACE_PRIMARY)
        .stroke(Stroke::new(1.0, BORDER_PRIMARY))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .inner_margin(Margin::same(16))
}

/// A card highlighted with the accent border (focus / primary areas).
pub fn accent_card(accent: Accent) -> Frame {
    card().stroke(Stroke::new(1.0, mix(accent.dark, BORDER_PRIMARY, 0.3)))
}

pub fn title(ui: &mut Ui, text: &str) -> Response {
    ui.label(RichText::new(text).size(30.0).color(TEXT_PRIMARY).strong())
}

pub fn heading(ui: &mut Ui, text: &str) -> Response {
    ui.label(RichText::new(text).heading().color(TEXT_PRIMARY).strong())
}

pub fn subheading(ui: &mut Ui, text: &str) -> Response {
    ui.label(RichText::new(text).size(17.0).color(TEXT_PRIMARY).strong())
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(TEXT_MUTED)
}

pub fn secondary(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(TEXT_SECONDARY)
}

/// Filled accent button for primary actions.
pub fn primary_button(ui: &mut Ui, accent: Accent, text: &str) -> Response {
    let fg = on_color(accent.primary);
    let btn = egui::Button::new(RichText::new(text).color(fg).strong())
        .fill(accent.primary)
        .stroke(Stroke::new(1.0, accent.hover))
        .corner_radius(CornerRadius::same(RADIUS));
    let r = ui.add(btn);
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r
}

/// Same as [`primary_button`] but disabled when `enabled` is false.
pub fn primary_button_enabled(ui: &mut Ui, accent: Accent, text: &str, enabled: bool) -> Response {
    ui.add_enabled_ui(enabled, |ui| primary_button(ui, accent, text)).inner
}

/// Subtle outlined button.
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(TEXT_PRIMARY))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, BORDER_PRIMARY)),
    )
}

/// Small coloured status label.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    Frame::new()
        .fill(mix(BG_PRIMARY, color, 0.18))
        .stroke(Stroke::new(1.0, mix(BG_PRIMARY, color, 0.55)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| ui.label(RichText::new(text).small().color(mix(color, Color32::WHITE, 0.25))))
        .response
}

/// A progress bar in the accent colour. `fraction = None` draws an
/// indeterminate (animated unless motion is reduced) bar; it never fakes a value.
pub fn progress_bar(
    ui: &mut Ui,
    accent: Accent,
    fraction: Option<f32>,
    text: Option<String>,
    animate: bool,
) -> Response {
    let height = 8.0;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    let r = CornerRadius::same(255);
    painter.rect_filled(rect, r, SURFACE_SECONDARY);
    match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            if f > 0.0 {
                let mut fill = rect;
                fill.set_width((rect.width() * f).max(height));
                painter.rect_filled(fill, r, accent.primary);
            }
        }
        None => {
            let seg = rect.width() * 0.25;
            let x = if animate {
                let t = ui.input(|i| i.time) as f32;
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                ((t * 0.6).fract()) * (rect.width() + seg) - seg
            } else {
                (rect.width() - seg) * 0.5
            };
            let left = (rect.left() + x).max(rect.left());
            let right = (rect.left() + x + seg).min(rect.right());
            if right > left {
                let fill = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(right, rect.bottom()));
                painter.rect_filled(fill, r, mix(accent.dark, accent.primary, 0.6));
            }
        }
    }
    if let Some(t) = text {
        ui.add_space(2.0);
        ui.label(RichText::new(t).small().color(TEXT_SECONDARY));
    }
    response
}

/// Friendly empty state with an icon glyph, title and hint.
pub fn empty_state(ui: &mut Ui, icon: &str, title_text: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(32.0);
        ui.label(RichText::new(icon).size(42.0).color(ACCENT_SECONDARY));
        ui.add_space(6.0);
        subheading(ui, title_text);
        ui.label(muted(hint));
        ui.add_space(24.0);
    });
}

/// Draw the waydlp logo mark (a stylised downward chevron in a rounded
/// square) with the painter, so it scales crisply at any size.
pub fn logo(ui: &mut Ui, size: f32, accent: Accent) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint_logo(ui.painter(), rect, accent);
    response
}

pub fn paint_logo(painter: &egui::Painter, rect: egui::Rect, accent: Accent) {
    let s = rect.width();
    painter.rect_filled(rect, CornerRadius::same((s * 0.26).min(255.0) as u8), accent.secondary);
    let inner = rect.shrink(s * 0.06);
    painter.rect_filled(inner, CornerRadius::same((s * 0.22).min(255.0) as u8), BG_PRIMARY);
    let c = rect.center();
    let w = s * 0.5;
    let stroke = Stroke::new((s * 0.1).max(1.5), accent.primary);
    // Wave-like "w" chevron pointing down (download), with a base line.
    let y0 = c.y - s * 0.16;
    let y1 = c.y + s * 0.10;
    let pts = [
        egui::pos2(c.x - w / 2.0, y0),
        egui::pos2(c.x - w / 4.0, y1),
        egui::pos2(c.x, y0 + s * 0.06),
        egui::pos2(c.x + w / 4.0, y1),
        egui::pos2(c.x + w / 2.0, y0),
    ];
    painter.add(egui::Shape::line(pts.to_vec(), stroke));
    let by = c.y + s * 0.24;
    painter.line_segment(
        [egui::pos2(c.x - w * 0.42, by), egui::pos2(c.x + w * 0.42, by)],
        Stroke::new((s * 0.07).max(1.0), accent.hover),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_accent_matches_palette() {
        assert_eq!(Accent::from_rgb([0x9D, 0x4E, 0xDD]), Accent::default());
    }

    #[test]
    fn contrast_helper() {
        assert_eq!(on_color(Color32::WHITE), Color32::BLACK);
        assert_eq!(on_color(ACCENT_PRIMARY), Color32::WHITE);
    }
}
