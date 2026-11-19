//! Turns a message body into the page shown by the reading pane: inline pictures served from
//! the app's own origin, scripts/forms/remote content blocked by a CSP (Outlook's "pictures
//! not downloaded" behaviour), and a plain-text body laid out like Outlook does.

use crate::msg::{Body, BodyKind, Message};
use crate::i18n::tr;

/// Origin of the custom protocol (`mailcraft://` is served as http://mailcraft.localhost on Windows).
pub const ORIGIN: &str = "http://mailcraft.localhost";

pub fn page(m: &Message, allow_remote: bool, nonce: u64) -> String {
    let remote = if allow_remote { " http: https:" } else { "" };
    let csp = format!(
        "default-src 'none'; img-src {ORIGIN} data:{remote}; style-src 'unsafe-inline' {ORIGIN}{remote}; font-src {ORIGIN} data:; media-src {ORIGIN}; form-action 'none'; base-uri 'none'"
    );
    // A saved web page lost its stylesheets: keep its unsized SVG icons from filling the pane.
    let extra = if m.body_kind == BodyKind::HtmlFile { "svg:not([width]){max-width:2em;max-height:2em}" } else { "" };
    let head = format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><meta name=\"mailcraft\" content=\"{nonce}\"><style>{BASE_CSS}{extra}</style>"
    );
    match &m.body {
        Body::Html(h) => inject_head(&neutralise(&rewrite_cid(h)), &head),
        Body::Text(t) => {
            let empty = m.body_kind == BodyKind::Empty || t.trim().is_empty();
            let inner = if empty { format!("<p class=\"mc-empty\">{}</p>", tr("(Ce message n'a pas de contenu)")) } else { format!("<div class=\"mc-text\">{}</div>", linkify(&escape(t))) };
            format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\">{head}</head><body>{inner}</body></html>")
        }
    }
}

/// Defaults that the message's own styles override (they come first in <head>).
/// One exception: a negative `text-indent` with no `margin-left` (old Outlook forward headers
/// re-saved by Word) is cancelled, as Word does, instead of pushing the text off the page.
const BASE_CSS: &str = "html{background:#fff;color:#000}\
body{margin:0;padding:18px 26px 28px;font-family:Calibri,Carlito,'Segoe UI',sans-serif;font-size:11pt;overflow-wrap:break-word}\
.mc-text{white-space:pre-wrap;font-family:Calibri,'Segoe UI',sans-serif;font-size:11pt}\
.mc-empty{color:#777;font-style:italic}\
[style*='text-indent:-']:not([style*='margin-left']){text-indent:0!important}\
a{color:#0563C1}\
::-webkit-scrollbar{width:12px;height:12px}::-webkit-scrollbar-thumb{background:#c8c8cc;border-radius:6px;border:3px solid #fff}::-webkit-scrollbar-thumb:hover{background:#a8a8ae}";

/// True when the HTML loads pictures from the internet.
pub fn has_remote_content(m: &Message) -> bool {
    let Body::Html(h) = &m.body else { return false };
    let l = h.to_ascii_lowercase();
    ["src=\"http", "src='http", "src=http", "background=\"http", "url(http", "url('http", "url(\"http"].iter().any(|p| l.contains(p))
        || l.contains("<link") && l.contains("stylesheet") && l.contains("href=\"http")
}

fn rewrite_cid(h: &str) -> String {
    let lower = h.to_ascii_lowercase();
    let mut out = String::with_capacity(h.len() + 256);
    let mut last = 0;
    let mut from = 0;
    while let Some(i) = lower[from..].find("cid:").map(|i| i + from) {
        let prev = lower[..i].chars().next_back();
        if matches!(prev, Some('"') | Some('\'') | Some('=') | Some('(')) {
            out.push_str(&h[last..i]);
            out.push_str(ORIGIN);
            out.push_str("/cid/");
            last = i + 4;
        }
        from = i + 4;
    }
    out.push_str(&h[last..]);
    out
}

/// Meta refresh would navigate away by itself: disable it.
fn neutralise(h: &str) -> String {
    let lower = h.to_ascii_lowercase();
    if !lower.contains("refresh") {
        return h.to_owned();
    }
    let mut s = h.to_owned();
    for pat in ["http-equiv=\"refresh\"", "http-equiv='refresh'", "http-equiv=refresh"] {
        let mut from = 0;
        loop {
            let l = s.to_ascii_lowercase();
            let Some(i) = l[from..].find(pat).map(|i| i + from) else { break };
            s.replace_range(i..i + pat.len(), "data-mc-disabled=\"refresh\"");
            from = i + 1;
        }
    }
    s
}

fn inject_head(h: &str, head: &str) -> String {
    let lower = h.to_ascii_lowercase();
    if let Some(i) = find_tag(&lower, "<head") {
        if let Some(j) = lower[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &h[..at], head, &h[at..]);
        }
    }
    if let Some(i) = find_tag(&lower, "<html") {
        if let Some(j) = lower[i..].find('>') {
            let at = i + j + 1;
            return format!("{}<head>{}</head>{}", &h[..at], head, &h[at..]);
        }
    }
    format!("<!DOCTYPE html><html><head>{head}</head><body>{h}</body></html>")
}

/// `<head` but not `<header`.
fn find_tag(lower: &str, tag: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = lower[from..].find(tag).map(|i| i + from) {
        let next = lower[i + tag.len()..].chars().next();
        if matches!(next, Some('>') | Some(' ') | Some('\t') | Some('\r') | Some('\n') | Some('/')) {
            return Some(i);
        }
        from = i + tag.len();
    }
    None
}

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            '\r' => {}
            _ => o.push(c),
        }
    }
    o
}

/// http(s) URLs and e-mail addresses of escaped text become links.
fn linkify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let next = ["http://", "https://", "www.", "mailto:"].iter().filter_map(|p| rest.find(p)).min();
        let Some(i) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = [tail.find(|c: char| c.is_whitespace() || c == '<' || c == '"'), tail.find("&gt;"), tail.find("&lt;")].into_iter().flatten().min().unwrap_or(tail.len());
        let mut url = &tail[..end];
        url = url.trim_end_matches(['.', ',', ';', ':', ')', ']', '!', '?']);
        if url.len() <= 7 {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        }
        let href = if url.starts_with("www.") { format!("http://{url}") } else { url.to_owned() };
        out.push_str(&format!("<a href=\"{href}\">{url}</a>"));
        rest = &tail[url.len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cid_rewritten_only_in_attributes() {
        let h = "<img src=\"cid:image001.png@01D\"> texte cid:non";
        assert_eq!(rewrite_cid(h), format!("<img src=\"{ORIGIN}/cid/image001.png@01D\"> texte cid:non"));
    }

    #[test]
    fn head_injected_after_head_not_header() {
        assert_eq!(inject_head("<html><header>x</header><head><title>t</title></head>", "X"), "<html><header>x</header><head>X<title>t</title></head>");
        assert_eq!(inject_head("<p>a</p>", "X"), "<!DOCTYPE html><html><head>X</head><body><p>a</p></body></html>");
    }

    #[test]
    fn links() {
        assert_eq!(linkify("voir www.a.fr."), "voir <a href=\"http://www.a.fr\">www.a.fr</a>.");
    }
}

#[cfg(test)]
mod dump {
    /// `MAILCRAFT_SAMPLE=<file.msg> MAILCRAFT_OUT=<page.html> cargo test -- --ignored dump_page`
    #[test]
    #[ignore]
    fn dump_page() {
        let m = crate::msg::load(std::path::Path::new(&std::env::var("MAILCRAFT_SAMPLE").unwrap())).unwrap();
        std::fs::write(std::env::var("MAILCRAFT_OUT").unwrap(), super::page(&m, false, 0)).unwrap();
    }
}
