//! Locate libmpv via pkg-config.
//!
//! `libmpv2-sys` only emits `-lmpv` without a search path, and Homebrew's
//! prefix on Apple Silicon (`/opt/homebrew/lib`) is not on the default
//! linker path, so we add the search path from `mpv.pc` here. Homebrew's
//! mpv ≥ 0.42 carries the CoreAudio fix the retired local build had
//! (ADR-049).

fn main() {
    match pkg_config::Config::new()
        .atleast_version("2.2")
        .cargo_metadata(true)
        .probe("mpv")
    {
        Ok(lib) => {
            for dir in &lib.link_paths {
                println!("cargo::warning=libmpv from {}", dir.display());
            }
            // Probe again after `brew upgrade mpv` (the old Cellar's mpv.pc
            // disappears or changes).
            if let Ok(dir) = pkg_config::get_variable("mpv", "pcfiledir") {
                println!("cargo::rerun-if-changed={dir}/mpv.pc");
            }
            // libmpv client API 2.6 = mpv 0.42 (2.5 = mpv 0.41).
            let minor = lib
                .version
                .split('.')
                .nth(1)
                .and_then(|m| m.parse::<u32>().ok());
            if lib.version.starts_with("2.") && minor.is_some_and(|m| m < 6) {
                println!(
                    "cargo::warning=libmpv {} is older than mpv 0.42: CoreAudio output may fail and fall back to AVFoundation (ADR-049); `brew upgrade mpv`",
                    lib.version
                );
            }
        }
        Err(e) => println!(
            "cargo::warning=pkg-config could not find libmpv ({e}); `brew install mpv pkgconf`"
        ),
    }
}
