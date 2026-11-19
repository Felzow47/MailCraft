//! Modal windows: Internet headers, MAPI properties, settings, shortcuts, about.

use crate::app::{App, Dialog};
use crate::theme::{self, Accent, ThemePreference, Tokens};
use crate::widgets;
use egui::{vec2, Align2, Color32, CornerRadius, Sense, Stroke};
use crate::i18n::{tr, trf};

pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(d) = &app.dialog else { return };
    let t = Tokens::get(ctx);
    // Dim the window behind the dialog.
    let screen = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("dialog_dim"))).rect_filled(screen, 0, Color32::from_black_alpha(if t.dark() { 110 } else { 60 }));
    let title = match d {
        Dialog::Headers(_) => tr("En-têtes Internet"),
        Dialog::Properties(_) => tr("Propriétés MAPI"),
        Dialog::Settings => tr("Paramètres"),
        Dialog::About => tr("À propos"),
        Dialog::Shortcuts => tr("Raccourcis clavier"),
    };
    let size = match d {
        Dialog::Headers(_) | Dialog::Properties(_) => vec2((screen.width() - 120.0).min(860.0), (screen.height() - 140.0).min(620.0)),
        _ => vec2(480.0, 0.0),
    };
    let mut close = false;
    egui::Area::new(egui::Id::new("dialog")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        widgets::dialog_frame(ctx).show(ui, |ui| {
            widgets::begin_dialog(ui);
            ui.set_width(size.x);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).font(theme::semibold(16.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::button(ui, "x", 28.0, false, tr("Fermer (Échap)")).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(8.0);
            if matches!(app.dialog, Some(Dialog::Settings)) {
                settings(app, ui, &t);
                return;
            }
            match app.dialog.as_ref().unwrap() {
                Dialog::Headers(h) => {
                    let text = if h.trim().is_empty() { tr("Ce message ne contient pas d'en-têtes Internet (il n'a pas transité par SMTP, ou Outlook ne les a pas conservés).").to_string() } else { h.clone() };
                    egui::ScrollArea::both().max_height(size.y).auto_shrink([false, true]).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(text).font(theme::mono(12.0))).selectable(true).extend());
                    });
                    ui.add_space(8.0);
                    if !h.is_empty() && widgets::pill_button(ui, tr("Copier"), false).clicked() {
                        ctx.copy_text(h.clone());
                    }
                }
                Dialog::Properties(props) => {
                    egui::ScrollArea::vertical().max_height(size.y).auto_shrink([false, true]).show(ui, |ui| {
                        egui::Grid::new("props").num_columns(2).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
                            for (k, v) in props.iter() {
                                ui.label(egui::RichText::new(k).font(theme::mono(11.5)).color(t.text_muted));
                                ui.add(egui::Label::new(egui::RichText::new(v).font(theme::regular(12.0))).truncate().selectable(true));
                                ui.end_row();
                            }
                        });
                    });
                }
                Dialog::Settings => {}
                Dialog::About => about(ui, &t),
                Dialog::Shortcuts => shortcuts(ui, &t),
            }
        });
    });
    if close {
        app.dialog = None;
    }
}

fn settings(app: &mut App, ui: &mut egui::Ui, t: &Tokens) {
    let row = |ui: &mut egui::Ui, label: &str, sub: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - 200.0);
                ui.label(egui::RichText::new(label).font(theme::medium(13.0)));
                if !sub.is_empty() {
                    ui.label(egui::RichText::new(sub).font(theme::regular(11.5)).color(t.text_muted));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| add(ui));
        });
        ui.add_space(10.0);
    };
    let s = &mut app.settings;
    let mut dirty = false;
    row(ui, tr("Langue"), "", &mut |ui| {
        use crate::i18n::LangPref as L;
        dirty |= widgets::segmented(ui, &mut s.lang, &[(L::System, tr("Système")), (L::Fr, "Français"), (L::En, "English")]);
    });
    row(ui, tr("Thème"), "", &mut |ui| {
        dirty |= widgets::segmented(ui, &mut s.theme, &[(ThemePreference::System, tr("Système")), (ThemePreference::Light, tr("Clair")), (ThemePreference::Dark, tr("Sombre"))]);
    });
    row(ui, tr("Couleur d'accent"), tr(s.accent.label()), &mut |ui| {
        for a in Accent::ALL.iter().rev() {
            let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            let c = if t.dark() { a.colors().0 } else { a.colors().1 };
            ui.painter().circle_filled(rect.center(), 9.0, c);
            if s.accent == *a {
                ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(2.0, t.text));
            }
            if theme::hand(resp).on_hover_text(tr(a.label())).clicked() {
                s.accent = *a;
                dirty = true;
            }
        }
    });
    row(ui, tr("Toujours télécharger les images distantes"), tr("Désactivé, les images hébergées sur Internet ne sont chargées qu'à votre demande, comme dans Outlook."), &mut |ui| {
        dirty |= widgets::switch(ui, &mut s.remote_images).changed();
    });
    row(ui, tr("Aperçu dans la liste des messages"), tr("Affiche le début du texte sous l'objet."), &mut |ui| {
        dirty |= widgets::switch(ui, &mut s.preview_lines).changed();
    });
    if dirty {
        app.settings_dirty = true;
    }
    ui.separator();
    ui.add_space(6.0);
    default_app(app, ui, t);
}

fn about(ui: &mut egui::Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        widgets::app_icon(ui, 64.0);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            widgets::wordmark(ui, 22.0);
            ui.label(widgets::mono_caps(&format!("V{}", crate::branding::VERSION), 11.0, t.accent_text));
            ui.label(egui::RichText::new(trf("par {a}", &[("a", crate::branding::AUTHOR)])).font(theme::regular(12.0)).color(t.text_muted));
        });
    });
    ui.add_space(10.0);
    ui.label(tr("Visionneuse de messages Outlook (.msg) native, écrite en Rust (egui + WebView2). Lit le format MSG directement : Outlook n'est pas nécessaire."));
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            tr("Interface dans le style de la suite ArtCraft (mise en page reprise de PdfCraft, MIT/Apache-2.0). Projet indépendant, sans lien avec ArtCraft ni Microsoft. Icônes Lucide (ISC), polices Inter et JetBrains Mono (OFL)."),
        )
        .font(theme::regular(11.5))
        .color(t.text_muted),
    );
}

fn shortcuts(ui: &mut egui::Ui, t: &Tokens) {
    let list = [
        ("Ctrl+O", "Ouvrir un message"),
        ("Ctrl+Maj+O", "Ouvrir un dossier"),
        ("Ctrl+W", "Fermer l'onglet"),
        ("Ctrl+Tab", "Onglet suivant"),
        ("↑ ↓  Début  Fin", "Parcourir la liste"),
        ("Entrée", "Ouvrir le message dans un onglet"),
        ("Ctrl+F", "Rechercher dans le dossier"),
        ("Ctrl+R", "Répondre"),
        ("Ctrl+Maj+R", "Répondre à tous"),
        ("Ctrl+Maj+F", "Transférer"),
        ("Ctrl+P", "Imprimer"),
        ("Ctrl+ + / - / 0", "Zoom du message"),
        ("F1", "Cette aide"),
    ];
    egui::Grid::new("keys").num_columns(2).spacing(vec2(20.0, 7.0)).show(ui, |ui| {
        for (k, l) in list {
            egui::Frame::NONE.fill(t.field).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                ui.label(egui::RichText::new(tr(k)).font(theme::mono(11.5)));
            });
            ui.label(tr(l));
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tr("Les raccourcis s'appliquent quand le clavier n'est pas dans le corps du message : cliquez dans la liste ou l'en-tête pour le reprendre.")).font(theme::regular(11.5)).color(t.text_muted));
}

/// tr("Lecteur par défaut des fichiers .msg") section.
fn default_app(app: &mut App, ui: &mut egui::Ui, t: &Tokens) {
    use crate::platform::{self, MsgDefault};
    ui.label(egui::RichText::new(tr("Lecteur par défaut des fichiers .msg")).font(theme::medium(13.0)));
    // Reading the registry every frame is cheap enough (a few keys) and stays in sync with Windows.
    let current = platform::msg_default();
    let (icon, color, text) = match &current {
        MsgDefault::MailCraft => ("check", t.accent_text, tr("MailCraft ouvre les fichiers .msg (double-clic).").to_string()),
        MsgDefault::Pending => ("info", t.warning, tr("MailCraft est proposé pour les .msg : Windows demandera de confirmer au prochain double-clic (choisir MailCraft, puis « Toujours »).").to_string()),
        MsgDefault::Other(p) => ("info", t.text_muted, trf("Les fichiers .msg s'ouvrent actuellement avec {a}.", &[("a", &platform::prog_label(p))])),
        MsgDefault::Nobody => ("info", t.text_muted, tr("Aucune application n'ouvre les fichiers .msg.").to_string()),
    };
    ui.horizontal(|ui| {
        ui.add(crate::icons::image(icon, 14.0, color));
        ui.add(egui::Label::new(egui::RichText::new(text).font(theme::regular(12.0)).color(t.text_muted)).wrap());
    });
    if platform::is_portable() {
        ui.label(egui::RichText::new(tr("Version portable : l'association pointe vers cet emplacement ; si vous déplacez MailCraft, refaites-la.")).font(theme::regular(11.5)).color(t.warning));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let label = if current == MsgDefault::Pending { tr("Confirmer dans Windows") } else { tr("Définir MailCraft par défaut") };
        if current != MsgDefault::MailCraft && widgets::pill_button(ui, label, true).clicked() {
            match platform::make_default() {
                Ok(MsgDefault::MailCraft) => app.notify(tr("MailCraft ouvre maintenant les fichiers .msg")),
                Ok(_) => app.notify(tr("Dans la fenêtre Windows ouverte, choisissez MailCraft pour .msg")),
                Err(e) => app.notify(trf("Échec : {e}", &[("e", &e)])),
            }
        }
        if widgets::pill_button(ui, tr("Retirer l'association"), false).clicked() {
            match platform::unregister() {
                Ok(()) => app.notify(tr("MailCraft n'est plus associé aux fichiers .msg")),
                Err(e) => app.notify(trf("Échec : {e}", &[("e", &e)])),
            }
        }
    });
    ui.label(
        egui::RichText::new(tr("Sans droits administrateur, pour votre compte uniquement. Windows demande toujours une confirmation de votre part : MailCraft ouvre la page « Applications par défaut » pour la donner."))
            .font(theme::regular(11.0))
            .color(t.text_faint),
    );
}
