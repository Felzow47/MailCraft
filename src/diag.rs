//! Headless check: `mailcraft --diag <file|folder>` parses every message and reports what it found.

use crate::msg::{self, AttachData, BodyKind};
use std::collections::BTreeMap;
use std::path::Path;

pub fn run(target: &Path) -> i32 {
    let files: Vec<_> = if target.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(target).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| crate::is_msg(p)).collect();
        v.sort();
        v
    } else {
        vec![target.to_owned()]
    };
    let single = files.len() == 1;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let (mut no_subject, mut no_from, mut no_date, mut no_to, mut atts, mut inline, mut embedded, mut failed, mut remote) = (0, 0, 0, 0, 0, 0, 0, 0, 0);
    let mut examples: BTreeMap<&str, String> = BTreeMap::new();
    let t0 = std::time::Instant::now();
    for f in &files {
        let s = msg::summary(f);
        match msg::load(f) {
            Ok(m) => {
                *kinds.entry(format!("{:?}", m.body_kind)).or_default() += 1;
                no_subject += m.subject.trim().is_empty() as usize;
                let html_file = m.body_kind == BodyKind::HtmlFile;
                no_from += (!html_file && m.from.label().is_empty()) as usize;
                no_date += (!html_file && m.date().is_none()) as usize;
                no_to += (!html_file && m.recipients.is_empty() && m.display_to.is_empty()) as usize;
                atts += m.visible_attachments().count();
                inline += m.attachments.iter().filter(|a| a.inline).count();
                let emb = m.attachments.iter().filter(|a| matches!(a.data, AttachData::Message(_))).count();
                embedded += emb;
                let has_remote = crate::html::has_remote_content(&m);
                remote += has_remote as usize;
                let name = f.file_name().unwrap_or_default().to_string_lossy().into_owned();
                if has_remote {
                    examples.entry("images distantes").or_insert(name.clone());
                }
                if emb > 0 {
                    examples.entry("message joint").or_insert(name.clone());
                }
                if m.body_kind == BodyKind::Text {
                    examples.entry("texte brut").or_insert(name);
                }
                if let Err(e) = &s {
                    println!("RÉSUMÉ ÉCHOUÉ {} : {e}", f.display());
                }
                if single {
                    print_message(&m, 0);
                }
            }
            Err(e) => {
                failed += 1;
                println!("ÉCHEC {} : {e}", f.display());
            }
        }
    }
    println!(
        "\n{} fichiers en {:.1} s | corps {:?} | sans objet {no_subject}, sans expéditeur {no_from}, sans date {no_date}, sans destinataire {no_to} | pièces jointes {atts}, images intégrées {inline}, messages joints {embedded}, images distantes {remote} | échecs {failed}",
        files.len(),
        t0.elapsed().as_secs_f32(),
        kinds
    );
    if !single {
        for (k, v) in examples {
            println!("exemple {k} : {v}");
        }
    }
    (failed > 0) as i32
}

fn print_message(m: &msg::Message, depth: usize) {
    let p = "  ".repeat(depth);
    println!("{p}Objet   : {}", m.subject);
    println!("{p}De      : {} <{}>", m.from.name, m.from.email);
    if let Some(b) = &m.on_behalf {
        println!("{p}Envoyé par : {} <{}>", b.name, b.email);
    }
    for r in &m.recipients {
        println!("{p}{:?} : {} <{}>", r.kind, r.person.name, r.person.email);
    }
    println!("{p}Date    : {:?}", m.date());
    println!("{p}Corps   : {:?}", m.body_kind);
    for a in &m.attachments {
        println!("{p}PJ      : {} ({} o, {}, cid={}, caché={}, inline={})", a.name, a.size(), a.mime, a.content_id, a.hidden, a.inline);
        if let AttachData::Message(sub) = &a.data {
            print_message(sub, depth + 1);
        }
    }
    let body = match &m.body {
        msg::Body::Html(h) => h,
        msg::Body::Text(t) => t,
    };
    let cut: String = body.chars().take(1500).collect();
    println!("{p}--- début du corps ---\n{cut}\n{p}---");
}
