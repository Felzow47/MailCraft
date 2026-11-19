//! Windows integration: opening attachments, Explorer, portable mode and the .msg association
//! (per user, under HKCU: no admin rights, fully reversible).

use std::path::{Path, PathBuf};

/// Writes an attachment to a private temp folder (opened from there, as Outlook does).
pub fn write_temp(name: &str, data: &[u8], key: u64) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("MailCraft").join(format!("{key:x}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(name);
    std::fs::write(&p, data).map_err(|e| e.to_string())?;
    Ok(p)
}

/// Opens a file with its default application and waits until Windows has launched it, so the
/// caller learns about failures. Blocking: call it from a worker thread. When no application is
/// registered for the type, Windows shows its own "How do you want to open this file?" picker;
/// closing that picker is not an error.
#[cfg(windows)]
pub fn shell_open(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct ShellExecuteInfoW {
        cb_size: u32,
        f_mask: u32,
        hwnd: isize,
        verb: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
        inst_app: isize,
        id_list: *mut std::ffi::c_void,
        class: *const u16,
        hkey_class: isize,
        hot_key: u32,
        icon_or_monitor: isize,
        process: isize,
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteExW(info: *mut ShellExecuteInfoW) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut std::ffi::c_void, coinit: u32) -> i32;
        fn CoUninitialize();
    }
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let file = wide(path.as_os_str());
    let dir = wide(path.parent().unwrap_or(path).as_os_str());
    // Our own error message replaces Windows' dialogs (SEE_MASK_FLAG_NO_UI); a type with no
    // application gets the "Open with" picker instead.
    let run = |verb: &str| -> Result<(), std::io::Error> {
        let verb = wide(std::ffi::OsStr::new(verb));
        let mut info = ShellExecuteInfoW {
            cb_size: std::mem::size_of::<ShellExecuteInfoW>() as u32,
            f_mask: 0x0000_0100 | 0x0000_0400, // SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI
            hwnd: 0,
            verb: verb.as_ptr(),
            file: file.as_ptr(),
            parameters: std::ptr::null(),
            directory: dir.as_ptr(),
            show: 1, // SW_SHOWNORMAL
            inst_app: 0,
            id_list: std::ptr::null_mut(),
            class: std::ptr::null(),
            hkey_class: 0,
            hot_key: 0,
            icon_or_monitor: 0,
            process: 0,
        };
        if unsafe { ShellExecuteExW(&mut info) } != 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
    };
    // COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE, as the ShellExecuteEx docs ask.
    let com = unsafe { CoInitializeEx(std::ptr::null_mut(), 0x2 | 0x4) };
    let mut res = run("open");
    if res.as_ref().err().and_then(|e| e.raw_os_error()) == Some(1155) {
        res = run("openas");
    }
    if com >= 0 {
        unsafe { CoUninitialize() };
    }
    let Err(err) = res else { return Ok(()) };
    Err(match err.raw_os_error() {
        Some(1223) => return Ok(()), // ERROR_CANCELLED: the user closed the app picker
        Some(1155) => crate::i18n::tr("aucune application n'est associée à ce type de fichier").into(),
        Some(2) | Some(3) => crate::i18n::tr("le fichier temporaire est introuvable").into(),
        _ => err.to_string(),
    })
}

pub fn reveal(p: &Path) {
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
}

/// Portable copy: the executable's name contains "portable", or a `MailCraft-data` folder sits
/// next to it. Settings and the WebView2 cache then live in that folder, nothing in %APPDATA%.
pub fn portable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("MailCraft-data");
    let named = exe.file_stem().is_some_and(|s| s.to_string_lossy().to_ascii_lowercase().contains("portable"));
    (named || dir.is_dir()).then_some(dir)
}

pub fn is_portable() -> bool {
    portable_dir().is_some()
}

/// Who opens .msg files for the current user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MsgDefault {
    MailCraft,
    /// Declared as the .msg handler, but the user never confirmed a choice in Windows: with
    /// several apps registered for .msg, Windows asks on the next double-click.
    Pending,
    /// Another program (its ProgID), e.g. `Outlook.File.msg.15`.
    Other(String),
    Nobody,
}

#[cfg(windows)]
mod assoc {
    use super::MsgDefault;
    use crate::branding::{NAME, PROG_ID};
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_ALL_ACCESS};
    use winreg::RegKey;

    const CAPS: &str = "Software\\MailCraft\\Capabilities";
    const USER_CHOICE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.msg\\UserChoice";

    fn exe_name() -> String {
        std::env::current_exe().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "mailcraft.exe".into())
    }

    /// Declares MailCraft as a .msg viewer ("Open with", Default apps); with `make_default`, also
    /// makes it the handler of .msg files when Windows lets a program do so.
    pub fn register(make_default: bool) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
        let open_cmd = format!("\"{exe}\" \"%1\"");
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let set = |path: &str, name: &str, value: &str| -> Result<(), String> {
            let (k, _) = hkcu.create_subkey(path).map_err(|e| format!("{path} : {e}"))?;
            k.set_value(name, &value).map_err(|e| format!("{path} : {e}"))
        };
        let prog = format!("Software\\Classes\\{PROG_ID}");
        set(&prog, "", "Message Outlook")?;
        set(&prog, "FriendlyTypeName", "Message Outlook")?;
        set(&format!("{prog}\\DefaultIcon"), "", &format!("\"{exe}\",0"))?;
        set(&format!("{prog}\\shell\\open\\command"), "", &open_cmd)?;
        let app = format!("Software\\Classes\\Applications\\{}", exe_name());
        set(&app, "FriendlyAppName", NAME)?;
        set(&format!("{app}\\shell\\open\\command"), "", &open_cmd)?;
        set(&format!("{app}\\SupportedTypes"), ".msg", "")?;
        set("Software\\Classes\\.msg\\OpenWithProgids", PROG_ID, "")?;
        // Capabilities: lists MailCraft in Settings > Apps > Default apps.
        set(CAPS, "ApplicationName", NAME)?;
        set(CAPS, "ApplicationDescription", "Visionneuse de messages Outlook (.msg)")?;
        set(&format!("{CAPS}\\FileAssociations"), ".msg", PROG_ID)?;
        set("Software\\RegisteredApplications", NAME, CAPS)?;
        if make_default {
            // Wins as long as the user never picked another app in "Open with" (UserChoice).
            set("Software\\Classes\\.msg", "", PROG_ID)?;
        }
        notify_shell();
        Ok(())
    }

    /// Removes everything `register` wrote (other programs' entries are left alone).
    pub fn unregister() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(format!("Software\\Classes\\{PROG_ID}"));
        let _ = hkcu.delete_subkey_all(format!("Software\\Classes\\Applications\\{}", exe_name()));
        let _ = hkcu.delete_subkey_all("Software\\MailCraft");
        if let Ok(k) = hkcu.open_subkey_with_flags("Software\\RegisteredApplications", KEY_ALL_ACCESS) {
            let _ = k.delete_value(NAME);
        }
        if let Ok(k) = hkcu.open_subkey_with_flags("Software\\Classes\\.msg\\OpenWithProgids", KEY_ALL_ACCESS) {
            let _ = k.delete_value(PROG_ID);
        }
        if let Ok(k) = hkcu.open_subkey_with_flags("Software\\Classes\\.msg", KEY_ALL_ACCESS) {
            if k.get_value::<String, _>("").is_ok_and(|v| v == PROG_ID) {
                let _ = k.delete_value("");
            }
        }
        notify_shell();
        Ok(())
    }

    pub fn current() -> MsgDefault {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let ours = |p: &str| p == PROG_ID || p.to_ascii_lowercase().contains("mailcraft");
        // The user's choice (made in "Open with" or Default apps) is what Explorer really uses.
        if let Some(p) = hkcu.open_subkey(USER_CHOICE).and_then(|k| k.get_value::<String, _>("ProgId")).ok().filter(|s| !s.is_empty()) {
            return if ours(&p) { MsgDefault::MailCraft } else { MsgDefault::Other(p) };
        }
        match RegKey::predef(HKEY_CLASSES_ROOT).open_subkey(".msg").and_then(|k| k.get_value::<String, _>("")).ok().filter(|s| !s.is_empty()) {
            Some(p) if ours(&p) => MsgDefault::Pending,
            Some(p) => MsgDefault::Other(p),
            None => MsgDefault::Nobody,
        }
    }

    /// Windows 10/11 protect the user's choice: when another app was picked explicitly, only
    /// the user can switch, from MailCraft's page in Default apps.
    pub fn open_default_apps_page() {
        let _ = open::that_detached(format!("ms-settings:defaultapps?registeredAppUser={NAME}"));
    }

    fn notify_shell() {
        #[link(name = "shell32")]
        extern "system" {
            fn SHChangeNotify(event: i32, flags: u32, a: *const std::ffi::c_void, b: *const std::ffi::c_void);
        }
        // SHCNE_ASSOCCHANGED, SHCNF_IDLIST
        unsafe { SHChangeNotify(0x0800_0000, 0, std::ptr::null(), std::ptr::null()) };
    }
}

#[cfg(windows)]
pub use assoc::{current as msg_default, open_default_apps_page, register, unregister};

/// Registers MailCraft, makes it the .msg viewer, and unless the user already confirmed it in
/// Windows, opens MailCraft's Default apps page so they can (Windows 10/11 never let a program
/// take over an extension silently). Returns the state before that confirmation.
#[cfg(windows)]
pub fn make_default() -> Result<MsgDefault, String> {
    register(true)?;
    let now = msg_default();
    if now != MsgDefault::MailCraft {
        open_default_apps_page();
    }
    Ok(now)
}

/// A readable name for a ProgID.
pub fn prog_label(p: &str) -> String {
    let l = p.to_ascii_lowercase();
    if l.starts_with("outlook.") {
        "Microsoft Outlook".into()
    } else if l.contains("mailcraft") {
        "MailCraft".into()
    } else {
        p.to_owned()
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn missing_file_is_an_error_without_dialog() {
        let p = std::env::temp_dir().join("mailcraft-does-not-exist").join("x.pdf");
        assert!(super::shell_open(&p).is_err());
    }
}
