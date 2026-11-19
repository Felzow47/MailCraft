//! "Exporter tous les médias": every file a message carries (attachments, inline pictures,
//! hidden ones, and what attached messages carry), for one message or a whole folder.

use crate::msg::{self, AttachData, Message};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use crate::i18n::{tr, trf};

pub enum Progress {
    Step(usize),
    Done { files: usize, messages: usize, failed: usize, dir: PathBuf },
}

/// `dir/name`, or `dir/name (2).ext`… when it exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let mut p = dir.join(name);
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = Path::new(name).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut k = 2;
    while p.exists() {
        p = dir.join(format!("{stem} ({k}){ext}"));
        k += 1;
    }
    p
}

/// Number of files `write_message` would write.
pub fn count(m: &Message) -> usize {
    m.attachments
        .iter()
        .map(|a| match &a.data {
            AttachData::Bytes(_) => 1,
            AttachData::Message(sub) => count(sub),
            AttachData::None => 0,
        })
        .sum()
}

/// Writes the media of `m` into `dir` (created when needed); an attached message's media go to a
/// sub-folder named after it. Returns the number of files written.
pub fn write_message(m: &Message, dir: &Path) -> std::io::Result<usize> {
    let mut n = 0;
    for a in &m.attachments {
        match &a.data {
            AttachData::Bytes(b) => {
                std::fs::create_dir_all(dir)?;
                std::fs::write(unique_path(dir, &a.name), b)?;
                n += 1;
            }
            AttachData::Message(sub) if count(sub) > 0 => {
                let subject = if sub.subject.trim().is_empty() { tr("sans objet") } else { sub.subject.trim() };
                n += write_message(sub, &unique_path(dir, &short(&trf("Message joint - {s}", &[("s", subject)]))))?;
            }
            _ => {}
        }
    }
    Ok(n)
}

/// Folder name for one message of a folder export: "2019-11-26 RE: Demande de devis".
fn message_dir_name(m: &Message, path: &Path) -> String {
    let subject = if m.subject.trim().is_empty() { path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { m.subject.trim().to_owned() };
    match m.date() {
        Some(d) => short(&format!("{} {subject}", d.format("%Y-%m-%d"))),
        None => short(&subject),
    }
}

fn short(name: &str) -> String {
    let s: String = msg::sanitize_name(name).chars().take(80).collect();
    let s = s.trim().trim_end_matches('.').to_owned();
    if s.is_empty() { "message".into() } else { s }
}

/// Exports the media of every message in `files` into `dest`, one sub-folder per message, on a
/// worker thread.
pub fn folder(files: Vec<PathBuf>, dest: PathBuf) -> Receiver<Progress> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (mut written, mut messages, mut failed) = (0, 0, 0);
        for (i, p) in files.iter().enumerate() {
            match msg::load(p) {
                Ok(m) if count(&m) > 0 => match write_message(&m, &unique_path(&dest, &message_dir_name(&m, p))) {
                    Ok(n) => {
                        written += n;
                        messages += 1;
                    }
                    Err(_) => failed += 1,
                },
                Ok(_) => {}
                Err(_) => failed += 1,
            }
            if tx.send(Progress::Step(i + 1)).is_err() {
                return;
            }
        }
        let _ = tx.send(Progress::Done { files: written, messages, failed, dir: dest });
    });
    rx
}

#[cfg(test)]
mod tests {
    /// `MAILCRAFT_SAMPLE=<file.msg> cargo test -- --ignored export_sample`
    #[test]
    #[ignore]
    fn export_sample() {
        let src = std::path::PathBuf::from(std::env::var("MAILCRAFT_SAMPLE").expect("MAILCRAFT_SAMPLE"));
        let m = crate::msg::load(&src).unwrap();
        let dir = std::env::temp_dir().join("mailcraft-export-test");
        let _ = std::fs::remove_dir_all(&dir);
        let n = super::write_message(&m, &dir).unwrap();
        assert_eq!(n, super::count(&m));
        println!("{n} fichiers dans {}", dir.display());
    }
}
