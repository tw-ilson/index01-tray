//! libadwaita-like look for egui: Adwaita Sans, GNOME palette, system dark mode and accent colour,
//! plus preference groups ("boxed lists"), rows and toggle switches.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Sense,
    Stroke, StrokeKind, TextStyle, Ui, Vec2,
};
use std::process::Command;

fn gsetting(key: &str) -> String {
    Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", key])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_owned()
        })
        .unwrap_or_default()
}

pub fn prefers_dark() -> bool {
    match gsetting("color-scheme").as_str() {
        "prefer-dark" => true,
        "prefer-light" | "default" => false,
        _ => gsetting("gtk-theme").to_lowercase().contains("dark"),
    }
}

/// GNOME 47+ accent colours.
pub fn accent() -> Color32 {
    match gsetting("accent-color").as_str() {
        "teal" => Color32::from_rgb(0x21, 0x90, 0xa4),
        "green" => Color32::from_rgb(0x3a, 0x94, 0x4a),
        "yellow" => Color32::from_rgb(0xc8, 0x88, 0x00),
        "orange" => Color32::from_rgb(0xed, 0x5b, 0x00),
        "red" => Color32::from_rgb(0xe6, 0x2d, 0x42),
        "pink" => Color32::from_rgb(0xd5, 0x61, 0x99),
        "purple" => Color32::from_rgb(0x91, 0x41, 0xac),
        "slate" => Color32::from_rgb(0x6f, 0x83, 0x96),
        _ => Color32::from_rgb(0x35, 0x84, 0xe4),
    }
}

pub const SUCCESS: Color32 = Color32::from_rgb(0x2e, 0xc2, 0x7e);
pub const WARNING: Color32 = Color32::from_rgb(0xe5, 0xa5, 0x0a);
pub const ERROR: Color32 = Color32::from_rgb(0xe0, 0x1b, 0x24);

static PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();

#[derive(Clone, Copy)]
pub struct Palette {
    pub window: Color32,
    pub card: Color32,
    pub card_border: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub entry: Color32,
    pub fg: Color32,
    pub dim: Color32,
    pub accent: Color32,
}

impl Palette {
    pub fn get(ctx: &egui::Context) -> Self {
        let _ = ctx;
        *PALETTE.get_or_init(|| Self::new(prefers_dark(), accent()))
    }

    fn new(dark: bool, accent: Color32) -> Self {
        if dark {
            Palette {
                window: Color32::from_rgb(0x22, 0x22, 0x26),
                card: Color32::from_rgb(0x2e, 0x2e, 0x32),
                card_border: Color32::from_rgb(0x1a, 0x1a, 0x1e),
                control: Color32::from_rgb(0x3a, 0x3a, 0x3e),
                control_hover: Color32::from_rgb(0x45, 0x45, 0x49),
                entry: Color32::from_rgb(0x38, 0x38, 0x3c),
                fg: Color32::from_rgb(0xff, 0xff, 0xff),
                dim: Color32::from_rgb(0x9a, 0x9a, 0x9e),
                accent,
            }
        } else {
            Palette {
                window: Color32::from_rgb(0xfa, 0xfa, 0xfb),
                card: Color32::WHITE,
                card_border: Color32::from_rgb(0xe0, 0xe0, 0xe4),
                control: Color32::from_rgb(0xe8, 0xe8, 0xea),
                control_hover: Color32::from_rgb(0xdd, 0xdd, 0xe0),
                entry: Color32::from_rgb(0xec, 0xec, 0xee),
                fg: Color32::from_rgb(0x1e, 0x1e, 0x22),
                dim: Color32::from_rgb(0x6e, 0x6e, 0x72),
                accent,
            }
        }
    }
}

fn load_font(fonts: &mut FontDefinitions, family: &str, name: &str) -> bool {
    let Ok(o) = Command::new("fc-match")
        .args([family, "-f", "%{file}"])
        .output()
    else {
        return false;
    };
    let path = String::from_utf8_lossy(&o.stdout).to_string();
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    fonts
        .font_data
        .insert(name.into(), FontData::from_owned(bytes).into());
    true
}

pub fn apply(ctx: &egui::Context) {
    // Fonts: the desktop's UI font first, egui's built-ins as fallback for symbols.
    let mut fonts = FontDefinitions::default();
    for (fam, family, name) in [
        (FontFamily::Proportional, "Adwaita Sans:style=Regular", "ui"),
        (FontFamily::Monospace, "Adwaita Mono:style=Regular", "mono"),
    ] {
        if load_font(&mut fonts, family, name) {
            fonts
                .families
                .entry(fam)
                .or_default()
                .insert(0, name.into());
        }
    }
    if load_font(&mut fonts, "Adwaita Sans:style=Bold", "ui-bold") {
        fonts.families.insert(
            FontFamily::Name("bold".into()),
            vec!["ui-bold".into(), "ui".into()],
        );
    } else {
        let p = fonts.families[&FontFamily::Proportional].clone();
        fonts.families.insert(FontFamily::Name("bold".into()), p);
    }
    ctx.set_fonts(fonts);

    let dark = prefers_dark();
    let p = Palette::new(dark, accent());
    let _ = PALETTE.set(p);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(14.5)),
            (TextStyle::Button, FontId::proportional(14.5)),
            (TextStyle::Monospace, FontId::monospace(13.5)),
            (
                TextStyle::Heading,
                FontId::new(15.0, FontFamily::Name("bold".into())),
            ),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(8.0, 6.0);
        s.button_padding = Vec2::new(14.0, 7.0);
        s.interact_size.y = 34.0;
        s.combo_height = 300.0;
        s.menu_margin = Margin::same(6);

        let v = &mut style.visuals;
        v.dark_mode = dark;
        v.override_text_color = Some(p.fg);
        v.panel_fill = p.window;
        v.window_fill = p.card;
        v.window_stroke = Stroke::new(1.0, p.card_border);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = p.entry;
        v.text_edit_bg_color = Some(p.entry);
        v.faint_bg_color = p.card;
        v.selection.bg_fill = p.accent.gamma_multiply(0.45);
        v.selection.stroke = Stroke::new(1.0, p.fg);
        v.hyperlink_color = p.accent;
        v.window_shadow = egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(60),
        };
        v.popup_shadow = v.window_shadow;
        v.text_cursor.stroke = Stroke::new(2.0, p.accent);

        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.card_border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
        w.noninteractive.corner_radius = r;
        for (ws, fill) in [
            (&mut w.inactive, p.control),
            (&mut w.hovered, p.control_hover),
            (&mut w.active, p.control_hover),
            (&mut w.open, p.control_hover),
        ] {
            ws.bg_fill = fill;
            ws.weak_bg_fill = fill;
            ws.bg_stroke = Stroke::NONE;
            ws.fg_stroke = Stroke::new(1.5, p.fg);
            ws.corner_radius = r;
            ws.expansion = 0.0;
        }
        // Text entries get the accent focus ring.
        w.active.bg_stroke = Stroke::new(2.0, p.accent.gamma_multiply(0.6));
    });
}

/// A libadwaita preferences group: bold title, then a rounded card of rows.
pub fn group<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::get(ui.ctx());
    ui.add_space(14.0);
    ui.label(egui::RichText::new(title).heading());
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.card_border))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(14, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            body(ui)
        })
        .inner
}

/// One row: title (and optional subtitle) on the left, the control on the right.
pub fn row<R>(
    ui: &mut Ui,
    first: bool,
    title: &str,
    subtitle: Option<&str>,
    control: impl FnOnce(&mut Ui) -> R,
) -> R {
    let p = Palette::get(ui.ctx());
    if !first {
        let y = ui.cursor().top();
        let x = ui.max_rect().x_range();
        ui.painter().hline(
            egui::Rangef::new(x.min - 14.0, x.max + 14.0),
            y,
            Stroke::new(1.0, p.card_border),
        );
    }
    let h = if subtitle.is_some() { 58.0 } else { 50.0 };
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_height(h);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let text_h = if subtitle.is_some() { 37.0 } else { 19.0 };
                ui.add_space(((h - text_h) / 2.0).max(0.0));
                ui.label(title);
                if let Some(s) = subtitle {
                    ui.label(egui::RichText::new(s).small().color(p.dim));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                control(ui)
            })
            .inner
        },
    )
    .inner
}

/// GTK-style toggle switch.
pub fn switch(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let p = Palette::get(ui.ctx());
    let size = Vec2::new(46.0, 26.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let off = if resp.hovered() {
        p.control_hover
    } else {
        p.control
    };
    let track = egui::lerp(egui::Rgba::from(off)..=egui::Rgba::from(p.accent), t);
    ui.painter().rect(
        rect,
        13.0,
        Color32::from(track),
        Stroke::NONE,
        StrokeKind::Inside,
    );
    let x = egui::lerp(rect.left() + 13.0..=rect.right() - 13.0, t);
    ui.painter().circle(
        egui::pos2(x, rect.center().y),
        10.0,
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_black_alpha(25)),
    );
    resp
}

/// Accent-filled ("suggested-action") or red ("destructive-action") pill button.
pub fn action_button(ui: &mut Ui, text: &str, color: Color32, enabled: bool) -> egui::Response {
    let b = egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
        .fill(color)
        .corner_radius(CornerRadius::same(17))
        .min_size(Vec2::new(90.0, 34.0));
    ui.add_enabled(enabled, b)
}

pub fn entry(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width)
            .margin(Margin::symmetric(10, 8)),
    )
}
