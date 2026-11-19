// Embeds the Windows icon and version info (MSVC: rc.exe through winresource).
fn main() {
    icons();
    println!("cargo:rerun-if-changed=assets/mailcraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || !std::path::Path::new("assets/mailcraft.ico").exists() {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/mailcraft.ico")
        .set("FileDescription", "MailCraft - visionneuse de messages Outlook")
        .set("ProductName", "MailCraft")
        .set("LegalCopyright", "MIT")
        .set_language(0x040C);
    if let Err(e) = res.compile() {
        println!("cargo:warning=ressources Windows non intégrées : {e}");
    }
}

/// Embed every Lucide SVG of assets/icons as (name, bytes).
fn icons() {
    println!("cargo:rerun-if-changed=assets/icons");
    let mut names: Vec<String> = std::fs::read_dir("assets/icons")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".svg"))
        .collect();
    names.sort();
    let mut out = String::from("pub static ICONS: &[(&str, &[u8])] = &[\n");
    for n in names {
        let stem = n.trim_end_matches(".svg");
        out.push_str(&format!("    (\"{stem}\", include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/icons/{n}\"))),\n"));
    }
    out.push_str("];\n");
    let dst = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("icon_data.rs");
    std::fs::write(dst, out).unwrap();
}
