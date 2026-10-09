//! Embeds the web UI into the binary when `ui/` holds a built bundle.
//!
//! In the repository `ui/` is empty: the server finds the UI in
//! `crates/czar-web-react/dist` instead. Before publishing,
//! `scripts/embed-ui.sh` copies the built UI into `ui/`, so the crate on
//! crates.io (and everything installed from it) carries the UI.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let ui = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui");
    println!("cargo:rerun-if-changed={}", ui.display());
    let mut files = Vec::new();
    if ui.join("index.html").is_file() {
        collect(&ui, &ui, &mut files);
    }
    files.sort();
    // A content hash names the extracted folder, so a new build never serves an old UI.
    let mut hash: u64 = 0xcbf29ce484222325;
    for (rel, abs) in &files {
        for byte in rel.bytes().chain(std::fs::read(abs).unwrap()) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let mut out = format!(
        "/// Content hash of the embedded UI.\npub static ID: &str = \"{hash:016x}\";\n/// The embedded UI bundle: (path inside the bundle, contents).\npub static FILES: &[(&str, &[u8])] = &[\n"
    );
    for (rel, abs) in &files {
        println!("cargo:rerun-if-changed={}", abs.display());
        writeln!(
            out,
            "    ({rel:?}, include_bytes!({:?})),",
            abs.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_ui.rs");
    std::fs::write(dest, out).unwrap();
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            files.push((rel, path));
        }
    }
}
