# MailCraft - Outlook message (.msg) viewer

<p align="center"><img src="assets/icon_256.png" width="128" alt="MailCraft logo"></p>

<p align="center"><a href="https://github.com/Felzow47/MailCraft/actions/workflows/ci.yml"><img src="https://github.com/Felzow47/MailCraft/actions/workflows/ci.yml/badge.svg" alt="CI"></a> <a href="https://github.com/Felzow47/MailCraft/releases/latest"><img src="https://img.shields.io/github/v/release/Felzow47/MailCraft" alt="Latest release"></a></p>

MailCraft is a lightweight native viewer for **Outlook `.msg`** files, written in Rust
(egui + WebView2). It shows them **the way your mail client does**: subject, sender with avatar,
recipients (To / Cc / Bcc), date, attachments, the original HTML formatting and inline pictures.
It works offline and does not need Outlook. It comes as an installer or as a portable exe.

The interface follows the ArtCraft suite style, like ModelCraft (layout taken from PdfCraft,
MIT/Apache, see `NOTICE.md`). MailCraft is an **independent** project, not affiliated with ArtCraft
or Microsoft. The interface is in **French and English** (it follows the Windows language;
the choice is in Settings).

## Download

Get both files from the [latest release](https://github.com/Felzow47/MailCraft/releases/latest).

| Version | File | For |
|---|---|---|
| **Installer** | `MailCraft-Setup-<version>.exe` | Everyday use: Start menu, uninstaller, default viewer for `.msg` |
| **Portable** | `MailCraft-<version>-Portable.exe` | A single file to copy anywhere (USB stick, PC without admin rights) |

- **Installer**: installs for the current user, **no administrator rights**, into
  `%LOCALAPPDATA%\Programs\MailCraft`. The wizard (French or English) offers to make MailCraft the
  **default viewer for `.msg` files** (checked by default) and to create a desktop icon. Uninstalling
  removes everything, including the file association.
- **Portable**: as long as the file name contains "Portable" (or a `MailCraft-data` folder sits next
  to it), MailCraft keeps its settings and WebView2 cache in `MailCraft-data\` beside the exe and
  writes nothing to `%APPDATA%`.

Requirements: 64-bit Windows 10 or 11, with the **WebView2** runtime. Windows 11 and up-to-date
Windows 10 already include it. Without WebView2, MailCraft still shows the text of each message.

## Features

**Inbox**
- Open one or more messages (one tab each), or **a whole folder**: its `.msg` files become an inbox
  sorted by date and grouped like Outlook does (Today, Yesterday, This week, months…).
- Each list row shows a text preview and, when set, a paperclip, follow-up flag or importance mark.
  Unread messages are in bold.
- Instant **search** in subject, people and text, ignoring accents. Filters:
  *All / Unread / Attachments*.
- Keyboard navigation, drag and drop of files or folders, recent files.

**Reading pane**
- Outlook-style header: avatar with initials (for example `georges93@email.fr` shows as **GE**; addresses in this README are made up), "sent by … on behalf of …", To / Cc / Bcc recipients
  (right-click to copy the address or write to them), date, banners for importance, follow-up and
  drafts.
- **Message body** rendered by WebView2 (the Edge engine), so it looks as it does in Outlook:
  - the original HTML (`PR_HTML`), or the **HTML recovered from the compressed RTF** (`\fromhtml1`),
    which is what most Outlook messages contain, or plain text with clickable links;
  - **inline pictures** (`cid:`) appear in the text and are left out of the attachment list;
  - **remote pictures are blocked** by default, as in Outlook, with a "Download pictures" banner;
  - no scripts or forms run; links open in the default browser.
- **Attachments**: one card per file, with an icon by type (PDF, Word, Excel, picture, archive,
  CAD…). Click to open, right-click to save, or "Save all". An **attached message** (`.msg` inside
  the `.msg`) opens in a new tab.
- **Export all media**: writes every file the message carries (attachments, inline pictures, hidden
  ones) into a folder, with one sub-folder per attached message. For a folder tab, "Export the
  folder's media" does the same for every message shown (search and filter applied), with one
  sub-folder per message (`2019-11-26 Subject`), in the background.
- **Reply / Reply all / Forward**: opens your default mail client (`mailto:` link) with the
  recipients, an `RE :` or `TR :` subject and the original message header.
- **Print**, message **zoom**, **Internet headers** and raw **MAPI properties**.

**Settings**
- Language: system, Français or English.
- Light, dark or system theme, and accent colour.
- Automatic download of remote pictures (off by default).
- Text preview in the message list.
- **Default viewer for `.msg` files**: shows the current state, with "Set MailCraft as default" and
  "Remove the association" buttons. This applies to your account only and needs no administrator
  rights.

> **Why Windows asks for confirmation.** Windows 10 and 11 never let a program become the default
> app for a file type without the user's consent, especially when another app (Outlook, the new
> Outlook…) is already registered. MailCraft therefore registers as a `.msg` viewer and then opens
> its page in *Settings > Apps > Default apps*: choose MailCraft for `.msg` there. Otherwise Windows
> asks on the first double-click: pick MailCraft, then "Always".

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+O / Ctrl+Shift+O | Open a message / a folder |
| Ctrl+W, Ctrl+Tab | Close the tab, go to the next tab |
| ↑ ↓ Home End, Enter | Move through the list, open the message in a tab |
| Ctrl+F | Search the folder |
| Ctrl+R / Ctrl+Shift+R / Ctrl+Shift+F | Reply / Reply all / Forward |
| Ctrl+P | Print |
| Ctrl++ / Ctrl+- / Ctrl+0 | Message zoom |
| F1 | Help |

## Command line

```
mailcraft [files.msg | folders…] [--lang fr|en] [--theme dark|light] [--size WxH]
mailcraft --diag <file.msg | folder>              # headless read, detailed report
mailcraft --register [--default] | --unregister   # .msg association (used by the installer)
mailcraft --make-icons assets                     # rebuilds mailcraft.ico and icon_256.png from logo.svg
```

## The MSG format

The reader (`src/msg/`) was written for MailCraft from Microsoft's public specifications:
- **[MS-OXMSG]**: OLE container, `__substg1.0_*` and `__properties_version1.0` properties,
  recipients, attachments, embedded messages;
- **[MS-OXRTFCP]**: compressed RTF (LZFu);
- **[MS-OXRTFEX]**: de-encapsulation of the HTML stored inside the RTF.

HTML pages saved with a `.msg` extension are recognised and shown as such, with a warning and their
remote content blocked.

Tested on a set of 1,723 real files: `mailcraft --diag <folder>` reads them all in about 11 s,
**with no failures**. The set contains 1,650 HTML bodies recovered from RTF, 47 native HTML bodies,
12 plain-text bodies and 14 HTML pages, with 6,097 attachments, 2,422 inline pictures and 16
attached messages.

## Building

Requires Windows, the Rust MSVC toolchain and [Inno Setup 6](https://jrsoftware.org/isinfo.php)
for the installer:

```
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

The script builds in release mode, then writes to `dist\`:
- `MailCraft-<version>-Portable.exe`;
- `MailCraft-Setup-<version>.exe`, built from `installer/mailcraft.iss`. Without Inno Setup, only
  the portable version is produced.

Every push to `main` runs the same script on GitHub Actions (`.github/workflows/ci.yml`) and publishes the release `v<version>` with both files. The version comes from `Cargo.toml` (`major.minor.fix`: a new feature bumps the minor number, a
bug fix or small change the last one). Unit tests (RTF parser, HTML, formatting) run with
`cargo test --release`.

## Layout

| Folder / file | Role |
|---|---|
| `src/msg/` | MSG reader: MAPI properties, LZFu, RTF to HTML |
| `src/html.rs`, `src/webview.rs` | Message page (CSP, `cid:`) and WebView2 through `wry` |
| `src/mailview.rs` | Message list and reading pane |
| `src/chrome.rs`, `src/home.rs`, `src/dialogs.rs` | Tabs, toolbar, home page, dialogs |
| `src/theme.rs`, `src/widgets.rs`, `src/icons.rs` | ArtCraft style shared with ModelCraft |
| `src/export.rs` | "Export all media" for a message or a folder |
| `src/i18n.rs`, `src/i18n_en.rs` | French / English interface (French source, English table) |
| `src/platform.rs` | Portable mode, `.msg` association, Windows integration |
| `installer/mailcraft.iss` | Inno Setup installer |

## License

MIT, see [LICENSE](LICENSE). Attributions (PdfCraft, Lucide icons, Inter and JetBrains Mono fonts) are in `NOTICE.md` and
`licenses/`.
