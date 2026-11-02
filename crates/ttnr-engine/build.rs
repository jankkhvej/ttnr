//! Locate libmpv via pkg-config.
//!
//! `libmpv2-sys` only emits `-lmpv` without a search path, and Homebrew's
//! prefix on Apple Silicon (`/opt/homebrew/lib`) is not on the default
//! linker path, so we add the search path from `mpv.pc` here.
//!
//! A libmpv built by `scripts/build-libmpv.zsh` into
//! `third_party/mpv/prefix` takes precedence over Homebrew's (ADR-015):
//! it carries the CoreAudio fix missing from Homebrew's mpv 0.41.0.

use std::path::Path;

fn main() {
    let local_pc =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/mpv/prefix/lib/pkgconfig");
    println!(
        "cargo::rerun-if-changed={}",
        local_pc.join("mpv.pc").display()
    );
    if local_pc.join("mpv.pc").is_file() {
        let path = match std::env::var_os("PKG_CONFIG_PATH") {
            Some(old) if !old.is_empty() => {
                let mut p = local_pc.into_os_string();
                p.push(":");
                p.push(old);
                p
            }
            _ => local_pc.into_os_string(),
        };
        // SAFETY: build scripts are single-threaded; nothing else reads or
        // writes the environment concurrently.
        unsafe { std::env::set_var("PKG_CONFIG_PATH", path) };
    }

    match pkg_config::Config::new()
        .atleast_version("2.2")
        .cargo_metadata(true)
        .probe("mpv")
    {
        Ok(lib) => {
            for dir in &lib.link_paths {
                println!("cargo::warning=libmpv from {}", dir.display());
            }
        }
        Err(e) => println!(
            "cargo::warning=pkg-config could not find libmpv ({e}); run scripts/build-libmpv.zsh or `brew install mpv pkgconf`"
        ),
    }
}
