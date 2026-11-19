//! Window chrome in the ArtCraft layout (ported from PdfCraft's chrome.rs via ModelCraft,
//! MIT OR Apache-2.0, © ArtCraft Team and PdfCraft contributors): tab strip, toolbar, status bar.

use crate::app::{Action, App, Dialog, ReaderState, TabKind};
use crate::fmt::{self, Reply};
use crate::theme::{self, Press, Tokens};
use crate::{icons, widgets};
use egui::{vec2, Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke};
use crate::i18n::{tr, trf};

const TAB_STRIP: f32 = 38.0;
const TAB_HEIGHT: f32 = 30.0;
const STRIP_BUTTON: f32 = 28.0;
const TAB_CHROME: f32 = 64.0;
const TAB_MAX: f32 = 240.0;

pub fn tab_strip(app: &mut App, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("tab_strip")
        .exact_size(TAB_STRIP)
        .frame(egui::Frame::NONE.fill(t.titlebar).inner_margin(egui::Margin { left: 8, right: 10, top: 0, bottom: 0 }))
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icons::button(ui, "house", STRIP_BUTTON, app.active.is_none(), tr("Accueil")).clicked() {
                    app.active = None;
                }
                let right_w = 2.0 * STRIP_BUTTON + 16.0;
                let open_w = widgets::text_width(ui, tr("Ouvrir"), &theme::medium(13.0)) + 38.0 + 12.0;
                let budget = (ui.available_width() - right_w - open_w).max(0.0);
                let mut close = None;
                let mut activate = None;
                egui::ScrollArea::horizontal().id_salt("doc_tabs").max_width(budget).auto_shrink([true, true]).show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        for i in 0..app.tabs.len() {
                            let tab = &app.tabs[i];
                            let (icon, hover, path) = match &tab.kind {
                                TabKind::Folder(f) => (if f.rx.is_some() { "loader-circle" } else { "inbox" }, f.path.display().to_string(), f.path.clone()),
                                TabKind::Message(r) => (if matches!(r.state, ReaderState::Loading(_)) { "loader-circle" } else { "mail" }, r.path.display().to_string(), r.path.clone()),
                            };
                            let resp = tab_button(ui, &t, icon, &tab.title, &hover, app.active == Some(i), &mut close, i);
                            if resp.clicked() {
                                activate = Some(i);
                            }
                            if resp.middle_clicked() {
                                close = Some(i);
                            }
                            resp.context_menu(|ui| {
                                if widgets::menu_icon_item(ui, "x", tr("Fermer l'onglet"), "Ctrl+W").clicked() {
                                    close = Some(i);
                                    ui.close();
                                }
                                ui.separator();
                                if widgets::menu_icon_item(ui, "external-link", tr("Afficher dans l'explorateur"), "").clicked() {
                                    app.actions.push(Action::Reveal(path.clone()));
                                    ui.close();
                                }
                                if widgets::menu_icon_item(ui, "copy", tr("Copier le chemin"), "").clicked() {
                                    app.actions.push(Action::CopyPath(path.clone()));
                                    ui.close();
                                }
                            });
                        }
                    });
                });
                if widgets::ghost_button_sized(ui, "plus", tr("Ouvrir"), STRIP_BUTTON).on_hover_text(tr("Ouvrir un message (Ctrl+O)")).clicked() {
                    app.actions.push(Action::OpenFileDialog);
                }
                if let Some(i) = activate {
                    app.active = Some(i);
                }
                if let Some(i) = close {
                    app.actions.push(Action::CloseTab(i));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if icons::button(ui, "circle-help", STRIP_BUTTON, false, tr("Raccourcis clavier (F1)")).clicked() {
                        app.dialog = Some(Dialog::Shortcuts);
                    }
                    let dark = t.dark();
                    let r = icons::button(ui, if dark { "sun" } else { "moon" }, STRIP_BUTTON, false, if dark { tr("Thème clair") } else { tr("Thème sombre") });
                    if r.clicked() {
                        app.toggle_theme(ui.ctx());
                    }
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn tab_button(ui: &mut egui::Ui, t: &Tokens, icon: &str, name: &str, hover: &str, active: bool, close: &mut Option<usize>, index: usize) -> egui::Response {
    let font = theme::regular(13.0);
    let mut label = name.to_string();
    let room = TAB_MAX - TAB_CHROME;
    if widgets::text_width(ui, &label, &font) > room {
        while label.chars().count() > 4 && widgets::text_width(ui, &format!("{label}…"), &font) > room {
            label.pop();
        }
        label.push('…');
    }
    let text_w = widgets::text_width(ui, &label, &font);
    let width = text_w + TAB_CHROME;
    let height = ui.available_height().max(TAB_HEIGHT);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let press = Press::track(ui, &resp);
    let bg = if active { press.fill(t.chrome, t.chrome.lerp_to_gamma(t.hover, 0.65), t.hover) } else { press.fill(Color32::TRANSPARENT, t.hover, t.pressed) };
    let body = Rect::from_min_max(rect.min + vec2(0.0, height - TAB_HEIGHT), rect.max);
    if bg.a() > 0 {
        ui.painter().rect_filled(body, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, bg);
    }
    let icon_rect = Rect::from_min_size(body.min + vec2(8.0, (TAB_HEIGHT - 16.0) / 2.0), vec2(16.0, 16.0));
    if icon == "loader-circle" {
        widgets::paint_spinner(ui.painter(), icon_rect.center(), 6.0, t.accent);
    } else {
        icons::paint(ui, icon_rect, icon, 15.0, if active { t.accent } else { t.text_muted });
    }
    ui.painter().text(body.min + vec2(30.0, TAB_HEIGHT / 2.0), Align2::LEFT_CENTER, label, font, if active { t.text } else { t.text_muted });
    let x_rect = Rect::from_center_size(body.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
    let x = ui.interact(x_rect, resp.id.with("close"), Sense::click());
    let xp = Press::track(ui, &x);
    xp.wash(ui, x_rect, 4, false);
    if active || resp.hovered() || x.hovered() {
        icons::paint(ui, x_rect.translate(xp.offset()), "x", 13.0, t.text_muted);
    }
    if theme::hand(x).on_hover_text(tr("Fermer l'onglet")).clicked() {
        *close = Some(index);
    }
    theme::hand(resp).on_hover_text(hover)
}

/// Outlook's tr("Accueil") ribbon, flattened into the ArtCraft mode bar.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("toolbar")
        .exact_size(48.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, t.divider)))
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                main_menu(app, ui);
                ui.add_space(6.0);
                ui.painter().vline(ui.cursor().left(), ui.max_rect().y_range().shrink(12.0), Stroke::new(1.0, t.divider));
                ui.add_space(8.0);
                if widgets::ghost_button(ui, "mail", tr("Ouvrir")).on_hover_text(tr("Ouvrir un message (Ctrl+O)")).clicked() {
                    app.actions.push(Action::OpenFileDialog);
                }
                if widgets::ghost_button(ui, "inbox", tr("Dossier")).on_hover_text(tr("Ouvrir un dossier de messages (Ctrl+Maj+O)")).clicked() {
                    app.actions.push(Action::OpenFolderDialog);
                }
                ui.add_space(6.0);
                ui.painter().vline(ui.cursor().left(), ui.max_rect().y_range().shrink(12.0), Stroke::new(1.0, t.divider));
                ui.add_space(8.0);
                let has_msg = app.message().is_some();
                ui.add_enabled_ui(has_msg, |ui| {
                    if widgets::ghost_button(ui, "reply", tr("Répondre")).on_hover_text(tr("Répondre (Ctrl+R)")).clicked() {
                        app.actions.push(Action::Reply(Reply::Reply));
                    }
                    if widgets::ghost_button(ui, "reply-all", tr("Répondre à tous")).on_hover_text(tr("Répondre à tous (Ctrl+Maj+R)")).clicked() {
                        app.actions.push(Action::Reply(Reply::ReplyAll));
                    }
                    if widgets::ghost_button(ui, "forward", tr("Transférer")).on_hover_text(tr("Transférer (Ctrl+Maj+F)")).clicked() {
                        app.actions.push(Action::Reply(Reply::Forward));
                    }
                    ui.add_space(6.0);
                    ui.painter().vline(ui.cursor().left(), ui.max_rect().y_range().shrink(12.0), Stroke::new(1.0, t.divider));
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, "printer", tr("Imprimer")).on_hover_text(tr("Imprimer (Ctrl+P)")).clicked() {
                        app.actions.push(Action::Print);
                    }
                    if widgets::ghost_button(ui, "code", tr("En-têtes")).on_hover_text(tr("En-têtes Internet du message")).clicked() {
                        app.actions.push(Action::Headers);
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if icons::button(ui, "settings", 32.0, matches!(app.dialog, Some(Dialog::Settings)), tr("Paramètres")).clicked() {
                        app.dialog = Some(Dialog::Settings);
                    }
                    ui.add_enabled_ui(has_msg, |ui| {
                        if icons::button(ui, "zoom-in", 32.0, false, tr("Zoom avant (Ctrl++)")).clicked() {
                            app.actions.push(Action::Zoom(1.1));
                        }
                        let z = format!("{:.0} %", app.body.zoom_level() * 100.0);
                        if ui.add(egui::Button::new(egui::RichText::new(z).font(theme::medium(12.0)).color(t.text_muted)).frame(false)).on_hover_text(tr("Réinitialiser le zoom (Ctrl+0)")).clicked() {
                            app.actions.push(Action::Zoom(0.0));
                        }
                        if icons::button(ui, "zoom-out", 32.0, false, tr("Zoom arrière (Ctrl+-)")).clicked() {
                            app.actions.push(Action::Zoom(1.0 / 1.1));
                        }
                    });
                });
            });
        });
}

fn main_menu(app: &mut App, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(13.0);
    let w = widgets::text_width(ui, tr("Menu"), &font) + 46.0;
    let (rect, r) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&r));
    let press = Press::track(ui, &r);
    press.wash(ui, rect, 6, open);
    icons::paint(ui, Rect::from_min_size(rect.min + vec2(8.0, 7.0), vec2(16.0, 16.0)), "list", 15.0, t.icon);
    ui.painter().text(rect.left_center() + vec2(30.0, 0.0) + press.offset(), Align2::LEFT_CENTER, tr("Menu"), font, t.text);
    let r = theme::hand(r);
    let has_msg = app.message().is_some();
    let has_folder = app.export.is_none() && matches!(app.active.and_then(|i| app.tabs.get(i)).map(|t| &t.kind), Some(TabKind::Folder(f)) if !f.view.is_empty());
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(270.0);
        let mut item = |ui: &mut egui::Ui, icon: &str, label: &str, key: &str, enabled: bool, a: fn() -> Action| {
            if ui.add_enabled_ui(enabled, |ui| widgets::menu_icon_item(ui, icon, label, key)).inner.clicked() {
                app.actions.push(a());
                ui.close();
            }
        };
        item(ui, "mail", tr("Ouvrir un message…"), "Ctrl+O", true, || Action::OpenFileDialog);
        item(ui, "inbox", tr("Ouvrir un dossier…"), tr("Ctrl+Maj+O"), true, || Action::OpenFolderDialog);
        item(ui, "house", tr("Accueil"), "", true, || Action::Home);
        ui.separator();
        item(ui, "reply", tr("Répondre"), "Ctrl+R", has_msg, || Action::Reply(Reply::Reply));
        item(ui, "reply-all", tr("Répondre à tous"), tr("Ctrl+Maj+R"), has_msg, || Action::Reply(Reply::ReplyAll));
        item(ui, "forward", tr("Transférer"), tr("Ctrl+Maj+F"), has_msg, || Action::Reply(Reply::Forward));
        ui.separator();
        item(ui, "save", tr("Enregistrer les pièces jointes…"), "", has_msg, || Action::SaveAll);
        item(ui, "images", tr("Exporter tous les médias du message…"), "", has_msg, || Action::ExportMedia);
        item(ui, "folder-down", tr("Exporter les médias du dossier…"), "", has_folder, || Action::ExportFolderMedia);
        item(ui, "printer", tr("Imprimer…"), "Ctrl+P", has_msg, || Action::Print);
        item(ui, "code", tr("En-têtes Internet"), "", has_msg, || Action::Headers);
        item(ui, "table-2", tr("Propriétés MAPI"), "", has_msg, || Action::Properties);
        ui.separator();
        if widgets::menu_icon_item(ui, "settings", tr("Paramètres"), "").clicked() {
            app.dialog = Some(Dialog::Settings);
            ui.close();
        }
        if widgets::menu_icon_item(ui, "keyboard", tr("Raccourcis clavier"), "F1").clicked() {
            app.dialog = Some(Dialog::Shortcuts);
            ui.close();
        }
        if widgets::menu_icon_item(ui, "info", tr("À propos de MailCraft"), "").clicked() {
            app.dialog = Some(Dialog::About);
            ui.close();
        }
    });
}

pub fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("status_bar")
        .exact_size(26.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(12, 0)).stroke(Stroke::new(1.0, t.divider)))
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let small = |s: String| egui::RichText::new(s).font(theme::regular(11.5)).color(t.text_muted);
                if let Some((_, done, total)) = &app.export {
                    widgets::spinner(ui, 13.0);
                    ui.label(small(trf("Export des médias… {d} / {n}", &[("d", &fmt::int(*done)), ("n", &fmt::int(*total))])));
                }
                match app.active.and_then(|i| app.tabs.get(i)) {
                    None => {
                        ui.label(small(tr("Glissez un fichier .msg ou un dossier dans la fenêtre").into()));
                    }
                    Some(tab) => {
                        if let TabKind::Folder(f) = &tab.kind {
                            if f.rx.is_some() {
                                widgets::spinner(ui, 13.0);
                                ui.label(small(trf("Lecture des messages… {d} / {n}", &[("d", &fmt::int(f.items.len())), ("n", &fmt::int(f.total))])));
                            } else {
                                ui.label(small(trf("Éléments : {n}", &[("n", &fmt::int(f.items.len()))])));
                                if f.view.len() != f.items.len() {
                                    ui.label(small(trf("Affichés : {n}", &[("n", &fmt::int(f.view.len()))])));
                                }
                                let unread = f.items.iter().filter(|s| s.unread).count();
                                if unread > 0 {
                                    ui.label(small(trf("Non lus : {n}", &[("n", &fmt::int(unread))])));
                                }
                                if !f.failed.is_empty() {
                                    let r = ui.label(egui::RichText::new(trf("{n} fichier(s) illisible(s)", &[("n", &f.failed.len().to_string())])).font(theme::regular(11.5)).color(t.warning));
                                    let list: Vec<String> = f.failed.iter().take(20).map(|(p, e)| format!("{} : {e}", p.file_name().unwrap_or_default().to_string_lossy())).collect();
                                    r.on_hover_text(list.join("\n"));
                                }
                            }
                        }
                        if let Some(r) = tab.reader() {
                            if let Some(m) = r.msg() {
                                let kind = match m.body_kind {
                                    crate::msg::BodyKind::Html => "Format HTML",
                                    crate::msg::BodyKind::RtfHtml => "Format HTML (extrait du RTF)",
                                    crate::msg::BodyKind::Text => "Texte brut",
                                    crate::msg::BodyKind::HtmlFile => "Page HTML (pas un message Outlook)",
                                    crate::msg::BodyKind::Empty => "Message vide",
                                };
                                ui.label(small(tr(kind).into()));
                                if r.embedded {
                                    ui.label(small(tr("Message joint").into()));
                                }
                                if let Ok(md) = std::fs::metadata(&r.path) {
                                    if md.is_file() && !r.embedded {
                                        ui.label(small(fmt::human_size(md.len())));
                                    }
                                }
                            }
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(widgets::mono_caps(&format!("V{}", crate::branding::VERSION), 10.5, t.text_faint));
                    ui.label(egui::RichText::new(tr("Hors ligne · contenu distant bloqué")).font(theme::regular(11.5)).color(t.text_faint));
                });
            });
        });
}

pub fn toast(app: &mut App, ctx: &egui::Context) {
    let Some((text, age, error)) = &mut app.toast else { return };
    *age += ctx.input(|i| i.stable_dt).min(0.1) as f64;
    let error = *error;
    if *age > if error { 7.0 } else { 3.5 } {
        app.toast = None;
        return;
    }
    let t = Tokens::get(ctx);
    let text = text.clone();
    // Over the toolbar: the message page is a native window and would hide a toast drawn on it.
    let mut close = false;
    egui::Area::new(egui::Id::new("toast")).anchor(Align2::CENTER_TOP, vec2(0.0, 46.0)).order(egui::Order::Tooltip).show(ctx, |ui| {
        let edge = if error { t.danger } else { t.border };
        egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, edge)).corner_radius(CornerRadius::same(18)).inner_margin(egui::Margin::symmetric(16, 9)).shadow(ctx.global_style().visuals.popup_shadow).show(ui, |ui| {
            ui.set_max_width(640.0);
            ui.horizontal(|ui| {
                if error {
                    ui.add(icons::image("triangle-alert", 15.0, t.danger));
                }
                ui.add(egui::Label::new(egui::RichText::new(text).font(theme::medium(12.5))).wrap());
                if error && icons::button(ui, "x", 22.0, false, tr("Fermer")).clicked() {
                    close = true;
                }
            });
        });
    });
    if close {
        app.toast = None;
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

pub fn drop_overlay(ctx: &egui::Context) {
    if ctx.input(|i| i.raw.hovered_files.is_empty()) {
        return;
    }
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
    p.rect_filled(screen, 0, t.pasteboard.gamma_multiply(0.92));
    let r = screen.shrink(24.0);
    p.rect_stroke(r, 14, Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
    p.text(r.center(), Align2::CENTER_CENTER, tr("Déposez vos messages ou un dossier"), theme::semibold(20.0), t.text);
}
