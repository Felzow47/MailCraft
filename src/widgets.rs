//! Small custom widgets built on the design tokens. Metrics follow PdfCraft's `widgets.rs`
//! (© ArtCraft Team and the PdfCraft contributors, MIT OR Apache-2.0).

use crate::icons;
use crate::theme::{self, Tokens};
use egui::{vec2, Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke};

pub const DIALOG_PAD_X: i8 = 20;
pub const DIALOG_PAD_Y: i8 = 16;

pub fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    let t = Tokens::get(ctx);
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(DIALOG_PAD_X, DIALOG_PAD_Y))
        .shadow(ctx.global_style().visuals.window_shadow)
}

/// Outlined controls inside dialogs (the card fill would swallow them otherwise).
pub fn begin_dialog(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border);
    w.inactive.weak_bg_fill = t.field;
    w.inactive.bg_fill = t.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, t.text_muted);
    for state in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.corner_radius = CornerRadius::same(4);
    }
    ui.visuals_mut().selection.stroke = Stroke::new(1.5, t.accent);
    ui.spacing_mut().interact_size.y = 30.0;
}

pub fn text_width(ui: &egui::Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x)
}

/// Rounded pill button; `primary` fills with the accent.
pub fn pill_button(ui: &mut egui::Ui, label: &str, primary: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(12.5);
    let w = text_width(ui, label, &font);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 26.0, 28.0), Sense::click());
    let press = theme::Press::track(ui, &resp);
    let (fill, stroke, text) = if primary {
        (press.fill(t.accent, t.accent_text, t.accent.lerp_to_gamma(Color32::BLACK, 0.22)), Stroke::NONE, t.on_accent())
    } else {
        (press.fill(t.card, t.hover, t.pressed), Stroke::new(1.0, t.border), t.text)
    };
    ui.painter().rect(rect, CornerRadius::same(14), fill, stroke, egui::StrokeKind::Inside);
    ui.painter().text(rect.translate(press.offset()).center(), Align2::CENTER_CENTER, label, font, text);
    theme::hand(resp)
}

/// Icon + label, transparent until hovered.
pub fn ghost_button(ui: &mut egui::Ui, icon: &str, label: &str) -> Response {
    ghost_button_sized(ui, icon, label, 30.0)
}

pub fn ghost_button_sized(ui: &mut egui::Ui, icon: &str, label: &str, height: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(13.0);
    let w = text_width(ui, label, &font);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 38.0, height), Sense::click());
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 6, false);
    let body = rect.translate(press.offset());
    icons::paint(ui, Rect::from_min_size(body.min + vec2(6.0, (height - 18.0) / 2.0), vec2(18.0, 18.0)), icon, 17.0, t.icon);
    ui.painter().text(body.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, label, font, t.text);
    theme::hand(resp)
}

/// Menu entry with a leading icon.
pub fn menu_icon_item(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let img = icons::image(icon, 15.0, t.icon);
    ui.add(egui::Button::image_and_text(img, label).shortcut_text(shortcut))
}

pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(10.0);
    ui.label(egui::RichText::new(text.to_uppercase()).font(theme::semibold(10.5)).color(t.text_faint).extra_letter_spacing(0.6));
    ui.add_space(2.0);
}

/// ArtCraft-launcher style label: monospace, upper case, spaced out.
pub fn mono_caps(text: &str, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(text.to_uppercase()).font(theme::mono(size)).color(color).extra_letter_spacing(1.2)
}

/// One line cut with "…" to `max_width`, vertically centred on `pos`.
pub fn row_text(ui: &egui::Ui, pos: egui::Pos2, text: impl Into<String>, font: egui::FontId, color: Color32, max_width: f32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.into(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
    let galley = ui.painter().layout_job(job);
    let rect = Align2::LEFT_CENTER.anchor_size(pos, galley.size());
    ui.painter().galley(rect.min, galley, color);
}

/// iOS-style switch (used in the settings window).
pub fn switch(ui: &mut egui::Ui, on: &mut bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, mut resp) = ui.allocate_exact_size(vec2(34.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = ui.ctx().animate_bool_with_time(resp.id, *on, 0.12);
    let track = t.border.lerp_to_gamma(t.accent, k);
    ui.painter().rect_filled(rect, CornerRadius::same(10), track);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, k);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    theme::hand(resp)
}

/// Segmented control: one of `options`, returns true when changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(12.5);
    let widths: Vec<f32> = options.iter().map(|(_, l)| text_width(ui, l, &font) + 22.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 28.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(7), t.field, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    let mut changed = false;
    for ((v, label), w) in options.iter().zip(widths) {
        let r = Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), vec2(w, 24.0));
        let resp = ui.interact(r, ui.id().with(("seg", label)), Sense::click());
        let sel = *value == *v;
        let press = theme::Press::track(ui, &resp);
        if sel {
            ui.painter().rect_filled(r, CornerRadius::same(5), t.accent_soft);
            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, t.accent.gamma_multiply(0.5)), egui::StrokeKind::Inside);
        } else {
            press.wash(ui, r, 5, false);
        }
        ui.painter().text(r.center() + press.offset(), Align2::CENTER_CENTER, *label, font.clone(), if sel { t.accent_text } else { t.text_muted });
        if theme::hand(resp).clicked() && !sel {
            *value = *v;
            changed = true;
        }
        x += w;
    }
    changed
}

/// The two-tone product name: "Model" in the text colour, "Craft" in the accent.
pub fn wordmark(ui: &mut egui::Ui, size: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let mut job = egui::text::LayoutJob::default();
    job.append(crate::branding::NAME_HEAD, 0.0, egui::TextFormat { font_id: theme::semibold(size), color: t.text, ..Default::default() });
    job.append(crate::branding::NAME_TAIL, 0.0, egui::TextFormat { font_id: theme::semibold(size), color: t.accent, ..Default::default() });
    ui.label(job)
}

/// The app icon (the SVG logo) at `size` points.
pub fn app_icon(ui: &mut egui::Ui, size: f32) -> Response {
    ui.add(egui::Image::from_bytes("bytes://mailcraft-logo.svg", crate::branding::ICON_SVG).fit_to_exact_size(vec2(size, size)))
}

/// Indeterminate spinner, Material style: an arc that grows and shrinks while it turns, over a
/// faint track. Keeps repainting while it is drawn.
pub fn paint_spinner(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32) {
    use std::f32::consts::PI;
    let ctx = painter.ctx();
    let t = ctx.input(|i| i.time) as f32;
    let width = (radius * 0.2).max(2.0);
    painter.circle_stroke(center, radius, Stroke::new(width, color.gamma_multiply(0.16)));
    const CYCLE: f32 = 1.3;
    const SPAN: f32 = 1.5 * PI;
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let c = (t / CYCLE).fract();
    let base = (t / CYCLE).floor() * SPAN + t * 2.4 - PI / 2.0;
    let head = base + ease(c / 0.6) * SPAN;
    let tail = base + ease((c - 0.4) / 0.6) * SPAN;
    let (a0, a1) = (tail, head.max(tail + 0.12));
    let n = ((a1 - a0) * radius / 2.0).ceil().clamp(6.0, 96.0) as usize;
    let at = |a: f32| center + vec2(a.cos(), a.sin()) * radius;
    let pts: Vec<egui::Pos2> = (0..=n).map(|i| at(a0 + (a1 - a0) * i as f32 / n as f32)).collect();
    painter.add(egui::epaint::PathShape::line(pts, Stroke::new(width, color)));
    // Round caps.
    painter.circle_filled(at(a0), width / 2.0, color);
    painter.circle_filled(at(a1), width / 2.0, color);
    ctx.request_repaint();
}

/// A spinner widget of `size` points in the accent colour.
pub fn spinner(ui: &mut egui::Ui, size: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_spinner(ui.painter(), rect.center(), size / 2.0 - 1.0, t.accent);
    resp
}

/// Big centred spinner with a caption, for panes that are loading.
pub fn loading(ui: &egui::Ui, rect: Rect, caption: &str) {
    let t = Tokens::get(ui.ctx());
    let c = rect.center() - vec2(0.0, 14.0);
    paint_spinner(ui.painter(), c, 15.0, t.accent);
    if !caption.is_empty() {
        ui.painter().text(c + vec2(0.0, 34.0), Align2::CENTER_CENTER, caption, theme::regular(12.5), t.text_muted);
    }
}
