//! The mail client itself: message list (folder tabs) and reading pane.

use crate::app::{Action, App, Filter, Folder, Reader, ReaderState, TabKind};
use crate::fmt::{self, Reply};
use crate::msg::{AttachData, Attachment, BodyKind, Message, Recipient};
use crate::theme::{self, Press, Tokens};
use crate::{icons, widgets};
use egui::{vec2, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, Sense, Stroke};
use crate::i18n::{tr, trf};

const ROW_H: f32 = 70.0;
const ROW_H_COMPACT: f32 = 52.0;
const GROUP_H: f32 = 30.0;

/// Draws tab `i`; returns where the message body (web page) goes.
pub fn show(app: &mut App, ui: &mut egui::Ui, i: usize) -> Option<Rect> {
    let t = Tokens::get(ui.ctx());
    let mut actions = Vec::new();
    let mut body = None;
    let preview = app.settings.preview_lines;
    let search_focus = std::mem::take(&mut app.search_focus);
    let mut list_width = app.settings.list_width;
    let opening: Vec<(u64, usize)> = app.opening.iter().map(|o| (o.reader, o.index)).collect();
    match &mut app.tabs[i].kind {
        TabKind::Folder(f) => {
            let resp = egui::Panel::left("message_list")
                .default_size(list_width)
                .size_range(300.0..=720.0)
                .resizable(true)
                .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.divider)))
                .show_inside(ui, |ui| message_list(ui, &t, f, preview, search_focus, &mut actions));
            let w = resp.response.rect.width();
            if (w - list_width).abs() > 1.0 && !ui.ctx().input(|i| i.pointer.any_down()) {
                list_width = w;
            }
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.panel)).show_inside(ui, |ui| match &f.reader {
                Some(r) => body = reading_pane(ui, &t, r, &opening, &mut actions),
                None => empty_pane(ui, &t, f),
            });
        }
        TabKind::Message(r) => {
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.panel)).show_inside(ui, |ui| body = reading_pane(ui, &t, r, &opening, &mut actions));
        }
    }
    if (list_width - app.settings.list_width).abs() > 1.0 {
        app.settings.list_width = list_width;
        app.settings_dirty = true;
    }
    if body.is_some() && app.body.error.is_some() {
        // No WebView2: show the text in egui instead.
        let rect = body.take().unwrap();
        if let Some(m) = app.message() {
            fallback_body(ui, &t, rect, m, app.body.error.as_deref().unwrap_or(""));
        }
    }
    if let Some(r) = body {
        if app.body.page_loading() {
            let mut p = ui.painter().clone();
            p.set_clip_rect(r);
            let c = r.center() - vec2(0.0, (r.height() * 0.15).min(80.0));
            widgets::paint_spinner(&p, c, 15.0, t.accent);
        }
    }
    app.actions.extend(actions);
    body
}

// ---------------------------------------------------------------------------------------------
// Message list

fn message_list(ui: &mut egui::Ui, t: &Tokens, f: &mut Folder, preview: bool, focus_search: bool, actions: &mut Vec<Action>) {
    // Header: folder name, search, filters.
    egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 12, top: 12, bottom: 8 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.path.display().to_string());
            widgets::row_text(ui, ui.cursor().min + vec2(0.0, 11.0), name, theme::semibold(16.0), t.text, ui.available_width() - 40.0);
            ui.allocate_space(vec2(ui.available_width() - 60.0, 22.0));
            if icons::button(ui, "folder-down", 26.0, false, tr("Exporter les médias des messages affichés")).clicked() {
                actions.push(Action::ExportFolderMedia);
            }
            let icon = if f.newest_first { "arrow-down-up" } else { "arrow-down-up" };
            let tip = if f.newest_first { tr("Trié par date : les plus récents en haut") } else { tr("Trié par date : les plus anciens en haut") };
            if icons::button(ui, icon, 26.0, !f.newest_first, tip).clicked() {
                f.newest_first = !f.newest_first;
                f.view_dirty = true;
            }
        });
        ui.add_space(6.0);
        // Search field.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
        ui.painter().rect(rect, CornerRadius::same(16), t.field, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        icons::paint(ui, Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(16.0, 16.0)), "search", 15.0, t.text_muted);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(rect.min + vec2(32.0, 6.0), rect.max - vec2(30.0, 4.0))));
        let te = egui::TextEdit::singleline(&mut f.query).hint_text(tr("Rechercher (objet, personne, texte)")).frame(egui::Frame::NONE).desired_width(f32::INFINITY).font(theme::regular(13.0));
        let r = inner.add(te);
        if focus_search {
            r.request_focus();
        }
        if r.changed() {
            f.view_dirty = true;
        }
        if !f.query.is_empty() {
            let x = Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(22.0, 22.0));
            let xr = ui.interact(x, ui.id().with("clear_search"), Sense::click());
            Press::track(ui, &xr).wash(ui, x, 11, false);
            icons::paint(ui, x, "x", 13.0, t.text_muted);
            if theme::hand(xr).clicked() {
                f.query.clear();
                f.view_dirty = true;
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut v = f.filter;
            if widgets::segmented(ui, &mut v, &[(Filter::All, tr("Tous")), (Filter::Unread, tr("Non lus")), (Filter::Attachments, tr("Pièces jointes"))]) {
                f.filter = v;
                f.view_dirty = true;
            }
        });
    });
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, t.divider));
    if f.view_dirty {
        let keep = f.selected.clone();
        f.rebuild_view();
        f.selected = keep;
    }

    // Keyboard navigation (when no text field has focus).
    let typing = ui.ctx().egui_wants_keyboard_input();
    if !typing && !f.view.is_empty() {
        let (up, down, home, end, pgup, pgdn, enter) = ui.ctx().input_mut(|i| {
            use egui::{Key, Modifiers as M};
            (
                i.consume_key(M::NONE, Key::ArrowUp),
                i.consume_key(M::NONE, Key::ArrowDown),
                i.consume_key(M::NONE, Key::Home),
                i.consume_key(M::NONE, Key::End),
                i.consume_key(M::NONE, Key::PageUp),
                i.consume_key(M::NONE, Key::PageDown),
                i.consume_key(M::NONE, Key::Enter),
            )
        });
        let cur = f.selected_pos();
        let n = f.view.len() as isize;
        let target = if down {
            Some(cur.map(|c| c as isize + 1).unwrap_or(0))
        } else if up {
            Some(cur.map(|c| c as isize - 1).unwrap_or(0))
        } else if home {
            Some(0)
        } else if end {
            Some(n - 1)
        } else if pgdn {
            Some(cur.map(|c| c as isize + 10).unwrap_or(0))
        } else if pgup {
            Some(cur.map(|c| c as isize - 10).unwrap_or(0))
        } else {
            None
        };
        if let Some(k) = target {
            let k = k.clamp(0, n - 1) as usize;
            actions.push(Action::Select(f.items[f.view[k]].path.clone()));
            f.scroll_to_selected = true;
        }
        if enter {
            if let Some(p) = &f.selected {
                actions.push(Action::OpenInTab(p.clone()));
            }
        }
    }

    if f.items.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            if f.rx.is_some() {
                let r = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 90.0));
                let caption = if f.total > 0 { trf("Lecture du dossier… {d} / {n}", &[("d", &fmt::int(f.items.len())), ("n", &fmt::int(f.total))]) } else { tr("Lecture du dossier…").into() };
                widgets::loading(ui, r, &caption);
            } else {
                ui.label(egui::RichText::new(tr("Aucun message .msg dans ce dossier.")).color(t.text_muted));
            }
        });
        return;
    }
    if f.view.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| ui.label(egui::RichText::new(tr("Aucun résultat.")).color(t.text_muted)));
        return;
    }

    // Flatten into rows with Outlook's date groups.
    let row_h = if preview { ROW_H } else { ROW_H_COMPACT };
    let mut rows: Vec<(f32, Option<usize>, String)> = Vec::with_capacity(f.view.len() + 16);
    let mut y = 0.0;
    let mut last_group = String::new();
    for &idx in &f.view {
        let g = fmt::group(f.items[idx].date.as_ref());
        if g != last_group {
            rows.push((y, None, g.clone()));
            y += GROUP_H;
            last_group = g;
        }
        rows.push((y, Some(idx), String::new()));
        y += row_h;
    }
    let total = y;
    let selected = f.selected.clone();
    let scroll_to = std::mem::take(&mut f.scroll_to_selected);
    egui::ScrollArea::vertical().id_salt(("list", f.path.clone())).auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
        ui.set_height(total);
        let origin = ui.max_rect().min;
        let width = ui.max_rect().width();
        let first = rows.partition_point(|r| r.0 + row_h < viewport.min.y);
        for (ry, item, group) in rows[first..].iter() {
            if *ry > viewport.max.y {
                break;
            }
            match item {
                None => {
                    let r = Rect::from_min_size(origin + vec2(0.0, *ry), vec2(width, GROUP_H));
                    ui.painter().text(r.left_bottom() + vec2(16.0, -9.0), Align2::LEFT_BOTTOM, group, theme::semibold(12.0), t.text_muted);
                }
                Some(idx) => {
                    let s = &f.items[*idx];
                    let r = Rect::from_min_size(origin + vec2(0.0, *ry), vec2(width, row_h));
                    let resp = ui.interact(r, ui.id().with(("row", *idx)), Sense::click());
                    let is_sel = selected.as_ref() == Some(&s.path);
                    if is_sel && scroll_to {
                        ui.scroll_to_rect(r, None);
                    }
                    list_row(ui, t, r, s, is_sel, preview, &resp);
                    if resp.clicked() {
                        actions.push(Action::Select(s.path.clone()));
                    }
                    if resp.double_clicked() {
                        actions.push(Action::OpenInTab(s.path.clone()));
                    }
                    let path = s.path.clone();
                    resp.context_menu(|ui| {
                        if widgets::menu_icon_item(ui, "external-link", tr("Ouvrir dans un nouvel onglet"), tr("Entrée")).clicked() {
                            actions.push(Action::OpenInTab(path.clone()));
                            ui.close();
                        }
                        if widgets::menu_icon_item(ui, "folder-open", tr("Afficher dans l'explorateur"), "").clicked() {
                            actions.push(Action::Reveal(path.clone()));
                            ui.close();
                        }
                        if widgets::menu_icon_item(ui, "copy", tr("Copier le chemin"), "").clicked() {
                            actions.push(Action::CopyPath(path.clone()));
                            ui.close();
                        }
                    });
                }
            }
        }
    });
}

fn list_row(ui: &egui::Ui, t: &Tokens, r: Rect, s: &crate::msg::Summary, selected: bool, preview: bool, resp: &egui::Response) {
    let press = Press::track(ui, resp);
    let p = ui.painter();
    let bg = if selected { t.selected } else { press.fill(Color32::TRANSPARENT, t.hover, t.pressed) };
    if bg.a() > 0 {
        p.rect_filled(r.shrink2(vec2(6.0, 1.0)), CornerRadius::same(6), bg);
    }
    if selected || s.unread {
        let bar = Rect::from_min_size(r.min + vec2(6.0, 8.0), vec2(3.0, r.height() - 16.0));
        p.rect_filled(bar, CornerRadius::same(2), t.accent);
    }
    p.hline(r.x_range().shrink(16.0), r.bottom() - 0.5, Stroke::new(1.0, t.divider.gamma_multiply(0.6)));
    let left = r.left() + 18.0;
    let right = r.right() - 14.0;
    // Line 1: correspondent + status icons.
    let mut icon_x = right;
    let icon = |name: &str, color: Color32, x: &mut f32, y: f32| {
        icons::paint(ui, Rect::from_center_size(Pos2::new(*x - 7.0, y), vec2(14.0, 14.0)), name, 13.0, color);
        *x -= 18.0;
    };
    let y1 = r.top() + 16.0;
    if s.flagged {
        icon("flag", Color32::from_rgb(0xD1, 0x34, 0x38), &mut icon_x, y1);
    }
    if s.has_attachments {
        icon("paperclip", t.text_muted, &mut icon_x, y1);
    }
    if s.importance >= 2 {
        p.text(Pos2::new(icon_x - 6.0, y1), Align2::CENTER_CENTER, "!", theme::semibold(14.0), t.danger);
        icon_x -= 16.0;
    }
    let from = if s.html_file { s.from.clone() } else { fmt::clean_name(&s.from) };
    let from_font = if s.unread { theme::semibold(13.5) } else { theme::medium(13.5) };
    widgets::row_text(ui, Pos2::new(left, y1), if from.is_empty() { tr("(expéditeur inconnu)").into() } else { from }, from_font, t.text, icon_x - left - 6.0);
    // Line 2: subject + date.
    let y2 = r.top() + 34.0;
    let date = s.date.as_ref().map(fmt::list_date).unwrap_or_default();
    let dw = widgets::text_width(ui, &date, &theme::regular(11.5));
    p.text(Pos2::new(right, y2), Align2::RIGHT_CENTER, &date, theme::regular(11.5), if s.unread { t.accent_text } else { t.text_faint });
    let subject = if s.subject.trim().is_empty() { tr("(Pas d'objet)").to_string() } else { s.subject.clone() };
    let sub_col = if s.unread { t.accent_text } else { t.text };
    let sub_font = if s.unread { theme::semibold(12.5) } else { theme::regular(12.5) };
    widgets::row_text(ui, Pos2::new(left, y2), subject, sub_font, sub_col, right - left - dw - 10.0);
    // Line 3: preview.
    if preview {
        let text = if s.preview.is_empty() { tr("Ce message n'a pas de contenu").to_string() } else { s.preview.clone() };
        widgets::row_text(ui, Pos2::new(left, r.top() + 52.0), text, theme::regular(12.0), t.text_faint, right - left);
    }
    let _ = theme::hand(resp.clone());
}

fn empty_pane(ui: &mut egui::Ui, t: &Tokens, f: &Folder) {
    let r = ui.max_rect();
    icons::paint(ui, Rect::from_center_size(r.center() - vec2(0.0, 40.0), vec2(64.0, 64.0)), "mail-open", 60.0, t.text_faint.gamma_multiply(0.6));
    ui.painter().text(r.center() + vec2(0.0, 12.0), Align2::CENTER_CENTER, tr("Sélectionnez un élément à lire"), theme::semibold(16.0), t.text);
    let sub = if f.items.is_empty() { String::new() } else { tr("Rien n'est sélectionné · ↑ ↓ pour parcourir, Entrée pour ouvrir dans un onglet").into() };
    ui.painter().text(r.center() + vec2(0.0, 36.0), Align2::CENTER_CENTER, sub, theme::regular(12.5), t.text_muted);
}

// ---------------------------------------------------------------------------------------------
// Reading pane

fn reading_pane(ui: &mut egui::Ui, t: &Tokens, r: &Reader, opening: &[(u64, usize)], actions: &mut Vec<Action>) -> Option<Rect> {
    let m = match &r.state {
        ReaderState::Loading(_) => {
            widgets::loading(ui, ui.max_rect(), tr("Chargement du message…"));
            return None;
        }
        ReaderState::Failed(e) => {
            let c = ui.max_rect().center();
            icons::paint(ui, Rect::from_center_size(c - vec2(0.0, 36.0), vec2(40.0, 40.0)), "triangle-alert", 36.0, t.warning);
            ui.painter().text(c + vec2(0.0, 4.0), Align2::CENTER_CENTER, tr("Impossible d'ouvrir ce message"), theme::semibold(16.0), t.text);
            ui.painter().text(c + vec2(0.0, 28.0), Align2::CENTER_CENTER, e, theme::regular(12.5), t.text_muted);
            return None;
        }
        ReaderState::Ready(m) => m,
    };
    egui::Frame::NONE.inner_margin(egui::Margin { left: 24, right: 20, top: 16, bottom: 12 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        header(ui, t, m, actions);
        infobars(ui, t, m, r, actions);
        attachments(ui, t, m, r.id, opening, actions);
    });
    let rest = ui.available_rect_before_wrap();
    ui.painter().hline(rest.x_range(), rest.top(), Stroke::new(1.0, t.divider));
    let page = Rect::from_min_max(rest.min + vec2(0.0, 1.0), rest.max);
    ui.painter().rect_filled(page, 0, Color32::WHITE);
    ui.allocate_rect(page, Sense::hover());
    Some(page)
}

fn avatar(ui: &egui::Ui, center: Pos2, radius: f32, name: &str, key: &str) {
    let col = fmt::avatar_color(if key.is_empty() { name } else { key });
    ui.painter().circle_filled(center, radius, col);
    ui.painter().text(center, Align2::CENTER_CENTER, fmt::initials(name), theme::semibold(radius * 0.72), Color32::WHITE);
}

fn header(ui: &mut egui::Ui, t: &Tokens, m: &Message, actions: &mut Vec<Action>) {
    // Subject
    let subject = if m.subject.trim().is_empty() { tr("(Pas d'objet)") } else { m.subject.trim() };
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(subject).font(theme::semibold(19.0)).color(t.text)).wrap());
        if m.importance >= 2 {
            ui.label(egui::RichText::new("!").font(theme::semibold(18.0)).color(t.danger)).on_hover_text(tr("Importance haute"));
        }
        if m.flag_status == 2 {
            icons::image("flag", 16.0, Color32::from_rgb(0xD1, 0x34, 0x38)).paint_at(ui, Rect::from_min_size(ui.cursor().min + vec2(0.0, 4.0), vec2(16.0, 16.0)));
        }
    });
    ui.add_space(12.0);
    if m.body_kind == BodyKind::HtmlFile {
        return;
    }
    // Sender block
    let from_name = fmt::clean_name(m.from.label());
    let row_top = ui.cursor().top();
    ui.horizontal(|ui| {
        let (av, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
        avatar(ui, av.center(), 21.0, &from_name, &m.from.email);
        ui.add_space(6.0);
        let actions_w = 340.0f32.min(ui.available_width() * 0.5);
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - actions_w).max(160.0));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let name = ui.add(egui::Label::new(egui::RichText::new(if from_name.is_empty() { tr("(expéditeur inconnu)") } else { &from_name }).font(theme::semibold(14.0))).sense(Sense::click()));
                if !m.from.email.is_empty() && !m.from.email.eq_ignore_ascii_case(&from_name) {
                    ui.label(egui::RichText::new(format!("<{}>", m.from.email)).font(theme::regular(12.5)).color(t.text_muted));
                }
                person_menu(&name, &m.from.name, &m.from.email);
            });
            if let Some(b) = &m.on_behalf {
                ui.label(egui::RichText::new(trf("Envoyé par {a} de la part de {b}", &[("a", &fmt::person(b)), ("b", &from_name)])).font(theme::regular(12.0)).color(t.text_muted));
            }
            recipients_line(ui, t, "À", m.to().collect(), &m.display_to);
            let cc: Vec<&Recipient> = m.cc().collect();
            if !cc.is_empty() || !m.display_cc.is_empty() {
                recipients_line(ui, t, "Cc", cc, &m.display_cc);
            }
            let bcc: Vec<&Recipient> = m.bcc().collect();
            if !bcc.is_empty() || !m.display_bcc.is_empty() {
                recipients_line(ui, t, "Cci", bcc, &m.display_bcc);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let more = icons::button(ui, "ellipsis", 30.0, false, tr("Plus d'actions"));
            egui::Popup::menu(&more).show(|ui| {
                ui.set_min_width(240.0);
                let mut item = |ui: &mut egui::Ui, icon: &str, label: &str, a: Action| {
                    if widgets::menu_icon_item(ui, icon, label, "").clicked() {
                        actions.push(a);
                        ui.close();
                    }
                };
                item(ui, "printer", tr("Imprimer…"), Action::Print);
                item(ui, "save", tr("Enregistrer les pièces jointes…"), Action::SaveAll);
                item(ui, "images", tr("Exporter tous les médias…"), Action::ExportMedia);
                item(ui, "code", tr("Afficher les en-têtes Internet"), Action::Headers);
                item(ui, "table-2", tr("Propriétés MAPI"), Action::Properties);
            });
            if icons::button(ui, "forward", 30.0, false, tr("Transférer")).clicked() {
                actions.push(Action::Reply(Reply::Forward));
            }
            if icons::button(ui, "reply-all", 30.0, false, tr("Répondre à tous")).clicked() {
                actions.push(Action::Reply(Reply::ReplyAll));
            }
            if icons::button(ui, "reply", 30.0, false, tr("Répondre")).clicked() {
                actions.push(Action::Reply(Reply::Reply));
            }
        });
    });
    // Date under the action buttons, right-aligned like Outlook.
    if let Some(d) = m.date() {
        let txt = fmt::full_date(&d);
        let right = ui.max_rect().right();
        ui.painter().text(Pos2::new(right - 4.0, row_top + 46.0), Align2::RIGHT_CENTER, txt, theme::regular(12.0), t.text_muted);
    }
    ui.add_space(4.0);
}

fn person_menu(resp: &egui::Response, name: &str, email: &str) {
    let resp = if email.is_empty() { resp.clone() } else { resp.clone().on_hover_text(email) };
    let (name, email) = (name.to_owned(), email.to_owned());
    resp.context_menu(|ui| {
        if !email.is_empty() {
            if widgets::menu_icon_item(ui, "mail", tr("Écrire un message"), "").clicked() {
                crate::webview::open_external(&format!("mailto:{email}"));
                ui.close();
            }
            if widgets::menu_icon_item(ui, "copy", tr("Copier l'adresse"), "").clicked() {
                ui.ctx().copy_text(email.clone());
                ui.close();
            }
        }
        if widgets::menu_icon_item(ui, "user", tr("Copier le nom"), "").clicked() {
            ui.ctx().copy_text(fmt::clean_name(&name));
            ui.close();
        }
    });
}

fn recipients_line(ui: &mut egui::Ui, t: &Tokens, label: &'static str, list: Vec<&Recipient>, display: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        ui.label(egui::RichText::new(trf("{l} : ", &[("l", tr(label))])).font(theme::regular(12.5)).color(t.text_muted));
        if list.is_empty() {
            ui.label(egui::RichText::new(display).font(theme::regular(12.5)).color(t.text));
            return;
        }
        let n = list.len();
        for (k, r) in list.iter().enumerate() {
            let name = fmt::clean_name(r.person.label());
            let txt = if k + 1 < n { format!("{name}; ") } else { name };
            let resp = ui.add(egui::Label::new(egui::RichText::new(txt).font(theme::regular(12.5)).color(t.text)).sense(Sense::click()));
            person_menu(&resp, &r.person.name, &r.person.email);
        }
    });
}

fn infobars(ui: &mut egui::Ui, t: &Tokens, m: &Message, r: &Reader, actions: &mut Vec<Action>) {
    let bar = |ui: &mut egui::Ui, icon: &str, color: Color32, text: &str, button: Option<&str>| -> bool {
        let mut clicked = false;
        egui::Frame::NONE.fill(color.gamma_multiply(if t.dark() { 0.18 } else { 0.10 })).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::image(icon, 15.0, color).paint_at(ui, Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(15.0, 15.0)));
                ui.add_space(22.0);
                ui.add(egui::Label::new(egui::RichText::new(text).font(theme::regular(12.5)).color(t.text)).wrap());
                if let Some(b) = button {
                    if ui.link(egui::RichText::new(b).font(theme::medium(12.5)).color(t.accent_text)).clicked() {
                        clicked = true;
                    }
                }
            });
        });
        ui.add_space(6.0);
        clicked
    };
    if m.body_kind == BodyKind::HtmlFile {
        bar(ui, "triangle-alert", t.warning, tr("Ce fichier n'est pas un message Outlook : c'est une page web enregistrée avec l'extension .msg. Son contenu distant est bloqué."), None);
        return;
    }
    if m.importance >= 2 {
        bar(ui, "info", t.danger, tr("Ce message a une importance haute."), None);
    } else if m.importance == 0 {
        bar(ui, "info", t.text_muted, tr("Ce message a une importance faible."), None);
    }
    if m.is_draft() {
        bar(ui, "pen-line", t.text_muted, tr("Ce message n'a pas été envoyé (brouillon)."), None);
    }
    if m.flag_status == 2 {
        bar(ui, "flag", Color32::from_rgb(0xD1, 0x34, 0x38), tr("Assurer un suivi."), None);
    }
    if !r.allow_remote && crate::html::has_remote_content(m) {
        if bar(ui, "image-off", t.accent, tr("Pour protéger votre confidentialité, le téléchargement automatique de certaines images a été bloqué."), Some(tr("  Télécharger les images"))) {
            actions.push(Action::AllowRemote);
        }
    }
}

fn file_badge(name: &str, embedded: bool) -> (&'static str, Color32, &'static str) {
    if embedded {
        return ("mail", Color32::from_rgb(0x00, 0x78, 0xD4), "MSG");
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => ("file-text", Color32::from_rgb(0xD1, 0x34, 0x38), "PDF"),
        "doc" | "docx" | "rtf" | "odt" => ("file-text", Color32::from_rgb(0x2B, 0x57, 0x9A), "DOC"),
        "xls" | "xlsx" | "xlsm" | "csv" | "ods" => ("file-spreadsheet", Color32::from_rgb(0x21, 0x73, 0x46), "XLS"),
        "ppt" | "pptx" | "odp" => ("file", Color32::from_rgb(0xC4, 0x3E, 0x1C), "PPT"),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "heic" => ("image", Color32::from_rgb(0x88, 0x17, 0x98), "IMG"),
        "zip" | "rar" | "7z" | "gz" | "tar" => ("file-archive", Color32::from_rgb(0xB8, 0x86, 0x0B), "ZIP"),
        "dwg" | "dxf" | "step" | "stp" | "igs" | "iges" | "sldprt" | "sldasm" | "slddrw" | "stl" => ("shapes", Color32::from_rgb(0xCA, 0x50, 0x10), "CAO"),
        "msg" | "eml" => ("mail", Color32::from_rgb(0x00, 0x78, 0xD4), "MSG"),
        "txt" | "log" => ("file-text", Color32::from_rgb(0x60, 0x60, 0x68), "TXT"),
        _ => ("file", Color32::from_rgb(0x60, 0x60, 0x68), ""),
    }
}

fn attachments(ui: &mut egui::Ui, t: &Tokens, m: &Message, reader: u64, opening: &[(u64, usize)], actions: &mut Vec<Action>) {
    let list: Vec<(usize, &Attachment)> = m.visible_attachments().collect();
    if list.is_empty() {
        return;
    }
    ui.add_space(2.0);
    let card_w = 232.0;
    let gap = 8.0;
    let avail = ui.available_width();
    let cols = (((avail + gap) / (card_w + gap)).floor() as usize).max(1);
    let w = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).min(300.0);
    // At most two rows unless expanded.
    let id = ui.id().with(("att_expand", m.message_id.as_str(), m.subject.as_str()));
    let expanded = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let limit = if expanded { list.len() } else { (cols * 2).min(list.len()) };
    for row in list[..limit].chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (k, a) in row {
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
                let press = Press::track(ui, &resp);
                ui.painter().rect(rect, CornerRadius::same(6), press.fill(t.card, t.hover, t.pressed), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                let body = rect.translate(press.offset());
                let embedded = matches!(a.data, AttachData::Message(_));
                let (icon, col, _) = file_badge(&a.name, embedded);
                let badge = Rect::from_min_size(body.min + vec2(8.0, 8.0), vec2(30.0, 30.0));
                ui.painter().rect_filled(badge, CornerRadius::same(5), col.gamma_multiply(if t.dark() { 0.3 } else { 0.13 }));
                let busy = opening.contains(&(reader, *k));
                if busy {
                    widgets::paint_spinner(ui.painter(), badge.center(), 8.0, col);
                } else {
                    icons::paint(ui, badge, icon, 17.0, col);
                }
                widgets::row_text(ui, body.min + vec2(46.0, 16.0), a.name.clone(), theme::medium(12.5), t.text, body.width() - 54.0);
                let sub = match &a.data {
                    _ if busy => tr("Ouverture…").into(),
                    AttachData::Bytes(b) => fmt::human_size(b.len() as u64),
                    AttachData::Message(_) => tr("Message Outlook").into(),
                    AttachData::None => tr("Objet incorporé").into(),
                };
                widgets::row_text(ui, body.min + vec2(46.0, 32.0), sub, theme::regular(11.0), t.text_muted, body.width() - 54.0);
                let resp = theme::hand(resp).on_hover_text(format!("{}\n{}", a.name, tr("Cliquer pour ouvrir · clic droit pour enregistrer")));
                if resp.clicked() {
                    actions.push(Action::OpenAttachment(*k));
                }
                let k = *k;
                resp.context_menu(|ui| {
                    if widgets::menu_icon_item(ui, "external-link", tr("Ouvrir"), "").clicked() {
                        actions.push(Action::OpenAttachment(k));
                        ui.close();
                    }
                    if !embedded && widgets::menu_icon_item(ui, "save", tr("Enregistrer sous…"), "").clicked() {
                        actions.push(Action::SaveAttachment(k));
                        ui.close();
                    }
                    if widgets::menu_icon_item(ui, "download", tr("Enregistrer toutes les pièces jointes…"), "").clicked() {
                        actions.push(Action::SaveAll);
                        ui.close();
                    }
                    if widgets::menu_icon_item(ui, "images", tr("Exporter tous les médias…"), "").clicked() {
                        actions.push(Action::ExportMedia);
                        ui.close();
                    }
                });
            }
        });
        ui.add_space(gap - 2.0);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let total: usize = list.iter().map(|(_, a)| a.size()).sum();
        let files = list.iter().filter(|(_, a)| matches!(a.data, AttachData::Bytes(_))).count();
                let size = if total > 0 { format!(" ({})", fmt::human_size(total as u64)) } else { String::new() };
        ui.label(egui::RichText::new(format!("{}{size}", if list.len() > 1 { trf("{n} pièces jointes", &[("n", &list.len().to_string())]) } else { tr("1 pièce jointe").to_string() })).font(theme::regular(12.0)).color(t.text_muted));
        if list.len() > limit && ui.link(egui::RichText::new(trf("Afficher les {n} autres", &[("n", &(list.len() - limit).to_string())])).font(theme::medium(12.0)).color(t.accent_text)).clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(id, true));
        }
        if files > 1 && ui.link(egui::RichText::new(tr("Tout enregistrer")).font(theme::medium(12.0)).color(t.accent_text)).clicked() {
            actions.push(Action::SaveAll);
        }
        let media = crate::export::count(m);
        if media > files && ui.link(egui::RichText::new(trf("Exporter tous les médias ({n})", &[("n", &media.to_string())])).font(theme::medium(12.0)).color(t.accent_text)).on_hover_text(tr("Pièces jointes, images du message et fichiers des messages joints")).clicked() {
            actions.push(Action::ExportMedia);
        }
    });
    ui.add_space(4.0);
}

/// Body as text inside egui, when WebView2 can't run.
fn fallback_body(ui: &mut egui::Ui, t: &Tokens, rect: Rect, m: &Message, err: &str) {
    let text = match &m.body {
        crate::msg::Body::Text(s) => s.clone(),
        crate::msg::Body::Html(h) => fmt::html_text(h),
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(24.0, 14.0))));
    child.label(egui::RichText::new(err).font(theme::regular(11.5)).color(t.warning));
    child.add_space(6.0);
    egui::ScrollArea::vertical().id_salt("fallback_body").show(&mut child, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).color(Color32::from_rgb(0x20, 0x20, 0x20)).font(theme::regular(14.0))).wrap());
    });
}
