//! Builds the fonts the kit ships into `kition-note`: the faces the editor's web build copies into
//! the package (`web/scripts/build-engine.sh`; `kition_note_fonts::CHECKOUT_FILES`), from the same
//! fonts folder: `KITION_FONTS_DIR`, else `kition-fonts` beside this repository. The executable
//! then lays notes out and draws them with those faces wherever it runs, with no environment
//! variable. Without them the build fails and says which file is missing and where to put it
//! (docs/development.md, "Fonts").

use std::fmt::Write as _;
use std::path::PathBuf;

/// `kition_note_fonts::CHECKOUT_FILES` (a test checks they agree).
const FONTS: &[&str] = &[
    "fonts/inter/Inter-Variable.ttf",
    "fonts/jetbrains-mono/JetBrainsMono-Variable.ttf",
    "fonts/droid-sans-fallback/DroidSansFallbackFull.ttf",
    "fonts/noto-color-emoji/Noto-COLRv1.ttf",
];

fn main() {
    println!("cargo::rerun-if-env-changed=KITION_FONTS_DIR");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let dir = std::env::var_os("KITION_FONTS_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../../kition-fonts"));
    let mut src = String::from(
        "/// The font files the kit ships, built in: their path in a kition-fonts checkout and bytes.\nconst SHIPPED: &[(&str, &[u8])] = &[\n",
    );
    for font in FONTS {
        let path = dir.join(font);
        match std::fs::canonicalize(&path) {
            Ok(path) => {
                println!("cargo::rerun-if-changed={}", path.display());
                let _ = writeln!(src, "    ({font:?}, include_bytes!({:?})),", path.display().to_string());
            }
            Err(_) => {
                println!(
                    "cargo::error=the font file {} is missing: set KITION_FONTS_DIR to a folder that holds {font} (docs/development.md, \"Fonts\", lists each file and where to download it)",
                    path.display()
                );
                if let Some(folder) = path.parent().filter(|folder| folder.is_dir()) {
                    println!("cargo::rerun-if-changed={}", folder.display());
                }
            }
        }
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("shipped_fonts.rs");
    if let Err(error) = std::fs::write(&out, src) {
        println!("cargo::error=writing {}: {error}", out.display());
    }
}
