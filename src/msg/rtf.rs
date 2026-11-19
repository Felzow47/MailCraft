//! RTF bodies: HTML de-encapsulation ([MS-OXRTFEX], `\fromhtml1`) and a plain-text fallback.

use super::encoding_for_codepage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// HTML encapsulated by Outlook (`\fromhtml1`).
    Html,
    /// Plain text encapsulated by Outlook (`\fromtext`).
    Text,
    /// Genuine RTF.
    Native,
}

pub fn kind(rtf: &[u8]) -> Kind {
    let head = &rtf[..rtf.len().min(1024)];
    let has = |w: &[u8]| head.windows(w.len()).any(|x| x == w);
    if has(b"\\fromhtml") {
        Kind::Html
    } else if has(b"\\fromtext") {
        Kind::Text
    } else {
        Kind::Native
    }
}

/// Destinations whose content is never body text.
const SKIP_DEST: &[&str] = &[
    "fonttbl", "colortbl", "stylesheet", "info", "pict", "listtable", "listoverridetable", "rsidtbl", "generator", "xmlnstbl",
    "themedata", "colorschememapping", "latentstyles", "datastore", "filetbl", "revtbl", "header", "footer", "headerl", "headerr",
    "footerl", "footerr", "object", "fldinst", "mhtmltag", "pntext", "pntxta", "pntxtb", "bkmkstart", "bkmkend", "userprops",
    "operator", "author", "title", "subject", "company", "nonshppict", "shpinst", "falt", "panose", "wgrffmtfilter", "pgdsctbl",
];

#[derive(Clone, Copy)]
struct Group {
    skip: bool,
    htmltag: bool,
    htmlrtf: bool,
    uc: usize,
    star: bool,
    first: bool,
}

struct Out {
    text: String,
    pending: Vec<u8>,
    enc: &'static encoding_rs::Encoding,
}

impl Out {
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            let (s, _, _) = self.enc.decode(&self.pending);
            self.text.push_str(&s);
            self.pending.clear();
        }
    }
    fn byte(&mut self, b: u8) {
        self.pending.push(b);
    }
    fn str(&mut self, s: &str) {
        self.flush();
        self.text.push_str(s);
    }
}

/// HTML out of an `\fromhtml1` body; `None` for other RTF.
pub fn to_html(rtf: &[u8]) -> Option<String> {
    (kind(rtf) == Kind::Html).then(|| walk(rtf, true))
}

/// The visible text of any RTF body (used for `\fromtext` and native RTF without `PR_BODY`).
pub fn to_text(rtf: &[u8]) -> String {
    walk(rtf, false)
}

fn walk(rtf: &[u8], html: bool) -> String {
    let mut out = Out { text: String::with_capacity(rtf.len()), pending: Vec::new(), enc: encoding_rs::WINDOWS_1252 };
    let mut stack: Vec<Group> = Vec::new();
    let mut g = Group { skip: false, htmltag: false, htmlrtf: false, uc: 1, star: false, first: false };
    // Characters still to skip after a \uN (its ANSI fallback).
    let mut uc_skip = 0usize;
    let mut i = 0;
    let n = rtf.len();
    // Whether the current group's content reaches the output.
    let emit = |g: &Group| !g.skip && (!html || g.htmltag || !g.htmlrtf);
    while i < n {
        let c = rtf[i];
        match c {
            b'{' => {
                stack.push(g);
                g.first = true;
                g.star = false;
                uc_skip = 0;
                i += 1;
            }
            b'}' => {
                if let Some(p) = stack.pop() {
                    g = p;
                }
                uc_skip = 0;
                i += 1;
            }
            b'\r' | b'\n' => i += 1,
            b'\\' if i + 1 < n => {
                let d = rtf[i + 1];
                if d.is_ascii_alphabetic() {
                    let s = i + 1;
                    let mut j = s;
                    while j < n && rtf[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    let word = std::str::from_utf8(&rtf[s..j]).unwrap_or("");
                    let ns = j;
                    if j < n && rtf[j] == b'-' {
                        j += 1;
                    }
                    while j < n && rtf[j].is_ascii_digit() {
                        j += 1;
                    }
                    let param: Option<i32> = std::str::from_utf8(&rtf[ns..j]).ok().and_then(|p| p.parse().ok());
                    if j < n && rtf[j] == b' ' {
                        j += 1;
                    }
                    i = j;
                    let first = std::mem::replace(&mut g.first, false);
                    if first {
                        if html && (word == "htmltag") {
                            g.htmltag = true;
                            g.skip = false;
                            continue;
                        }
                        if SKIP_DEST.contains(&word) || g.star {
                            g.skip = true;
                            continue;
                        }
                    }
                    if g.skip {
                        continue;
                    }
                    if uc_skip > 0 && !matches!(word, "u" | "uc") {
                        uc_skip -= 1;
                        continue;
                    }
                    match word {
                        "ansicpg" => {
                            if let Some(enc) = param.and_then(|p| encoding_for_codepage(p as u32)) {
                                out.flush();
                                out.enc = enc;
                            }
                        }
                        "htmlrtf" => g.htmlrtf = param != Some(0),
                        "uc" => g.uc = param.unwrap_or(1).max(0) as usize,
                        "u" => {
                            if emit(&g) {
                                let v = param.unwrap_or(0);
                                let v = if v < 0 { v + 65536 } else { v } as u32;
                                if let Some(ch) = char::from_u32(v) {
                                    out.str(ch.encode_utf8(&mut [0; 4]));
                                }
                            }
                            uc_skip = g.uc;
                        }
                        _ if emit(&g) => {
                            let s = match word {
                                "par" | "line" => {
                                    if html && !g.htmltag && g.htmlrtf {
                                        ""
                                    } else {
                                        "\r\n"
                                    }
                                }
                                "tab" => "\t",
                                "lquote" => "\u{2018}",
                                "rquote" => "\u{2019}",
                                "ldblquote" => "\u{201C}",
                                "rdblquote" => "\u{201D}",
                                "bullet" => "\u{2022}",
                                "endash" => "\u{2013}",
                                "emdash" => "\u{2014}",
                                "emspace" | "enspace" | "qmspace" => " ",
                                "cell" if !html => "\t",
                                "row" if !html => "\r\n",
                                _ => "",
                            };
                            if !s.is_empty() {
                                out.str(s);
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Control symbol.
                    i += 2;
                    match d {
                        b'*' => {
                            g.star = true;
                            continue;
                        }
                        b'\'' => {
                            let hex = rtf.get(i..i + 2).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
                            i += 2;
                            g.first = false;
                            if uc_skip > 0 {
                                uc_skip -= 1;
                                continue;
                            }
                            if let (Some(b), true) = (hex, emit(&g)) {
                                out.byte(b);
                            }
                            continue;
                        }
                        _ => {}
                    }
                    g.first = false;
                    if g.skip || !emit(&g) {
                        continue;
                    }
                    if uc_skip > 0 {
                        uc_skip -= 1;
                        continue;
                    }
                    match d {
                        b'\\' | b'{' | b'}' => out.byte(d),
                        b'~' => out.str("\u{A0}"),
                        b'_' => out.str("\u{2011}"),
                        b'\r' | b'\n' => out.str("\r\n"),
                        _ => {}
                    }
                }
            }
            _ => {
                g.first = false;
                i += 1;
                if uc_skip > 0 {
                    uc_skip -= 1;
                    continue;
                }
                if emit(&g) {
                    out.byte(c);
                }
            }
        }
    }
    out.flush();
    out.text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deencapsulates_html() {
        let rtf = br"{\rtf1\ansi\ansicpg1252\fromhtml1 \deff0{\fonttbl{\f0\fswiss Arial;}}{\*\htmltag19 <html>}{\*\htmltag34 <body>}\htmlrtf {\htmlrtf0 caf\'e9 \{x\}{\*\htmltag84 &nbsp;}\par\htmlrtf }\htmlrtf0 {\*\htmltag35 </body>}}";
        let html = to_html(rtf).unwrap();
        assert_eq!(html, "<html><body>caf\u{e9} {x}&nbsp;\r\n</body>");
    }

    #[test]
    fn unicode_skips_fallback() {
        let rtf = br"{\rtf1\ansi\fromhtml1 {\*\htmltag1 <p>}\uc1\u8364?\u-3913?}";
        assert_eq!(to_html(rtf).unwrap(), "<p>\u{20AC}\u{F0B7}");
    }
}
