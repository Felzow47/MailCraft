//! MailCraft - visionneuse de messages Outlook (.msg), comme dans le client mail.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// egui 0.37 renamed a few calls (show_inside, Ui::id, Id::new) that the shared Craft UI code still uses.
#![allow(deprecated)]

mod app;
mod branding;
mod chrome;
mod diag;
mod dialogs;
mod export;
mod fmt;
mod home;
mod html;
mod i18n;
mod i18n_en;
mod icons;
mod mailview;
mod msg;
mod platform;
mod settings;
mod theme;
mod webview;
mod widgets;

use eframe::egui;
use std::path::{Path, PathBuf};

pub fn is_msg(p: &Path) -> bool {
    p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("msg"))
}

fn attach_console() {
    // Release builds have no console: reattach to the parent's for --diag / --help output.
    #[cfg(windows)]
    unsafe {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(pid: u32) -> i32;
        }
        AttachConsole(u32::MAX);
    }
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(i) = args.iter().position(|x| x == "--diag") {
        attach_console();
        let Some(t) = args.get(i + 1).map(PathBuf::from) else {
            eprintln!("--diag <fichier.msg | dossier>");
            std::process::exit(2)
        };
        std::process::exit(diag::run(&t));
    }
    // Used by the installer: --register [--default] / --unregister.
    if args.iter().any(|x| x == "--register" || x == "--unregister") {
        #[cfg(windows)]
        {
            let res = if args.iter().any(|x| x == "--unregister") {
                platform::unregister()
            } else if args.iter().any(|x| x == "--default") {
                platform::make_default().map(|_| ())
            } else {
                platform::register(false)
            };
            if let Err(e) = res {
                attach_console();
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }
    if let Some(i) = args.iter().position(|x| x == "--make-icons") {
        attach_console();
        let dir = args.get(i + 1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("assets"));
        match branding::make_icons(&dir) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1)
            }
        }
    }
    let mut opts = app::StartupOptions { files: Vec::new(), theme: None, lang: None };
    let mut size = [1360.0f32, 860.0];
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("-h") | Some("--help") => {
                attach_console();
                println!("{} {}\n\nUsage : mailcraft [fichiers.msg | dossiers…] [--lang fr|en] [--theme dark|light] [--size LxH]\n        mailcraft --diag <fichier.msg | dossier>
        mailcraft --register [--default] | --unregister   (association des .msg)", branding::NAME, branding::VERSION);
                std::process::exit(0)
            }
            Some("--lang") => opts.lang = it.next().and_then(|s| s.into_string().ok()).map(|s| if s.starts_with("en") { i18n::Lang::En } else { i18n::Lang::Fr }),
            Some("--theme") => opts.theme = it.next().and_then(|s| s.into_string().ok()).map(|s| if s == "light" { theme::ThemeKind::Light } else { theme::ThemeKind::Dark }),
            Some("--size") => {
                if let Some((w, h)) = it.next().and_then(|s| s.into_string().ok()).and_then(|s| s.split_once('x').map(|(a, b)| (a.to_owned(), b.to_owned()))) {
                    size = [w.parse().unwrap_or(size[0]), h.parse().unwrap_or(size[1])];
                }
            }
            _ => opts.files.push(PathBuf::from(a)),
        }
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(branding::NAME)
        .with_app_id("mailcraft")
        .with_inner_size(size)
        .with_min_inner_size([820.0, 520.0])
        .with_drag_and_drop(true);
    if let Some((rgba, w, h)) = branding::icon_rgba(256) {
        viewport = viewport.with_icon(egui::IconData { rgba, width: w, height: h });
    }
    let native = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Wgpu, centered: true, ..Default::default() };
    eframe::run_native(branding::NAME, native, Box::new(move |cc| Ok(Box::new(app::App::new(cc, opts)))))
}
