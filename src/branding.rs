//! Product name and identity, kept in one place so the app can be renamed in a single edit.
//! The name is two-tone in the UI: [`NAME_HEAD`] in the text colour, [`NAME_TAIL`] in the accent.

pub const NAME_HEAD: &str = "Mail";
pub const NAME_TAIL: &str = "Craft";
pub const NAME: &str = "MailCraft";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = "Felzow47";
/// Folder name under %APPDATA%.
pub const CONFIG_DIR: &str = "MailCraft";
/// ProgID for the optional Windows file association.
pub const PROG_ID: &str = "MailCraft.Message";
pub const ICON_SVG: &[u8] = include_bytes!("../assets/logo.svg");

/// The logo rasterised at `size` px (window icon, .ico frames).
pub fn icon_rgba(size: u32) -> Option<(Vec<u8>, u32, u32)> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(ICON_SVG, &usvg::Options::default()).ok()?;
    let mut pix = tiny_skia::Pixmap::new(size, size)?;
    let s = size as f32 / tree.size().width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(s, s), &mut pix.as_mut());
    // tiny-skia stores premultiplied alpha.
    let mut rgba = pix.take();
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some((rgba, size, size))
}

/// `mailcraft --make-icons`: writes assets/mailcraft.ico from the SVG logo.
pub fn make_icons(dir: &std::path::Path) -> Result<(), String> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let mut frames = Vec::new();
    for s in [16u32, 20, 24, 32, 40, 48, 64, 128, 256] {
        let (rgba, w, h) = icon_rgba(s).ok_or("logo illisible")?;
        frames.push(IcoFrame::as_png(&rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?);
    }
    let f = std::fs::File::create(dir.join("mailcraft.ico")).map_err(|e| e.to_string())?;
    IcoEncoder::new(f).encode_images(&frames).map_err(|e| e.to_string())?;
    let (rgba, w, h) = icon_rgba(256).ok_or("logo illisible")?;
    image::save_buffer(dir.join("icon_256.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())
}
