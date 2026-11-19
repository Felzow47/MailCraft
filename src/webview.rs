//! The reading pane's message body: a WebView2 child window laid over the egui frame, fed through
//! a custom protocol (no size limit, inline pictures by Content-ID) and kept off the network.

use crate::html::ORIGIN;
use crate::msg::{AttachData, Message};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wry::http::{header::CONTENT_TYPE, Response, StatusCode};
use wry::{dpi, WebContext, WebView, WebViewBuilder};

#[derive(Default)]
struct Content {
    html: String,
    /// lower-cased Content-ID or file name -> (mime, bytes)
    files: HashMap<String, (String, Arc<Vec<u8>>)>,
}

pub struct BodyView {
    view: Option<WebView>,
    _ctx: Option<WebContext>,
    content: Arc<Mutex<Content>>,
    bounds: Option<[i32; 4]>,
    visible: bool,
    loaded: Option<(u64, bool)>,
    pub error: Option<String>,
    zoom: f64,
    /// Set when a page finished loading: WebView2 then grabs the keyboard, which we hand back.
    loaded_flag: Arc<AtomicBool>,
    /// When the page being loaded was requested; the view stays hidden (spinner) until it is done.
    awaiting: Option<std::time::Instant>,
}

impl BodyView {
    pub fn new() -> Self {
        Self { view: None, _ctx: None, content: Arc::default(), bounds: None, visible: false, loaded: None, error: None, zoom: 1.0, loaded_flag: Arc::default(), awaiting: None }
    }

    fn ensure(&mut self, frame: &eframe::Frame) -> Option<&WebView> {
        if self.view.is_none() && self.error.is_none() {
            let content = self.content.clone();
            let flag = self.loaded_flag.clone();
            let mut ctx = WebContext::new(Some(crate::settings::config_dir().join("WebView2")));
            let res = {
                use wry::WebViewBuilderExtWindows;
                WebViewBuilder::new_with_web_context(&mut ctx)
                    .with_https_scheme(false)
                    .with_devtools(false)
                    .with_visible(false)
                    .with_focused(false)
                    .with_browser_accelerator_keys(false)
                    .with_background_color((255, 255, 255, 255))
                    .with_bounds(wry::Rect { position: dpi::LogicalPosition::new(0, 0).into(), size: dpi::LogicalSize::new(10, 10).into() })
                    .with_custom_protocol("mailcraft".into(), move |_id, req| serve(&content, req.uri().path()))
                    .with_on_page_load_handler(move |ev, _| {
                        if matches!(ev, wry::PageLoadEvent::Finished) {
                            flag.store(true, Ordering::Relaxed);
                        }
                    })
                    .with_navigation_handler(|url| {
                        if url.starts_with(ORIGIN) || url == "about:blank" {
                            return true;
                        }
                        open_external(&url);
                        false
                    })
                    .with_new_window_req_handler(|url, _| {
                        open_external(&url);
                        wry::NewWindowResponse::Deny
                    })
                    .build_as_child(frame)
            };
            match res {
                Ok(v) => {
                    self.view = Some(v);
                    self._ctx = Some(ctx);
                }
                Err(e) => {
                    log::error!("WebView2 : {e}");
                    self.error = Some(crate::i18n::trf("Le moteur d'affichage WebView2 n'a pas pu démarrer ({e}).", &[("e", &e.to_string())]));
                }
            }
        }
        self.view.as_ref()
    }

    /// Shows `msg` (identified by `key`) in `rect` (egui points).
    pub fn show(&mut self, frame: &eframe::Frame, ctx: &egui::Context, rect: egui::Rect, key: u64, msg: &Message, allow_remote: bool) {
        if self.loaded != Some((key, allow_remote)) {
            self.set_content(msg, allow_remote, key);
            self.loaded_flag.store(false, Ordering::Relaxed);
            if let Some(v) = self.ensure(frame) {
                let _ = v.load_url(&format!("{ORIGIN}/body?m={key}&r={}", allow_remote as u8));
                self.loaded = Some((key, allow_remote));
                self.awaiting = Some(std::time::Instant::now());
            }
        }
        let ppp = ctx.pixels_per_point();
        let b = [(rect.left() * ppp).round() as i32, (rect.top() * ppp).round() as i32, (rect.width() * ppp).round().max(1.0) as i32, (rect.height() * ppp).round().max(1.0) as i32];
        if self.ensure(frame).is_none() {
            return;
        }
        let Some(v) = &self.view else { return };
        if self.bounds != Some(b) {
            let _ = v.set_bounds(wry::Rect { position: dpi::PhysicalPosition::new(b[0], b[1]).into(), size: dpi::PhysicalSize::new(b[2] as u32, b[3] as u32).into() });
            self.bounds = Some(b);
        }
        if self.loaded_flag.swap(false, Ordering::Relaxed) {
            let _ = v.focus_parent();
            self.awaiting = None;
        }
        // Give up waiting after a few seconds (a page that never reports "finished").
        if self.awaiting.is_some_and(|t| t.elapsed().as_secs_f32() > 4.0) {
            self.awaiting = None;
        }
        let show = self.awaiting.is_none();
        if show != self.visible {
            let _ = v.set_visible(show);
            self.visible = show;
        }
        if self.awaiting.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// True while a message page is loading (after a short delay, so quick loads don't flash).
    pub fn page_loading(&self) -> bool {
        self.awaiting.is_some_and(|t| t.elapsed().as_millis() > 120)
    }

    pub fn hide(&mut self) {
        if self.visible {
            if let Some(v) = &self.view {
                let _ = v.set_visible(false);
            }
            self.visible = false;
        }
    }

    pub fn print(&self) {
        if let Some(v) = &self.view {
            let _ = v.print();
        }
    }

    pub fn zoom_by(&mut self, f: f64) {
        self.zoom = if f == 0.0 { 1.0 } else { (self.zoom * f).clamp(0.3, 4.0) };
        if let Some(v) = &self.view {
            let _ = v.zoom(self.zoom);
        }
    }

    pub fn zoom_level(&self) -> f64 {
        self.zoom
    }

    fn set_content(&mut self, msg: &Message, allow_remote: bool, key: u64) {
        let mut files = HashMap::new();
        for a in &msg.attachments {
            let AttachData::Bytes(b) = &a.data else { continue };
            let mime = if a.mime.is_empty() { mime_for(&a.name).to_owned() } else { a.mime.clone() };
            let bytes = Arc::new(b.clone());
            if !a.content_id.is_empty() {
                files.insert(a.content_id.to_ascii_lowercase(), (mime.clone(), bytes.clone()));
            }
            files.entry(a.name.to_ascii_lowercase()).or_insert((mime, bytes));
        }
        let html = crate::html::page(msg, allow_remote, key);
        *self.content.lock().unwrap() = Content { html, files };
    }
}

fn serve(content: &Mutex<Content>, path: &str) -> Response<Cow<'static, [u8]>> {
    let c = content.lock().unwrap();
    let ok = |mime: &str, body: Vec<u8>| {
        Response::builder()
            .header(CONTENT_TYPE, mime)
            .header("Cache-Control", "no-store")
            .body(Cow::Owned(body))
            .unwrap()
    };
    if path == "/body" || path == "/" {
        return ok("text/html; charset=utf-8", c.html.as_bytes().to_vec());
    }
    let name = percent_decode(path.trim_start_matches("/cid/").trim_start_matches('/')).to_ascii_lowercase();
    let hit = c.files.get(&name).or_else(|| {
        // "image001.png@01D5..." also matches by the part before '@'.
        let stem = name.split('@').next().unwrap_or("");
        c.files.get(stem).or_else(|| name.rsplit('/').next().and_then(|n| c.files.get(n)))
    });
    match hit {
        Some((mime, b)) => ok(mime, b.as_ref().clone()),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Cow::Borrowed(&b""[..])).unwrap(),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn mime_for(name: &str) -> &'static str {
    let l = name.to_ascii_lowercase();
    let ext = l.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// Links clicked in a message open in the default browser / mail client, like in Outlook.
pub fn open_external(url: &str) {
    let l = url.to_ascii_lowercase();
    if l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:") || l.starts_with("tel:") {
        let _ = open::that_detached(url);
    }
}
