//! Home tab in the ArtCraft layout (structure ported from PdfCraft's home.rs via ModelCraft,
//! MIT OR Apache-2.0, © ArtCraft Team and PdfCraft contributors): header, actions, recents.

use crate::app::{Action, App};
use crate::theme::{self, Press, Tokens};
use crate::{icons, widgets};
use egui::{vec2, Align2, CornerRadius, Rect, Sense, Stroke};
use std::path::PathBuf;
use crate::i18n::{tr, trf};

fn card_lift(ui: &egui::Ui, t: &Tokens, rect: Rect, press: Press) {
    let lift = press.hover * (1.0 - press.down);
    if lift > 0.04 {
        let shadow = egui::Shadow { offset: [0, 3], blur: (14.0 * lift) as u8, spread: 0, color: t.shadow.gamma_multiply(0.85 * lift) };
        ui.painter().add(shadow.as_shape(rect, CornerRadius::same(10)));
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.pasteboard)).show_inside(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin { left: 36, right: 36, top: 28, bottom: 28 }).show(ui, |ui| {
                header(ui, &t);
                ui.add_space(22.0);
                actions(app, ui, &t);
                ui.add_space(26.0);
                recents(app, ui, &t);
                ui.add_space(20.0);
                widgets::section_title(ui, tr("Confidentialité"));
                ui.label(
                    egui::RichText::new(
                        tr("MailCraft fonctionne hors ligne. Comme Outlook, il bloque les images distantes et n'exécute aucun script : ouvrir un message ne prévient jamais son expéditeur. Aucune télémétrie, vos messages ne quittent pas votre ordinateur."),
                    )
                    .color(t.text_muted),
                );
            });
        });
    });
}

fn header(ui: &mut egui::Ui, t: &Tokens) {
    egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::app_icon(ui, 56.0);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(egui::RichText::new(tr("Bienvenue dans ")).font(theme::semibold(24.0)));
                    widgets::wordmark(ui, 24.0);
                });
                ui.label(egui::RichText::new(tr("Lisez vos messages Outlook (.msg) comme dans votre messagerie : expéditeur, destinataires, mise en forme, images intégrées et pièces jointes, sans Outlook.")).color(t.text_muted).font(theme::regular(14.0)));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    ui.label(widgets::mono_caps(&format!("V{}", crate::branding::VERSION), 11.0, t.accent_text));
                    ui.label(egui::RichText::new(trf("par {a}", &[("a", crate::branding::AUTHOR)])).font(theme::mono(11.0)).color(t.text_faint));
                });
            });
        });
    });
}

struct Tile {
    icon: &'static str,
    title: &'static str,
    blurb: &'static str,
    hue: egui::Color32,
    action: fn() -> Action,
}

fn actions(app: &mut App, ui: &mut egui::Ui, t: &Tokens) {
    let tiles = [
        Tile { icon: "mail", title: "Ouvrir un message", blurb: "Un ou plusieurs fichiers .msg, ouverts chacun dans un onglet", hue: egui::Color32::from_rgb(0x3E, 0x8E, 0xDE), action: || Action::OpenFileDialog },
        Tile { icon: "inbox", title: "Ouvrir un dossier", blurb: "Tous les .msg d'un dossier, en boîte de réception triée par date", hue: egui::Color32::from_rgb(0xE0, 0x8A, 0x2E), action: || Action::OpenFolderDialog },
    ];
    let gap = 14.0;
    let avail = ui.available_width();
    let cols = if avail > 560.0 { 2 } else { 1 };
    let w = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    for row in tiles.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            for c in row {
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 92.0), Sense::click());
                let press = Press::track(ui, &resp);
                card_lift(ui, t, rect, press);
                let body = rect.translate(press.offset());
                ui.painter().rect(rect, CornerRadius::same(10), press.fill(t.card, t.hover, t.pressed), Stroke::new(1.0, t.divider), egui::StrokeKind::Inside);
                let badge = Rect::from_min_size(body.min + vec2(16.0, 18.0), vec2(40.0, 40.0));
                ui.painter().rect_filled(badge, CornerRadius::same(10), c.hue.gamma_multiply(if t.dark() { 0.28 } else { 0.14 }));
                icons::paint(ui, badge, c.icon, 22.0, c.hue);
                ui.painter().text(body.min + vec2(72.0, 30.0), Align2::LEFT_CENTER, tr(c.title), theme::semibold(14.5), t.text);
                widgets::row_text(ui, body.min + vec2(72.0, 52.0), tr(c.blurb), theme::regular(12.0), t.text_muted, body.width() - 90.0);
                ui.painter().text(body.left_bottom() + vec2(72.0, -14.0), Align2::LEFT_CENTER, tr("Parcourir…"), theme::medium(12.0), t.accent_text);
                if theme::hand(resp).clicked() {
                    app.actions.push((c.action)());
                }
            }
        });
        ui.add_space(gap);
    }
    ui.label(egui::RichText::new(tr("Astuce : glissez des fichiers .msg ou un dossier n'importe où dans la fenêtre.")).font(theme::regular(12.0)).color(t.text_faint));
}

fn recents(app: &mut App, ui: &mut egui::Ui, t: &Tokens) {
    ui.label(egui::RichText::new(tr("Récents")).font(theme::semibold(17.0)));
    ui.add_space(8.0);
    if app.settings.recent.is_empty() {
        ui.label(egui::RichText::new(tr("Les messages et dossiers que vous ouvrez apparaîtront ici.")).color(t.text_muted));
        return;
    }
    for s in app.settings.recent.clone() {
        let p = PathBuf::from(&s);
        let is_dir = p.is_dir();
        let exists = is_dir || p.is_file();
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
        let press = Press::track(ui, &resp);
        press.wash(ui, rect, 8, false);
        let body = rect.translate(press.offset());
        icons::paint(ui, Rect::from_min_size(body.min + vec2(10.0, 11.0), vec2(22.0, 22.0)), if is_dir { "inbox" } else { "mail" }, 20.0, if exists { t.accent } else { t.text_faint });
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.clone());
        widgets::row_text(ui, body.min + vec2(44.0, 15.0), name, theme::medium(13.5), if exists { t.text } else { t.text_faint }, body.width() - 160.0);
        let parent = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        widgets::row_text(ui, body.min + vec2(44.0, 31.0), parent, theme::regular(11.0), t.text_faint, body.width() - 160.0);
        ui.painter().text(
            body.right_center() - vec2(14.0, 0.0),
            Align2::RIGHT_CENTER,
            if !exists { tr("introuvable") } else if is_dir { tr("dossier") } else { tr("message") },
            theme::regular(12.0),
            if exists { t.text_faint } else { t.warning },
        );
        let resp = theme::hand(resp).on_hover_text(&s);
        if resp.clicked() && exists {
            app.actions.push(Action::Open(p.clone()));
        }
        resp.context_menu(|ui| {
            if widgets::menu_icon_item(ui, "external-link", tr("Afficher dans l'explorateur"), "").clicked() {
                app.actions.push(Action::Reveal(p.clone()));
                ui.close();
            }
            if widgets::menu_icon_item(ui, "trash-2", tr("Retirer de la liste"), "").clicked() {
                app.actions.push(Action::ForgetRecent(s.clone()));
                ui.close();
            }
        });
    }
}
