# MailCraft - notices and attributions

MailCraft is an **independent** project. It is neither an official product of the ArtCraft suite
nor affiliated with the ArtCraft Team or Microsoft (Outlook, Exchange, WebView2).
The names ArtCraft, Outlook, Exchange… belong to their owners.
No ArtCraft logo is used: the MailCraft logo (`assets/logo.svg`) is an original creation.

The `.msg` reader (`src/msg/`) is an independent implementation written from Microsoft's public
Open Specifications ([MS-OXMSG], [MS-OXRTFCP], [MS-OXRTFEX]); it reads the OLE container with
the `cfb` library (MIT). No Outlook code or library is used.

## Reused code

| Item | Origin | License |
|---|---|---|
| Interface layout (tab bar, toolbar, home page, design tokens, widgets) - `src/{theme,widgets,icons,chrome,home}.rs`, through ModelCraft | [PdfCraft](https://github.com/storytold/pdfcraft), © 2026 ArtCraft Team and PdfCraft contributors | MIT OR Apache-2.0 - `licenses/LICENSE-PdfCraft-MIT.txt` |

## Bundled third-party resources

| Item | License | File |
|---|---|---|
| [Lucide](https://lucide.dev) icons (`assets/icons/*.svg`) | ISC | `licenses/LICENSE-lucide.txt` |
| Inter font (`Inter-*.ttf`) | SIL OFL 1.1 | `licenses/OFL-Inter.txt` |
| JetBrains Mono font | SIL OFL 1.1 | `licenses/OFL-JetBrainsMono.txt` |

The message body is displayed by the Microsoft Edge **WebView2** runtime installed with Windows,
through the `wry` library (MIT OR Apache-2.0); it is not redistributed with MailCraft.

The other Rust dependencies (egui, eframe, wgpu, wry, cfb, encoding_rs, chrono, resvg, image, rfd,
open, serde…) are under MIT / Apache-2.0 / zlib / BSD licenses; the full list is in `Cargo.lock`.
