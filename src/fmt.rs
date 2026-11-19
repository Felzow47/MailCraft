//! Formatting the way Outlook shows it (French source, English through the i18n table): dates,
//! sizes, people, reply drafts.

use crate::msg::{Body, Message, Person};
use chrono::{DateTime, Datelike, Local};
use crate::i18n::{en, tr, trf};
use egui::Color32;

pub const DAYS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
pub const DAYS_LONG: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
pub const MONTHS: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

/// Reading pane: "lun. 26/09/2014 16:13".
pub fn full_date(d: &DateTime<Local>) -> String {
    format!("{} {}", tr(DAYS[d.weekday().num_days_from_monday() as usize]), d.format("%d/%m/%Y %H:%M"))
}

/// Forward/reply header: "vendredi 26 septembre 2014 16:13" / "Friday, 26 September 2014 16:13".
pub fn long_date(d: &DateTime<Local>) -> String {
    let (day, month) = (tr(DAYS_LONG[d.weekday().num_days_from_monday() as usize]), tr(MONTHS[d.month0() as usize]));
    let sep = if en() { "," } else { "" };
    format!("{day}{sep} {} {month} {} {}", d.day(), d.year(), d.format("%H:%M"))
}

/// Message list: time today, weekday this week, day/month this year, full date before.
pub fn list_date(d: &DateTime<Local>) -> String {
    let now = Local::now();
    let days = (now.date_naive() - d.date_naive()).num_days();
    if days == 0 {
        d.format("%H:%M").to_string()
    } else if (1..7).contains(&days) {
        format!("{} {}", tr(DAYS[d.weekday().num_days_from_monday() as usize]), d.format("%H:%M"))
    } else if d.year() == now.year() {
        format!("{} {}", tr(DAYS[d.weekday().num_days_from_monday() as usize]), d.format("%d/%m"))
    } else {
        d.format("%d/%m/%Y").to_string()
    }
}

/// Outlook's list groups.
pub fn group(d: Option<&DateTime<Local>>) -> String {
    let Some(d) = d else { return tr("Sans date").into() };
    let now = Local::now();
    let days = (now.date_naive() - d.date_naive()).num_days();
    let wd = now.weekday().num_days_from_monday() as i64;
    match days {
        i64::MIN..=-1 => tr("Plus tard").into(),
        0 => tr("Aujourd'hui").into(),
        1 => tr("Hier").into(),
        _ if days <= wd => tr("Cette semaine").into(),
        _ if days <= wd + 7 => tr("La semaine dernière").into(),
        _ if d.year() == now.year() && d.month() == now.month() => tr("Plus tôt ce mois-ci").into(),
        _ => {
            let m = tr(MONTHS[d.month0() as usize]);
            let mut c = m.chars();
            let cap: String = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
            format!("{cap} {}", d.year())
        }
    }
}

pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    let k = 1024.0;
    let u = if en() { ["B", "KB", "MB", "GB"] } else { ["o", "Ko", "Mo", "Go"] };
    let s = if b < k {
        format!("{bytes} {}", u[0])
    } else if b < k * k {
        format!("{:.0} {}", b / k, u[1])
    } else if b < k * k * k {
        format!("{:.1} {}", b / k / k, u[2])
    } else {
        format!("{:.2} {}", b / k / k / k, u[3])
    };
    if en() { s } else { s.replace('.', ",") }
}

pub fn int(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('\u{202F}');
        }
        out.push(ch);
    }
    out
}

/// Display name without the quotes Exchange often keeps ('MARTIN Georges').
pub fn clean_name(s: &str) -> String {
    s.trim().trim_matches(['\'', '"']).trim().to_owned()
}

/// Two letters for the avatar, as Outlook does.
pub fn initials(name: &str) -> String {
    let n = clean_name(name);
    let n = n.split(['(', '<', '@']).next().unwrap_or("");
    let words: Vec<&str> = n.split(|c: char| c.is_whitespace() || c == ',' || c == '.' || c == '_').filter(|w| w.chars().next().is_some_and(|c| c.is_alphanumeric())).collect();
    let mut s: String = match words.len() {
        0 => String::new(),
        1 => words[0].chars().take(2).collect(),
        _ => [words[0], words[words.len() - 1]].iter().filter_map(|w| w.chars().next()).collect(),
    };
    s = s.to_uppercase();
    if s.is_empty() { "?".into() } else { s }
}

/// A stable colour per correspondent (Outlook's persona palette).
pub fn avatar_color(key: &str) -> Color32 {
    const P: [Color32; 12] = [
        Color32::from_rgb(0x75, 0x0B, 0x1C),
        Color32::from_rgb(0xA4, 0x26, 0x2C),
        Color32::from_rgb(0xCA, 0x50, 0x10),
        Color32::from_rgb(0x8F, 0x7D, 0x34),
        Color32::from_rgb(0x49, 0x82, 0x05),
        Color32::from_rgb(0x00, 0x5B, 0x70),
        Color32::from_rgb(0x03, 0x83, 0x87),
        Color32::from_rgb(0x00, 0x78, 0xD4),
        Color32::from_rgb(0x00, 0x4E, 0x8C),
        Color32::from_rgb(0x4F, 0x6B, 0xED),
        Color32::from_rgb(0x88, 0x17, 0x98),
        Color32::from_rgb(0xC2, 0x39, 0xB3),
    ];
    let h = key.to_lowercase().bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    P[(h % P.len() as u32) as usize]
}

pub fn person(p: &Person) -> String {
    let name = clean_name(&p.name);
    if name.is_empty() || name.eq_ignore_ascii_case(&p.email) {
        p.email.clone()
    } else if p.email.is_empty() {
        name
    } else {
        format!("{name} <{}>", p.email)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Reply,
    ReplyAll,
    Forward,
}

fn prefixed(subject: &str, prefix: &str) -> String {
    let s = subject.trim();
    let l = s.to_lowercase();
    let known = ["re:", "re :", "tr:", "tr :", "fw:", "fwd:", "aw:", "wg:"];
    if known.iter().any(|k| l.starts_with(k)) && l.starts_with(&prefix.to_lowercase()[..2]) {
        s.to_owned()
    } else {
        format!("{prefix} {s}")
    }
}

/// A `mailto:` link that opens the default mail client on a reply or forward draft.
pub fn mailto(m: &Message, kind: Reply) -> String {
    let addr = |p: &Person| if p.email.is_empty() { None } else { Some(p.email.clone()) };
    let (to, cc): (Vec<String>, Vec<String>) = match kind {
        Reply::Reply => (addr(&m.from).into_iter().collect(), vec![]),
        Reply::ReplyAll => {
            let mut to: Vec<String> = addr(&m.from).into_iter().collect();
            to.extend(m.to().filter_map(|r| addr(&r.person)));
            (dedup(to), dedup(m.cc().filter_map(|r| addr(&r.person)).collect()))
        }
        Reply::Forward => (vec![], vec![]),
    };
    let subject = prefixed(&m.subject, if kind == Reply::Forward { tr("TR :") } else { tr("RE :") });
    let mut body = String::from("\r\n\r\n");
    body.push_str(&format!("{}\r\n", trf("De : {v}", &[("v", &person(&m.from))])));
    if let Some(d) = m.date() {
        body.push_str(&format!("{}\r\n", trf("Envoyé : {v}", &[("v", &long_date(&d))])));
    }
    let to_list: Vec<String> = m.to().map(|r| clean_name(r.person.label())).collect();
    if !to_list.is_empty() {
        body.push_str(&format!("{}\r\n", trf("À : {v}", &[("v", &to_list.join("; "))])));
    }
    let cc_list: Vec<String> = m.cc().map(|r| clean_name(r.person.label())).collect();
    if !cc_list.is_empty() {
        body.push_str(&format!("{}\r\n", trf("Cc : {v}", &[("v", &cc_list.join("; "))])));
    }
    body.push_str(&format!("{}\r\n\r\n", trf("Objet : {v}", &[("v", &m.subject)])));
    let text = match &m.body {
        Body::Text(t) => t.clone(),
        Body::Html(h) => html_text(h),
    };
    let mut url = format!("mailto:{}?subject={}", enc(&to.join(";")), enc(&subject));
    if !cc.is_empty() {
        url.push_str(&format!("&cc={}", enc(&cc.join(";"))));
    }
    // ShellExecute caps command lines around 2 KB: quote what fits.
    let room = 1900usize.saturating_sub(url.len() + enc(&body).len() + 10);
    let mut quoted = String::new();
    for c in text.chars() {
        if enc(&quoted).len() + enc(&c.to_string()).len() > room {
            quoted.push('…');
            break;
        }
        quoted.push(c);
    }
    body.push_str(&quoted);
    url.push_str(&format!("&body={}", enc(&body)));
    url
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }
    out
}

fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~@".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Readable text of an HTML body (forward quotes, fallback view).
pub fn html_text(h: &str) -> String {
    let mut out = String::new();
    let lower = h.to_ascii_lowercase();
    let start = lower.find("<body").unwrap_or(0);
    let mut i = start;
    let b = h.as_bytes();
    while i < h.len() {
        if b[i] == b'<' {
            let rest = &lower[i..];
            let close = rest.find('>').map(|j| i + j + 1).unwrap_or(h.len());
            let tag: String = rest[1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '/').collect();
            let skip_to = match tag.as_str() {
                "style" => lower[i..].find("</style").map(|j| i + j),
                "script" => lower[i..].find("</script").map(|j| i + j),
                "head" => lower[i..].find("</head").map(|j| i + j),
                _ => None,
            };
            if matches!(tag.as_str(), "br" | "p" | "/p" | "div" | "/div" | "tr" | "li" | "/h1" | "/h2" | "/h3" | "/table") && !out.ends_with("\n\n") {
                out.push('\n');
            }
            i = skip_to.map(|s| lower[s..].find('>').map(|j| s + j + 1).unwrap_or(h.len())).unwrap_or(close);
            continue;
        }
        let next = h[i..].find('<').map(|j| i + j).unwrap_or(h.len());
        let chunk = &h[i..next];
        let chunk = chunk.replace(['\r', '\n'], " ");
        out.push_str(&decode_entities(&chunk));
        i = next;
    }
    let mut lines: Vec<String> = out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
    lines.join("\n").trim().to_owned()
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find(';').filter(|e| *e < 10);
        let decoded = end.and_then(|e| {
            let name = &tail[1..e];
            let c = match name {
                "nbsp" => Some('\u{A0}'),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "eacute" => Some('é'),
                "egrave" => Some('è'),
                "agrave" => Some('à'),
                "ccedil" => Some('ç'),
                "ecirc" => Some('ê'),
                "rsquo" => Some('\u{2019}'),
                "euro" => Some('\u{20AC}'),
                _ if name.starts_with("#x") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
                _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(if c == '\u{A0}' { ' ' } else { c });
                rest = &tail[e + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_like_outlook() {
        assert_eq!(initials("'MARTIN Georges'"), "MG");
        assert_eq!(initials("georges93@email.fr"), "GE");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn reply_prefix() {
        assert_eq!(prefixed("TR: DDE", "RE :"), "RE : TR: DDE");
        assert_eq!(prefixed("RE: DDE", "RE :"), "RE: DDE");
    }

    #[test]
    fn html_to_text() {
        assert_eq!(html_text("<html><head><style>p{}</style></head><body><p>Bonjour&nbsp;Jean</p><p>caf&eacute;</p></body>"), "Bonjour Jean\n\ncafé");
    }
}
