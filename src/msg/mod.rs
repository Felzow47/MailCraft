//! Outlook `.msg` reader ([MS-OXMSG]): an OLE compound file holding MAPI properties, one storage
//! per recipient and attachment, and embedded messages as nested storages.

pub mod lzfu;
pub mod rtf;

use chrono::{DateTime, Local, TimeZone, Utc};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use crate::i18n::{tr, trf};

// Property ids.
const PR_MESSAGE_CLASS: u16 = 0x001A;
const PR_SUBJECT: u16 = 0x0037;
const PR_IMPORTANCE: u16 = 0x0017;
const PR_CLIENT_SUBMIT_TIME: u16 = 0x0039;
const PR_SENT_REPRESENTING_NAME: u16 = 0x0042;
const PR_SENT_REPRESENTING_EMAIL: u16 = 0x0065;
const PR_TRANSPORT_HEADERS: u16 = 0x007D;
const PR_SENDER_NAME: u16 = 0x0C1A;
const PR_SENDER_EMAIL: u16 = 0x0C1F;
const PR_RECIPIENT_TYPE: u16 = 0x0C15;
const PR_DISPLAY_BCC: u16 = 0x0E02;
const PR_DISPLAY_CC: u16 = 0x0E03;
const PR_DISPLAY_TO: u16 = 0x0E04;
const PR_DELIVERY_TIME: u16 = 0x0E06;
const PR_MESSAGE_FLAGS: u16 = 0x0E07;
const PR_HASATTACH: u16 = 0x0E1B;
const PR_BODY: u16 = 0x1000;
const PR_RTF_COMPRESSED: u16 = 0x1009;
const PR_HTML: u16 = 0x1013;
const PR_INTERNET_MESSAGE_ID: u16 = 0x1035;
const PR_FLAG_STATUS: u16 = 0x1090;
const PR_DISPLAY_NAME: u16 = 0x3001;
const PR_EMAIL_ADDRESS: u16 = 0x3003;
const PR_CREATION_TIME: u16 = 0x3007;
const PR_ATTACH_DATA: u16 = 0x3701;
const PR_ATTACH_FILENAME: u16 = 0x3704;
const PR_ATTACH_METHOD: u16 = 0x3705;
const PR_ATTACH_LONG_FILENAME: u16 = 0x3707;
const PR_ATTACH_MIME_TAG: u16 = 0x370E;
const PR_ATTACH_CONTENT_ID: u16 = 0x3712;
const PR_ATTACH_FLAGS: u16 = 0x3714;
const PR_SMTP_ADDRESS: u16 = 0x39FE;
const PR_INTERNET_CPID: u16 = 0x3FDE;
const PR_MESSAGE_CODEPAGE: u16 = 0x3FFD;
const PR_SENDER_SMTP: u16 = 0x5D01;
const PR_SENT_REPRESENTING_SMTP: u16 = 0x5D02;
const PR_RECIPIENT_DISPLAY_NAME: u16 = 0x5FF6;
const PR_ATTACHMENT_HIDDEN: u16 = 0x7FFE;

const ATTACH_EMBEDDED_MSG: i32 = 5;
const ATTACH_OLE: i32 = 6;
const MSGFLAG_READ: u32 = 0x1;
const MSGFLAG_UNSENT: u32 = 0x8;

pub type Error = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipientKind {
    To,
    Cc,
    Bcc,
}

#[derive(Clone, Debug)]
pub struct Person {
    pub name: String,
    pub email: String,
}

impl Person {
    /// What a mail client shows: the display name, else the address.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() { &self.email } else { &self.name }
    }
}

#[derive(Clone, Debug)]
pub struct Recipient {
    pub person: Person,
    pub kind: RecipientKind,
}

#[derive(Clone, Debug)]
pub enum AttachData {
    Bytes(Vec<u8>),
    Message(Box<Message>),
    /// OLE object or reference attachment: nothing to extract.
    None,
}

#[derive(Clone, Debug)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub content_id: String,
    pub hidden: bool,
    /// Referenced from the HTML body (an inline picture), so not listed as an attachment.
    pub inline: bool,
    pub data: AttachData,
}

impl Attachment {
    pub fn size(&self) -> usize {
        match &self.data {
            AttachData::Bytes(b) => b.len(),
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// `PR_HTML`.
    Html,
    /// HTML recovered from the compressed RTF (`\fromhtml1`).
    RtfHtml,
    /// Plain text.
    Text,
    /// Not an Outlook message but an HTML page saved with a `.msg` extension.
    HtmlFile,
    Empty,
}

#[derive(Clone, Debug)]
pub enum Body {
    Html(String),
    Text(String),
}

#[derive(Clone, Debug)]
pub struct Message {
    #[allow(dead_code)]
    pub class: String,
    pub subject: String,
    pub from: Person,
    /// "Sent on behalf of" when it differs from the sender.
    pub on_behalf: Option<Person>,
    pub recipients: Vec<Recipient>,
    pub display_to: String,
    pub display_cc: String,
    pub display_bcc: String,
    pub sent: Option<DateTime<Local>>,
    pub received: Option<DateTime<Local>>,
    pub created: Option<DateTime<Local>>,
    pub importance: i32,
    pub flags: u32,
    pub flag_status: i32,
    pub message_id: String,
    pub headers: String,
    pub body: Body,
    pub body_kind: BodyKind,
    pub attachments: Vec<Attachment>,
    /// Every top-level property, for the properties dialog: (tag, type, value preview).
    pub props: Vec<(String, String)>,
}

impl Message {
    pub fn to(&self) -> impl Iterator<Item = &Recipient> {
        self.recipients.iter().filter(|r| r.kind == RecipientKind::To)
    }
    pub fn cc(&self) -> impl Iterator<Item = &Recipient> {
        self.recipients.iter().filter(|r| r.kind == RecipientKind::Cc)
    }
    pub fn bcc(&self) -> impl Iterator<Item = &Recipient> {
        self.recipients.iter().filter(|r| r.kind == RecipientKind::Bcc)
    }
    /// The date a client shows in the reading pane.
    pub fn date(&self) -> Option<DateTime<Local>> {
        self.sent.or(self.received).or(self.created)
    }
    pub fn is_draft(&self) -> bool {
        self.flags & MSGFLAG_UNSENT != 0
    }
    pub fn visible_attachments(&self) -> impl Iterator<Item = (usize, &Attachment)> {
        self.attachments.iter().enumerate().filter(|(_, a)| !a.hidden && !a.inline)
    }
}

/// What the message list needs, read without touching bodies or attachment data.
#[derive(Clone, Debug)]
pub struct Summary {
    pub path: PathBuf,
    pub subject: String,
    pub from: String,
    pub from_email: String,
    pub to: String,
    pub date: Option<DateTime<Local>>,
    pub has_attachments: bool,
    pub importance: i32,
    pub unread: bool,
    pub flagged: bool,
    pub preview: String,
    pub html_file: bool,
}

// ---------------------------------------------------------------------------------------------
// Low-level property access

/// A storage of the compound file seen as a MAPI property bag.
struct Bag<'a, F: Read + Seek> {
    cf: &'a mut cfb::CompoundFile<F>,
    dir: String,
    /// Property tag (id << 16 | type) -> stream name, for the variable-size properties.
    streams: HashMap<u32, String>,
    /// Property tag -> 8-byte value, from `__properties_version1.0`.
    fixed: HashMap<u32, [u8; 8]>,
    codepage: Option<&'static encoding_rs::Encoding>,
}

impl<'a, F: Read + Seek> Bag<'a, F> {
    fn open(cf: &'a mut cfb::CompoundFile<F>, dir: &str, header: usize) -> Result<Self, Error> {
        let mut streams = HashMap::new();
        let entries: Vec<(String, bool)> = cf.read_storage(if dir.is_empty() { "/" } else { dir }).map_err(|e| e.to_string())?.map(|e| (e.name().to_owned(), e.is_stream())).collect();
        for (name, is_stream) in entries {
            if let Some(hex) = name.strip_prefix("__substg1.0_") {
                if hex.len() == 8 {
                    if let Ok(tag) = u32::from_str_radix(hex, 16) {
                        // Embedded messages are storages (type 000D); keep them too.
                        if is_stream || tag & 0xFFFF == 0x000D {
                            streams.insert(tag, name);
                        }
                    }
                }
            }
        }
        let mut bag = Self { cf, dir: dir.trim_end_matches('/').to_owned(), streams, fixed: HashMap::new(), codepage: None };
        if let Some(raw) = bag.read_stream("__properties_version1.0", usize::MAX) {
            let mut i = header;
            while i + 16 <= raw.len() {
                let tag = u32::from_le_bytes(raw[i..i + 4].try_into().unwrap());
                let mut v = [0u8; 8];
                v.copy_from_slice(&raw[i + 8..i + 16]);
                bag.fixed.insert(tag, v);
                i += 16;
            }
        }
        let cp = bag.int(PR_MESSAGE_CODEPAGE).or_else(|| bag.int(PR_INTERNET_CPID));
        bag.codepage = cp.and_then(|c| encoding_for_codepage(c as u32));
        Ok(bag)
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{}", self.dir, name)
    }

    fn read_stream(&mut self, name: &str, max: usize) -> Option<Vec<u8>> {
        let p = self.path(name);
        let mut s = self.cf.open_stream(&p).ok()?;
        let mut buf = Vec::new();
        if max == usize::MAX {
            s.read_to_end(&mut buf).ok()?;
        } else {
            s.take(max as u64).read_to_end(&mut buf).ok()?;
        }
        Some(buf)
    }

    fn string_limited(&mut self, id: u16, max_chars: usize) -> Option<String> {
        let max = max_chars.saturating_mul(2);
        let tag = (id as u32) << 16;
        if let Some(name) = self.streams.get(&(tag | 0x001F)).cloned() {
            let b = self.read_stream(&name, max)?;
            let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            return Some(String::from_utf16_lossy(&units).trim_end_matches('\0').to_owned());
        }
        if let Some(name) = self.streams.get(&(tag | 0x001E)).cloned() {
            let b = self.read_stream(&name, max)?;
            let enc = self.codepage.unwrap_or(encoding_rs::WINDOWS_1252);
            return Some(enc.decode(&b).0.trim_end_matches('\0').to_owned());
        }
        None
    }

    fn string(&mut self, id: u16) -> Option<String> {
        self.string_limited(id, usize::MAX / 2)
    }

    fn str_or_empty(&mut self, id: u16) -> String {
        self.string(id).unwrap_or_default()
    }

    fn binary(&mut self, id: u16) -> Option<Vec<u8>> {
        let name = self.streams.get(&((id as u32) << 16 | 0x0102)).cloned()?;
        self.read_stream(&name, usize::MAX)
    }

    fn fixed_value(&self, id: u16, ty: u16) -> Option<[u8; 8]> {
        self.fixed.get(&((id as u32) << 16 | ty as u32)).copied()
    }

    fn int(&self, id: u16) -> Option<i32> {
        self.fixed_value(id, 0x0003).map(|v| i32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }

    fn bool(&self, id: u16) -> Option<bool> {
        self.fixed_value(id, 0x000B).map(|v| v[0] != 0)
    }

    fn time(&self, id: u16) -> Option<DateTime<Local>> {
        self.fixed_value(id, 0x0040).and_then(|v| filetime(u64::from_le_bytes(v)))
    }

    /// Sub-storages whose name starts with `prefix`, sorted.
    fn children(&self, prefix: &str) -> Vec<String> {
        let mut v: Vec<String> = self
            .cf
            .read_storage(&self.dir_or_root())
            .map(|it| it.filter(|e| e.is_storage() && e.name().starts_with(prefix)).map(|e| e.name().to_owned()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    fn dir_or_root(&self) -> String {
        if self.dir.is_empty() { "/".into() } else { self.dir.clone() }
    }

    /// Short listing of every property, for the properties dialog.
    fn listing(&mut self) -> Vec<(String, String)> {
        let mut tags: Vec<u32> = self.streams.keys().chain(self.fixed.keys()).copied().collect();
        tags.sort();
        tags.dedup();
        let mut out = Vec::new();
        for tag in tags {
            let id = (tag >> 16) as u16;
            let ty = (tag & 0xFFFF) as u16;
            let value = match ty {
                0x001F | 0x001E => self.string_limited(id, 300).map(|s| s.replace(['\r', '\n'], " ")),
                0x0102 => self.streams.get(&tag).and_then(|n| self.cf.entry(self.path(n)).ok()).map(|e| trf("<{n} octets>", &[("n", &e.len().to_string())])),
                0x000D => Some(tr("<message incorporé>").into()),
                0x0003 => self.int(id).map(|v| v.to_string()),
                0x000B => self.bool(id).map(|v| v.to_string()),
                0x0040 => self.time(id).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
                0x0014 => self.fixed_value(id, ty).map(|v| i64::from_le_bytes(v).to_string()),
                0x0002 => self.fixed_value(id, ty).map(|v| i16::from_le_bytes([v[0], v[1]]).to_string()),
                0x0005 => self.fixed_value(id, ty).map(|v| f64::from_le_bytes(v).to_string()),
                _ => self.streams.get(&tag).map(|_| "<flux>".to_string()),
            };
            let name = prop_name(id).map(|n| format!("{n} ")).unwrap_or_default();
            out.push((format!("{name}0x{id:04X} {}", type_name(ty)), value.unwrap_or_default()));
        }
        out
    }
}

pub fn filetime(ft: u64) -> Option<DateTime<Local>> {
    if ft == 0 || ft == u64::MAX {
        return None;
    }
    let secs = (ft / 10_000_000) as i64 - 11_644_473_600;
    let nanos = ((ft % 10_000_000) * 100) as u32;
    // 4500-08-31 is MAPI's "no date".
    if !(0..32_000_000_000).contains(&secs) {
        return None;
    }
    Utc.timestamp_opt(secs, nanos).single().map(|d| d.with_timezone(&Local))
}

pub fn encoding_for_codepage(cp: u32) -> Option<&'static encoding_rs::Encoding> {
    use encoding_rs::*;
    Some(match cp {
        65001 => UTF_8,
        1200 => UTF_16LE,
        1201 => UTF_16BE,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1252 | 28591 | 20127 | 0 => WINDOWS_1252,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        874 => WINDOWS_874,
        932 => SHIFT_JIS,
        936 | 54936 => GB18030,
        949 => EUC_KR,
        950 => BIG5,
        20866 => KOI8_R,
        21866 => KOI8_U,
        28592 => ISO_8859_2,
        28595 => ISO_8859_5,
        28597 => ISO_8859_7,
        28605 => ISO_8859_15,
        50220 | 50221 | 50222 => ISO_2022_JP,
        51932 => EUC_JP,
        _ => return None,
    })
}

fn type_name(ty: u16) -> &'static str {
    match ty {
        0x001F => "PT_UNICODE",
        0x001E => "PT_STRING8",
        0x0102 => "PT_BINARY",
        0x0003 => "PT_LONG",
        0x000B => "PT_BOOLEAN",
        0x0040 => "PT_SYSTIME",
        0x0014 => "PT_I8",
        0x0002 => "PT_SHORT",
        0x0005 => "PT_DOUBLE",
        0x000D => "PT_OBJECT",
        0x0048 => "PT_CLSID",
        _ => "",
    }
}

fn prop_name(id: u16) -> Option<&'static str> {
    Some(match id {
        PR_MESSAGE_CLASS => "PR_MESSAGE_CLASS",
        PR_SUBJECT => "PR_SUBJECT",
        PR_IMPORTANCE => "PR_IMPORTANCE",
        PR_CLIENT_SUBMIT_TIME => "PR_CLIENT_SUBMIT_TIME",
        PR_SENT_REPRESENTING_NAME => "PR_SENT_REPRESENTING_NAME",
        PR_SENT_REPRESENTING_EMAIL => "PR_SENT_REPRESENTING_EMAIL_ADDRESS",
        PR_TRANSPORT_HEADERS => "PR_TRANSPORT_MESSAGE_HEADERS",
        PR_SENDER_NAME => "PR_SENDER_NAME",
        PR_SENDER_EMAIL => "PR_SENDER_EMAIL_ADDRESS",
        PR_DISPLAY_BCC => "PR_DISPLAY_BCC",
        PR_DISPLAY_CC => "PR_DISPLAY_CC",
        PR_DISPLAY_TO => "PR_DISPLAY_TO",
        PR_DELIVERY_TIME => "PR_MESSAGE_DELIVERY_TIME",
        PR_MESSAGE_FLAGS => "PR_MESSAGE_FLAGS",
        PR_HASATTACH => "PR_HASATTACH",
        PR_BODY => "PR_BODY",
        PR_RTF_COMPRESSED => "PR_RTF_COMPRESSED",
        PR_HTML => "PR_HTML",
        PR_INTERNET_MESSAGE_ID => "PR_INTERNET_MESSAGE_ID",
        PR_FLAG_STATUS => "PR_FLAG_STATUS",
        PR_CREATION_TIME => "PR_CREATION_TIME",
        0x3008 => "PR_LAST_MODIFICATION_TIME",
        0x0070 => "PR_CONVERSATION_TOPIC",
        0x0071 => "PR_CONVERSATION_INDEX",
        0x0E1D => "PR_NORMALIZED_SUBJECT",
        0x0E08 => "PR_MESSAGE_SIZE",
        0x0C1E => "PR_SENDER_ADDRTYPE",
        0x0064 => "PR_SENT_REPRESENTING_ADDRTYPE",
        0x3FFA => "PR_LAST_MODIFIER_NAME",
        PR_INTERNET_CPID => "PR_INTERNET_CPID",
        PR_MESSAGE_CODEPAGE => "PR_MESSAGE_CODEPAGE",
        PR_SENDER_SMTP => "PR_SENDER_SMTP_ADDRESS",
        PR_SENT_REPRESENTING_SMTP => "PR_SENT_REPRESENTING_SMTP_ADDRESS",
        0x1042 => "PR_IN_REPLY_TO_ID",
        0x1039 => "PR_INTERNET_REFERENCES",
        0x0026 => "PR_PRIORITY",
        0x0036 => "PR_SENSITIVITY",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Messages

fn is_html_file(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(512)]).trim_start_matches('\u{FEFF}').trim_start().to_ascii_lowercase();
    s.starts_with("<html") || s.starts_with("<!doctype html") || s.starts_with("<?xml") && s.contains("<html")
}

const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

pub fn load(path: &Path) -> Result<Message, Error> {
    let data = std::fs::read(path).map_err(|e| trf("lecture impossible : {e}", &[("e", &e.to_string())]))?;
    let mut m = parse(data)?;
    if m.body_kind == BodyKind::HtmlFile && m.subject.is_empty() {
        m.subject = file_stem(path);
    }
    Ok(m)
}

pub fn parse(data: Vec<u8>) -> Result<Message, Error> {
    if !data.starts_with(&OLE_MAGIC) {
        if is_html_file(&data) {
            return Ok(html_file(&data));
        }
        return Err(tr("ce fichier n'est pas un message Outlook (.msg)").into());
    }
    let mut cf = cfb::CompoundFile::open(Cursor::new(data)).map_err(|e| trf("conteneur OLE illisible : {e}", &[("e", &e.to_string())]))?;
    read_message(&mut cf, "", 32)
}

fn html_file(data: &[u8]) -> Message {
    let (text, _, _) = encoding_rs::UTF_8.decode(data);
    let html = text.into_owned();
    let subject = html_title(&html).unwrap_or_default();
    Message {
        class: "text/html".into(),
        subject,
        from: Person { name: String::new(), email: String::new() },
        on_behalf: None,
        recipients: Vec::new(),
        display_to: String::new(),
        display_cc: String::new(),
        display_bcc: String::new(),
        sent: None,
        received: None,
        created: None,
        importance: 1,
        flags: MSGFLAG_READ,
        flag_status: 0,
        message_id: String::new(),
        headers: String::new(),
        body: Body::Html(html),
        body_kind: BodyKind::HtmlFile,
        attachments: Vec::new(),
        props: Vec::new(),
    }
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let a = lower.find("<title")?;
    let a = a + lower[a..].find('>')? + 1;
    let b = a + lower[a..].find("</title")?;
    let t = html[a..b].trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Sender from the bag: the SMTP address when the stored one is an Exchange DN.
fn person(bag: &mut Bag<impl Read + Seek>, name: u16, email: u16, smtp: u16) -> Person {
    let name = bag.str_or_empty(name);
    let mut email_v = bag.string(smtp).filter(|s| !s.is_empty()).unwrap_or_else(|| bag.str_or_empty(email));
    if email_v.starts_with('/') {
        // Exchange legacy DN without an SMTP copy: keep the display name only.
        email_v = String::new();
    }
    Person { name, email: email_v }
}

fn read_message<F: Read + Seek>(cf: &mut cfb::CompoundFile<F>, dir: &str, header: usize) -> Result<Message, Error> {
    let mut bag = Bag::open(cf, dir, header)?;
    let class = bag.str_or_empty(PR_MESSAGE_CLASS);
    let subject = bag.str_or_empty(PR_SUBJECT);
    let mut from = person(&mut bag, PR_SENDER_NAME, PR_SENDER_EMAIL, PR_SENDER_SMTP);
    let repr = person(&mut bag, PR_SENT_REPRESENTING_NAME, PR_SENT_REPRESENTING_EMAIL, PR_SENT_REPRESENTING_SMTP);
    let mut on_behalf = None;
    if from.label().is_empty() {
        from = repr;
    } else if !repr.label().is_empty() && !repr.email.eq_ignore_ascii_case(&from.email) && repr.name != from.name {
        // Outlook: "<sender> on behalf of <representing>": the representing person is the author.
        on_behalf = Some(std::mem::replace(&mut from, repr));
    }
    let display_to = bag.str_or_empty(PR_DISPLAY_TO);
    let display_cc = bag.str_or_empty(PR_DISPLAY_CC);
    let display_bcc = bag.str_or_empty(PR_DISPLAY_BCC);
    let sent = bag.time(PR_CLIENT_SUBMIT_TIME);
    let received = bag.time(PR_DELIVERY_TIME);
    let created = bag.time(PR_CREATION_TIME);
    let importance = bag.int(PR_IMPORTANCE).unwrap_or(1);
    let flags = bag.int(PR_MESSAGE_FLAGS).unwrap_or(0) as u32;
    let flag_status = bag.int(PR_FLAG_STATUS).unwrap_or(0);
    let message_id = bag.str_or_empty(PR_INTERNET_MESSAGE_ID);
    let headers = bag.str_or_empty(PR_TRANSPORT_HEADERS);
    let props = bag.listing();
    let (body, body_kind) = read_body(&mut bag);
    let base = bag.dir.clone();
    let recip_dirs = bag.children("__recip_version1.0_");
    let attach_dirs = bag.children("__attach_version1.0_");
    drop(bag);

    let mut recipients = Vec::new();
    for r in recip_dirs {
        let mut rb = Bag::open(cf, &format!("{base}/{r}"), 8)?;
        let name = rb.string(PR_DISPLAY_NAME).filter(|s| !s.is_empty()).or_else(|| rb.string(PR_RECIPIENT_DISPLAY_NAME)).unwrap_or_default();
        let mut email = rb.string(PR_SMTP_ADDRESS).filter(|s| !s.is_empty()).unwrap_or_else(|| rb.str_or_empty(PR_EMAIL_ADDRESS));
        if email.starts_with('/') {
            email = String::new();
        }
        let kind = match rb.int(PR_RECIPIENT_TYPE).unwrap_or(1) & 0xF {
            2 => RecipientKind::Cc,
            3 => RecipientKind::Bcc,
            _ => RecipientKind::To,
        };
        recipients.push(Recipient { person: Person { name, email }, kind });
    }

    let mut attachments = Vec::new();
    for a in attach_dirs {
        let adir = format!("{base}/{a}");
        let mut ab = Bag::open(cf, &adir, 8)?;
        let method = ab.int(PR_ATTACH_METHOD).unwrap_or(1);
        let mut name = ab
            .string(PR_ATTACH_LONG_FILENAME)
            .filter(|s| !s.is_empty())
            .or_else(|| ab.string(PR_ATTACH_FILENAME).filter(|s| !s.is_empty()))
            .or_else(|| ab.string(PR_DISPLAY_NAME).filter(|s| !s.is_empty()))
            .unwrap_or_default();
        let mime = ab.str_or_empty(PR_ATTACH_MIME_TAG);
        let content_id = ab.str_or_empty(PR_ATTACH_CONTENT_ID).trim_matches(['<', '>']).to_owned();
        let hidden = ab.bool(PR_ATTACHMENT_HIDDEN).unwrap_or(false);
        let _flags = ab.int(PR_ATTACH_FLAGS);
        let embedded = ab.streams.get(&((PR_ATTACH_DATA as u32) << 16 | 0x000D)).cloned();
        // Method 5 is an embedded message; an OLE object (6) also has a 000D storage but isn't one.
        let data = if let Some(sub) = embedded.filter(|_| method == ATTACH_EMBEDDED_MSG || method != ATTACH_OLE) {
            drop(ab);
            match read_message(cf, &format!("{adir}/{sub}"), 24) {
                Ok(m) => {
                    if name.is_empty() {
                        name = if m.subject.is_empty() { "Message".into() } else { m.subject.clone() };
                    }
                    if !name.to_ascii_lowercase().ends_with(".msg") {
                        name.push_str(".msg");
                    }
                    AttachData::Message(Box::new(m))
                }
                Err(_) => AttachData::None,
            }
        } else {
            match ab.binary(PR_ATTACH_DATA) {
                Some(b) => AttachData::Bytes(b),
                None => AttachData::None,
            }
        };
        if name.is_empty() {
            name = trf("pièce jointe {n}", &[("n", &(attachments.len() + 1).to_string())]);
        }
        attachments.push(Attachment { name: sanitize_name(&name), mime, content_id, hidden, inline: false, data });
    }

    let mut msg = Message {
        class,
        subject,
        from,
        on_behalf,
        recipients,
        display_to,
        display_cc,
        display_bcc,
        sent,
        received,
        created,
        importance,
        flags,
        flag_status,
        message_id,
        headers,
        body,
        body_kind,
        attachments,
        props,
    };
    mark_inline(&mut msg);
    Ok(msg)
}

fn read_body<F: Read + Seek>(bag: &mut Bag<F>) -> (Body, BodyKind) {
    // PR_HTML can be binary (with the charset in a meta tag) or a string.
    if let Some(raw) = bag.binary(PR_HTML) {
        if !raw.is_empty() {
            let enc = html_charset(&raw).or(bag.codepage).unwrap_or(encoding_rs::UTF_8);
            let (s, _, _) = enc.decode(&raw);
            return (Body::Html(s.into_owned()), BodyKind::Html);
        }
    }
    if let Some(s) = bag.string(PR_HTML).filter(|s| !s.trim().is_empty()) {
        return (Body::Html(s), BodyKind::Html);
    }
    let rtf = bag.binary(PR_RTF_COMPRESSED).and_then(|c| lzfu::decompress(&c));
    if let Some(r) = &rtf {
        if let Some(html) = rtf::to_html(r) {
            if !html.trim().is_empty() {
                return (Body::Html(html), BodyKind::RtfHtml);
            }
        }
    }
    if let Some(t) = bag.string(PR_BODY).filter(|s| !s.is_empty()) {
        return (Body::Text(t), BodyKind::Text);
    }
    if let Some(r) = &rtf {
        let t = rtf::to_text(r);
        if !t.trim().is_empty() {
            return (Body::Text(t), BodyKind::Text);
        }
    }
    (Body::Text(String::new()), BodyKind::Empty)
}

/// `charset=` from the first KB of an HTML document.
fn html_charset(raw: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    let head = String::from_utf8_lossy(&raw[..raw.len().min(2048)]).to_ascii_lowercase();
    let i = head.find("charset=")? + 8;
    let rest = head[i..].trim_start_matches(['"', '\'']);
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).unwrap_or(rest.len());
    encoding_rs::Encoding::for_label(rest[..end].as_bytes())
}

pub fn sanitize_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect();
    s.trim().trim_end_matches('.').to_owned()
}

/// Attachments shown inside the HTML body are pictures of the mail, not files of it.
fn mark_inline(msg: &mut Message) {
    let Body::Html(html) = &msg.body else { return };
    let lower = html.to_ascii_lowercase();
    for a in &mut msg.attachments {
        if !matches!(a.data, AttachData::Bytes(_)) {
            continue;
        }
        let by_cid = !a.content_id.is_empty() && lower.contains(&format!("cid:{}", a.content_id.to_ascii_lowercase()));
        let img = a.mime.starts_with("image/") || is_image_name(&a.name);
        let by_name = img && !a.name.is_empty() && lower.contains(&format!("cid:{}", a.name.to_ascii_lowercase()));
        a.inline = by_cid || by_name;
    }
}

pub fn is_image_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tif", ".tiff"].iter().any(|e| l.ends_with(e))
}

// ---------------------------------------------------------------------------------------------
// Summaries (message list)

pub fn summary(path: &Path) -> Result<Summary, Error> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; 512];
    let n = f.read(&mut head).map_err(|e| e.to_string())?;
    if !head[..n].starts_with(&OLE_MAGIC) {
        if is_html_file(&head[..n]) {
            let m = load(path)?;
            let preview = match &m.body {
                Body::Html(h) => html_to_preview(h),
                Body::Text(t) => t.clone(),
            };
            return Ok(Summary {
                path: path.to_owned(),
                subject: if m.subject.is_empty() { file_stem(path) } else { m.subject },
                from: tr("Page HTML").into(),
                from_email: String::new(),
                to: String::new(),
                date: None,
                has_attachments: false,
                importance: 1,
                unread: false,
                flagged: false,
                preview: one_line(&preview, 200),
                html_file: true,
            });
        }
        return Err(tr("pas un message Outlook").into());
    }
    f.rewind().map_err(|e| e.to_string())?;
    let mut cf = cfb::CompoundFile::open(std::io::BufReader::new(f)).map_err(|e| e.to_string())?;
    let mut bag = Bag::open(&mut cf, "", 32)?;
    let subject = bag.str_or_empty(PR_SUBJECT);
    let mut from = person(&mut bag, PR_SENT_REPRESENTING_NAME, PR_SENT_REPRESENTING_EMAIL, PR_SENT_REPRESENTING_SMTP);
    if from.label().is_empty() {
        from = person(&mut bag, PR_SENDER_NAME, PR_SENDER_EMAIL, PR_SENDER_SMTP);
    }
    let to = bag.str_or_empty(PR_DISPLAY_TO);
    let date = bag.time(PR_CLIENT_SUBMIT_TIME).or_else(|| bag.time(PR_DELIVERY_TIME)).or_else(|| bag.time(PR_CREATION_TIME));
    let flags = bag.int(PR_MESSAGE_FLAGS).unwrap_or(MSGFLAG_READ as i32) as u32;
    let has_attachments = bag.bool(PR_HASATTACH).unwrap_or_else(|| !bag.children("__attach_version1.0_").is_empty());
    let importance = bag.int(PR_IMPORTANCE).unwrap_or(1);
    let flagged = bag.int(PR_FLAG_STATUS).unwrap_or(0) == 2;
    let preview = bag.string_limited(PR_BODY, 400).unwrap_or_default();
    Ok(Summary {
        path: path.to_owned(),
        subject,
        from: from.label().to_owned(),
        from_email: from.email,
        to,
        date,
        has_attachments,
        importance,
        unread: flags & MSGFLAG_READ == 0 && flags & MSGFLAG_UNSENT == 0,
        flagged,
        preview: one_line(&preview, 200),
        html_file: false,
    })
}

fn file_stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn one_line(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() || c == '\u{A0}' {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Rough text of an HTML document for the list preview.
pub fn html_to_preview(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<body").unwrap_or(0);
    let mut skip_until: Option<&str> = None;
    let bytes = &html[start..];
    let mut i = 0;
    while i < bytes.len() && out.len() < 600 {
        let rest = &lower[start + i..];
        if let Some(end) = skip_until {
            if rest.starts_with(end) {
                skip_until = None;
            }
            i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            continue;
        }
        if rest.starts_with("<style") {
            skip_until = Some("</style");
        } else if rest.starts_with("<script") {
            skip_until = Some("</script");
        }
        let c = bytes[i..].chars().next().unwrap();
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag {
            out.push(c);
        }
        i += c.len_utf8();
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}
