//! Ensure the embedded web-bundle directory exists so rust-embed's
//! `#[folder = "../web/dist"]` compiles even before `npm run build` has run
//! (fresh checkout / CI). The real bundle is produced by `scripts/build.sh`
//! (or `npm run build`); here we only guarantee the directory is present.

use std::path::PathBuf;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let dist = PathBuf::from(manifest).join("../web/dist");
    let _ = std::fs::create_dir_all(&dist);
    println!("cargo:rerun-if-changed=build.rs");
}
