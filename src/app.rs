//! Application state: tabs (mail folders and single messages), background loading, actions.

use crate::fmt::Reply;
use crate::msg::{self, AttachData, Message, Summary};
use crate::settings::Settings;
use crate::theme::{self, ThemeKind};
use crate::webview::BodyView;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use crate::i18n::{tr, trf};

pub enum ReaderState {
    Loading(Receiver<Result<Message, String>>),
    Ready(Box<Message>),
    Failed(String),
}

/// One message in the reading pane.
pub struct Reader {
    pub id: u64,
    pub path: PathBuf,
    pub state: ReaderState,
    pub allow_remote: bool,
    /// A message attached to another one (`path` is then the outer file).
    pub embedded: bool,
}

impl Reader {
    pub fn msg(&self) -> Option<&Message> {
        match &self.state {
            ReaderState::Ready(m) => Some(m),
            _ => None,
        }
    }
    fn poll(&mut self) -> bool {
        if let ReaderState::Loading(rx) = &self.state {
            if let Ok(r) = rx.try_recv() {
                self.state = match r {
                    Ok(m) => ReaderState::Ready(Box::new(m)),
                    Err(e) => ReaderState::Failed(e),
                };
                return true;
            }
        }
        false
    }
}

pub enum FolderMsg {
    Total(usize),
    Batch(Vec<Summary>),
    Failed(PathBuf, String),
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Unread,
    Attachments,
}

pub struct Folder {
    pub path: PathBuf,
    pub items: Vec<Summary>,
    pub rx: Option<Receiver<FolderMsg>>,
    pub total: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub query: String,
    pub filter: Filter,
    pub newest_first: bool,
    /// Indexes into `items`, filtered and sorted.
    pub view: Vec<usize>,
    pub view_dirty: bool,
    pub selected: Option<PathBuf>,
    pub scroll_to_selected: bool,
    pub reader: Option<Reader>,
}

impl Folder {
    pub fn rebuild_view(&mut self) {
        let q = fold(&self.query);
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut v: Vec<usize> = (0..self.items.len())
            .filter(|&i| {
                let s = &self.items[i];
                let pass = match self.filter {
                    Filter::All => true,
                    Filter::Unread => s.unread,
                    Filter::Attachments => s.has_attachments,
                };
                pass && (words.is_empty() || {
                    let hay = fold(&format!("{} {} {} {} {}", s.subject, s.from, s.from_email, s.to, s.preview));
                    words.iter().all(|w| hay.contains(w))
                })
            })
            .collect();
        let items = &self.items;
        v.sort_by(|&a, &b| {
            // Undated items (saved web pages) always go last.
            match (&items[a].date, &items[b].date) {
                (Some(_), None) => return std::cmp::Ordering::Less,
                (None, Some(_)) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            let o = items[a].date.cmp(&items[b].date).then_with(|| items[a].path.cmp(&items[b].path));
            if self.newest_first { o.reverse() } else { o }
        });
        self.view = v;
        self.view_dirty = false;
    }

    pub fn selected_pos(&self) -> Option<usize> {
        let sel = self.selected.as_ref()?;
        self.view.iter().position(|&i| &self.items[i].path == sel)
    }
}

/// Lower case without accents, for search.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            _ => c,
        })
        .collect()
}

pub enum TabKind {
    Folder(Folder),
    Message(Reader),
}

pub struct Tab {
    pub title: String,
    pub kind: TabKind,
}

impl Tab {
    pub fn reader(&self) -> Option<&Reader> {
        match &self.kind {
            TabKind::Folder(f) => f.reader.as_ref(),
            TabKind::Message(r) => Some(r),
        }
    }
    pub fn reader_mut(&mut self) -> Option<&mut Reader> {
        match &mut self.kind {
            TabKind::Folder(f) => f.reader.as_mut(),
            TabKind::Message(r) => Some(r),
        }
    }
}

/// An attachment being opened in its application (spinner on its card until it is launched).
pub struct Opening {
    pub reader: u64,
    pub index: usize,
    name: String,
    started: std::time::Instant,
    rx: Receiver<Result<(), String>>,
    launched: bool,
}

pub enum Dialog {
    Headers(String),
    Properties(Vec<(String, String)>),
    Settings,
    About,
    Shortcuts,
}

pub enum Action {
    OpenFileDialog,
    OpenFolderDialog,
    Open(PathBuf),
    CloseTab(usize),
    Select(PathBuf),
    OpenInTab(PathBuf),
    OpenAttachment(usize),
    SaveAttachment(usize),
    SaveAll,
    /// Every file of the open message: attachments, inline pictures, attached messages' files.
    ExportMedia,
    /// The same for every message shown in the open folder tab.
    ExportFolderMedia,
    Reply(Reply),
    Print,
    Headers,
    Properties,
    AllowRemote,
    Reveal(PathBuf),
    CopyPath(PathBuf),
    Zoom(f64),
    Home,
    ForgetRecent(String),
}

pub struct StartupOptions {
    pub files: Vec<PathBuf>,
    pub theme: Option<ThemeKind>,
    pub lang: Option<crate::i18n::Lang>,
}

pub struct App {
    pub settings: Settings,
    pub settings_dirty: bool,
    pub tabs: Vec<Tab>,
    pub active: Option<usize>,
    pub body: BodyView,
    pub dialog: Option<Dialog>,
    pub actions: Vec<Action>,
    /// (text, age in seconds, is an error)
    pub toast: Option<(String, f64, bool)>,
    pub opening: Vec<Opening>,
    pub search_focus: bool,
    /// Folder media export in progress: (progress, messages done, total).
    pub export: Option<(Receiver<crate::export::Progress>, usize, usize)>,
    next_id: u64,
    forced_theme: Option<ThemeKind>,
    forced_lang: Option<crate::i18n::Lang>,
    applied: Option<(ThemeKind, theme::Accent)>,
    last_title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, opts: StartupOptions) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_fonts(theme::font_definitions());
        let mut app = Self {
            settings: Settings::load(),
            settings_dirty: false,
            tabs: Vec::new(),
            active: None,
            body: BodyView::new(),
            dialog: None,
            actions: Vec::new(),
            toast: None,
            opening: Vec::new(),
            search_focus: false,
            export: None,
            next_id: 1,
            forced_theme: opts.theme,
            forced_lang: opts.lang,
            applied: None,
            last_title: String::new(),
        };
        crate::i18n::set_current(app.forced_lang.unwrap_or_else(|| app.settings.lang.resolve()));
        for f in opts.files {
            app.open(f);
        }
        app
    }

    pub fn theme_kind(&self, ctx: &egui::Context) -> ThemeKind {
        if let Some(k) = self.forced_theme {
            return k;
        }
        let sys = ctx.system_theme().map(|t| t == egui::Theme::Dark);
        self.settings.theme.resolve(sys, ThemeKind::Light)
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let k = (self.theme_kind(ctx), self.settings.accent);
        if self.applied != Some(k) {
            theme::apply(ctx, k.0, k.1);
            self.applied = Some(k);
        }
    }

    pub fn toggle_theme(&mut self, ctx: &egui::Context) {
        use theme::ThemePreference as P;
        self.forced_theme = None;
        self.settings.theme = if self.theme_kind(ctx) == ThemeKind::Dark { P::Light } else { P::Dark };
        self.settings_dirty = true;
    }

    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn reader_for(&mut self, path: PathBuf) -> Reader {
        let (tx, rx) = channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(msg::load(&p));
        });
        Reader { id: self.new_id(), path, state: ReaderState::Loading(rx), allow_remote: self.settings.remote_images, embedded: false }
    }

    /// Opens a .msg file or a folder of them (reusing an open tab).
    pub fn open(&mut self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).map(strip_unc).unwrap_or(path);
        if let Some(i) = self.tabs.iter().position(|t| match &t.kind {
            TabKind::Folder(f) => f.path == path,
            TabKind::Message(r) => r.path == path,
        }) {
            self.active = Some(i);
            return;
        }
        if path.is_dir() {
            self.settings.push_recent(&path);
            self.settings_dirty = true;
            let title = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
            let folder = Folder {
                path: path.clone(),
                items: Vec::new(),
                rx: Some(scan(path)),
                total: 0,
                failed: Vec::new(),
                query: String::new(),
                filter: Filter::All,
                newest_first: true,
                view: Vec::new(),
                view_dirty: true,
                selected: None,
                scroll_to_selected: false,
                reader: None,
            };
            self.tabs.push(Tab { title, kind: TabKind::Folder(folder) });
        } else if path.is_file() {
            self.settings.push_recent(&path);
            self.settings_dirty = true;
            let title = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let r = self.reader_for(path);
            self.tabs.push(Tab { title, kind: TabKind::Message(r) });
        } else {
            self.notify(trf("Introuvable : {p}", &[("p", &path.display().to_string())]));
            return;
        }
        self.active = Some(self.tabs.len() - 1);
    }

    fn open_embedded(&mut self, m: Message, parent: &Path) {
        let title = if m.subject.is_empty() { tr("Message joint").to_string() } else { m.subject.clone() };
        let r = Reader { id: self.new_id(), path: parent.to_owned(), state: ReaderState::Ready(Box::new(m)), allow_remote: self.settings.remote_images, embedded: true };
        self.tabs.push(Tab { title, kind: TabKind::Message(r) });
        self.active = Some(self.tabs.len() - 1);
    }

    pub fn close(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.tabs.remove(i);
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > i => Some(a - 1),
            Some(a) if a == i => Some(i.min(self.tabs.len() - 1)),
            a => a,
        };
    }

    pub fn notify(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), 0.0, false));
    }

    pub fn notify_error(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), 0.0, true));
    }

    pub fn reader(&self) -> Option<&Reader> {
        self.tabs.get(self.active?)?.reader()
    }

    pub fn message(&self) -> Option<&Message> {
        self.reader()?.msg()
    }

    fn reader_mut(&mut self) -> Option<&mut Reader> {
        let i = self.active?;
        self.tabs.get_mut(i)?.reader_mut()
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let mut busy = false;
        let mut changed = false;
        let mut failed = Vec::new();
        self.opening.retain_mut(|o| {
            match o.rx.try_recv() {
                Ok(Ok(())) => o.launched = true,
                Ok(Err(e)) => {
                    failed.push(trf("Impossible d'ouvrir {n} : {e}", &[("n", &o.name), ("e", &e)]));
                    return false;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) if !o.launched => return false,
                Err(_) => {}
            }
            // Keep the spinner a moment after the launch: the application takes a while to show.
            !(o.launched && o.started.elapsed().as_secs_f32() > 1.2)
        });
        for f in failed {
            self.notify_error(f);
        }
        busy |= !self.opening.is_empty();
        if let Some((rx, done, _)) = &mut self.export {
            busy = true;
            let mut finished = None;
            while let Ok(p) = rx.try_recv() {
                match p {
                    crate::export::Progress::Step(i) => *done = i,
                    crate::export::Progress::Done { files, messages, failed, dir } => finished = Some((files, messages, failed, dir)),
                }
            }
            if let Some((files, messages, failed, dir)) = finished {
                self.export = None;
                let errs = if failed > 0 { trf(" ({n} message(s) illisible(s))", &[("n", &failed.to_string())]) } else { String::new() };
                self.notify(format!("{}{errs}", trf("{f} fichier(s) de {m} message(s) exporté(s)", &[("f", &files.to_string()), ("m", &messages.to_string())])));
                let _ = open::that_detached(&dir);
            }
        }
        for t in &mut self.tabs {
            match &mut t.kind {
                TabKind::Folder(f) => {
                    if let Some(rx) = &f.rx {
                        busy = true;
                        loop {
                            match rx.try_recv() {
                                Ok(FolderMsg::Total(n)) => f.total = n,
                                Ok(FolderMsg::Batch(b)) => {
                                    f.items.extend(b);
                                    f.view_dirty = true;
                                }
                                Ok(FolderMsg::Failed(p, e)) => f.failed.push((p, e)),
                                Ok(FolderMsg::Done) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    f.rx = None;
                                    f.view_dirty = true;
                                    break;
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    if let Some(r) = &mut f.reader {
                        busy |= matches!(r.state, ReaderState::Loading(_));
                        changed |= r.poll();
                    }
                }
                TabKind::Message(r) => {
                    busy |= matches!(r.state, ReaderState::Loading(_));
                    if r.poll() {
                        changed = true;
                        if let Some(m) = r.msg() {
                            if !m.subject.trim().is_empty() {
                                t.title = m.subject.clone();
                            }
                        }
                    }
                }
            }
        }
        if busy || changed {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            self.open(p);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers as M};
        let mut a = Vec::new();
        ctx.input_mut(|i| {
            if i.consume_key(M::COMMAND | M::SHIFT, Key::O) {
                a.push(Action::OpenFolderDialog);
            }
            if i.consume_key(M::COMMAND, Key::O) {
                a.push(Action::OpenFileDialog);
            }
            if i.consume_key(M::COMMAND, Key::W) {
                if let Some(t) = self.active {
                    a.push(Action::CloseTab(t));
                }
            }
            if i.consume_key(M::COMMAND, Key::P) {
                a.push(Action::Print);
            }
            if i.consume_key(M::COMMAND | M::SHIFT, Key::R) {
                a.push(Action::Reply(Reply::ReplyAll));
            }
            if i.consume_key(M::COMMAND, Key::R) {
                a.push(Action::Reply(Reply::Reply));
            }
            if i.consume_key(M::COMMAND, Key::F) {
                self.search_focus = true;
            }
            if i.consume_key(M::COMMAND, Key::Plus) || i.consume_key(M::COMMAND, Key::Equals) {
                a.push(Action::Zoom(1.1));
            }
            if i.consume_key(M::COMMAND, Key::Minus) {
                a.push(Action::Zoom(1.0 / 1.1));
            }
            if i.consume_key(M::COMMAND, Key::Num0) {
                a.push(Action::Zoom(0.0));
            }
            if i.consume_key(M::CTRL, Key::Tab) && !self.tabs.is_empty() {
                self.active = Some(self.active.map(|x| (x + 1) % self.tabs.len()).unwrap_or(0));
            }
            if i.consume_key(M::NONE, Key::Escape) && self.dialog.is_some() {
                self.dialog = None;
            }
            if i.consume_key(M::NONE, Key::F1) {
                self.dialog = Some(Dialog::Shortcuts);
            }
        });
        // Ctrl+Shift+F: forward (Outlook's shortcut)
        if ctx.input_mut(|i| i.consume_key(M::COMMAND | M::SHIFT, Key::F)) {
            a.push(Action::Reply(Reply::Forward));
        }
        self.actions.extend(a);
    }

    fn run(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::OpenFileDialog => {
                if let Some(files) = rfd::FileDialog::new().set_title(tr("Ouvrir un message")).add_filter(tr("Messages Outlook"), &["msg"]).add_filter(tr("Tous les fichiers"), &["*"]).pick_files() {
                    for f in files {
                        self.open(f);
                    }
                }
            }
            Action::OpenFolderDialog => {
                if let Some(d) = rfd::FileDialog::new().set_title(tr("Ouvrir un dossier de messages")).pick_folder() {
                    self.open(d);
                }
            }
            Action::Open(p) => self.open(p),
            Action::CloseTab(i) => self.close(i),
            Action::Home => self.active = None,
            Action::Select(p) => {
                let Some(i) = self.active else { return };
                let same = matches!(&self.tabs[i].kind, TabKind::Folder(f) if f.selected.as_ref() == Some(&p) && f.reader.is_some());
                if same {
                    return;
                }
                let r = self.reader_for(p.clone());
                if let TabKind::Folder(f) = &mut self.tabs[i].kind {
                    f.selected = Some(p.clone());
                    f.reader = Some(r);
                    // Opening marks it read, as in a mail client.
                    if let Some(s) = f.items.iter_mut().find(|s| s.path == p) {
                        s.unread = false;
                    }
                }
            }
            Action::OpenInTab(p) => {
                let title = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let r = self.reader_for(p);
                self.tabs.push(Tab { title, kind: TabKind::Message(r) });
                self.active = Some(self.tabs.len() - 1);
            }
            Action::OpenAttachment(k) => {
                let Some(r) = self.reader() else { return };
                let (id, parent) = (r.id, r.path.clone());
                let Some(att) = r.msg().and_then(|m| m.attachments.get(k)).cloned() else { return };
                match att.data {
                    AttachData::Bytes(b) => {
                        if self.opening.iter().any(|o| o.reader == id && o.index == k) {
                            return;
                        }
                        let (tx, rx) = channel();
                        let name = att.name.clone();
                        std::thread::spawn(move || {
                            let res = crate::platform::write_temp(&name, &b, id).and_then(|p| crate::platform::shell_open(&p));
                            let _ = tx.send(res);
                        });
                        self.opening.push(Opening { reader: id, index: k, name: att.name, started: std::time::Instant::now(), rx, launched: false });
                    }
                    AttachData::Message(m) => self.open_embedded(*m, &parent),
                    AttachData::None => self.notify(tr("Cette pièce jointe (objet OLE ou lien) ne contient pas de fichier.")),
                }
            }
            Action::SaveAttachment(k) => {
                let Some(att) = self.message().and_then(|m| m.attachments.get(k)).cloned() else { return };
                let AttachData::Bytes(b) = &att.data else {
                    self.notify(tr("Seuls les fichiers peuvent être enregistrés."));
                    return;
                };
                if let Some(p) = rfd::FileDialog::new().set_title(tr("Enregistrer la pièce jointe")).set_file_name(&att.name).save_file() {
                    match std::fs::write(&p, b) {
                        Ok(()) => self.notify(trf("Enregistré : {p}", &[("p", &p.display().to_string())])),
                        Err(e) => self.notify(trf("Échec de l'enregistrement : {e}", &[("e", &e.to_string())])),
                    }
                }
            }
            Action::SaveAll => {
                let Some(m) = self.message() else { return };
                let files: Vec<(String, Vec<u8>)> = m
                    .visible_attachments()
                    .filter_map(|(_, a)| match &a.data {
                        AttachData::Bytes(b) => Some((a.name.clone(), b.clone())),
                        _ => None,
                    })
                    .collect();
                if files.is_empty() {
                    return;
                }
                if let Some(dir) = rfd::FileDialog::new().set_title(tr("Enregistrer toutes les pièces jointes")).pick_folder() {
                    let n = files.iter().filter(|(name, b)| std::fs::write(crate::export::unique_path(&dir, name), b).is_ok()).count();
                    self.notify(trf("{n} pièce(s) jointe(s) enregistrée(s) dans {d}", &[("n", &n.to_string()), ("d", &dir.display().to_string())]));
                }
            }
            Action::ExportMedia => {
                let Some(m) = self.message() else { return };
                if crate::export::count(m) == 0 {
                    self.notify(tr("Ce message ne contient ni pièce jointe ni image."));
                    return;
                }
                let Some(dir) = rfd::FileDialog::new().set_title(tr("Exporter tous les médias du message")).pick_folder() else { return };
                let Some(m) = self.message() else { return };
                match crate::export::write_message(m, &dir) {
                    Ok(n) => {
                        self.notify(trf("{n} fichier(s) exporté(s) dans {d}", &[("n", &n.to_string()), ("d", &dir.display().to_string())]));
                        let _ = open::that_detached(&dir);
                    }
                    Err(e) => self.notify(trf("Échec de l'export : {e}", &[("e", &e.to_string())])),
                }
            }
            Action::ExportFolderMedia => {
                if self.export.is_some() {
                    self.notify(tr("Un export est déjà en cours."));
                    return;
                }
                let Some(TabKind::Folder(f)) = self.active.and_then(|i| self.tabs.get(i)).map(|t| &t.kind) else { return };
                // The messages shown (search and filter applied), in list order.
                let files: Vec<PathBuf> = f.view.iter().map(|&i| f.items[i].path.clone()).collect();
                if files.is_empty() {
                    return;
                }
                let title = trf("Exporter les médias de {n} message(s)", &[("n", &files.len().to_string())]);
                let Some(dir) = rfd::FileDialog::new().set_title(&title).pick_folder() else { return };
                let total = files.len();
                self.export = Some((crate::export::folder(files, dir), 0, total));
            }
            Action::Reply(kind) => {
                if let Some(m) = self.message() {
                    let url = crate::fmt::mailto(m, kind);
                    if open::that_detached(&url).is_err() {
                        self.notify(tr("Aucun client de messagerie n'est associé aux liens mailto:."));
                    }
                }
            }
            Action::Print => {
                if self.message().is_some() {
                    self.body.print();
                }
            }
            Action::Headers => {
                if let Some(m) = self.message() {
                    self.dialog = Some(Dialog::Headers(m.headers.clone()));
                }
            }
            Action::Properties => {
                if let Some(m) = self.message() {
                    self.dialog = Some(Dialog::Properties(m.props.clone()));
                }
            }
            Action::AllowRemote => {
                if let Some(r) = self.reader_mut() {
                    r.allow_remote = true;
                }
            }
            Action::Reveal(p) => crate::platform::reveal(&p),
            Action::CopyPath(p) => {
                ctx.copy_text(p.display().to_string());
                self.notify(tr("Chemin copié"));
            }
            Action::Zoom(f) => self.body.zoom_by(f),
            Action::ForgetRecent(s) => {
                self.settings.recent.retain(|r| r != &s);
                self.settings_dirty = true;
            }
        }
    }
}

/// `\\?\C:\…` from canonicalize back to `C:\…`.
fn strip_unc(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    match s.strip_prefix(r"\\?\") {
        Some(r) if !r.starts_with("UNC") => PathBuf::from(r),
        _ => p,
    }
}

/// Reads the summaries of every .msg in `dir` on a worker thread.
fn scan(dir: PathBuf) -> Receiver<FolderMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| crate::is_msg(p)).collect();
        files.sort();
        if tx.send(FolderMsg::Total(files.len())).is_err() {
            return;
        }
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
        let chunks: Vec<Vec<PathBuf>> = files.chunks(files.len().div_ceil(threads).max(1)).map(|c| c.to_vec()).collect();
        std::thread::scope(|s| {
            for chunk in chunks {
                let tx = tx.clone();
                s.spawn(move || {
                    let mut batch = Vec::new();
                    for p in chunk {
                        match msg::summary(&p) {
                            Ok(sm) => batch.push(sm),
                            Err(e) => {
                                let _ = tx.send(FolderMsg::Failed(p, e));
                            }
                        }
                        if batch.len() >= 40 && tx.send(FolderMsg::Batch(std::mem::take(&mut batch))).is_err() {
                            return;
                        }
                    }
                    let _ = tx.send(FolderMsg::Batch(batch));
                });
            }
        });
        let _ = tx.send(FolderMsg::Done);
    });
    rx
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        crate::i18n::set_current(self.forced_lang.unwrap_or_else(|| self.settings.lang.resolve()));
        self.apply_theme(&ctx);
        self.poll(&ctx);
        self.handle_drops(&ctx);
        self.shortcuts(&ctx);

        crate::chrome::tab_strip(self, ui);
        crate::chrome::toolbar(self, ui);
        crate::chrome::status_bar(self, ui);
        let body_rect = match self.active {
            Some(i) if i < self.tabs.len() => crate::mailview::show(self, ui, i),
            _ => {
                crate::home::show(self, ui);
                None
            }
        };
        crate::dialogs::show(self, &ctx);
        crate::chrome::toast(self, &ctx);
        crate::chrome::drop_overlay(&ctx);

        // The page is a native window above egui: hide it while anything floats over it.
        let covered = self.dialog.is_some() || egui::Popup::is_any_open(&ctx) || ctx.input(|i| !i.raw.hovered_files.is_empty());
        let App { body, tabs, active, .. } = self;
        let reader = active.and_then(|i| tabs.get(i)).and_then(|t| t.reader());
        match (body_rect, reader.and_then(|r| r.msg().map(|m| (r, m)))) {
            (Some(rect), Some((r, m))) if !covered => body.show(frame, &ctx, rect, r.id, m, r.allow_remote),
            _ => body.hide(),
        }

        for a in std::mem::take(&mut self.actions) {
            self.run(a, &ctx);
        }
        if self.settings_dirty {
            self.settings.save();
            self.settings_dirty = false;
        }
        let title = match self.active.and_then(|i| self.tabs.get(i)) {
            Some(t) => format!("{} - {}", t.title, crate::branding::NAME),
            None => crate::branding::NAME.to_string(),
        };
        if self.last_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}
