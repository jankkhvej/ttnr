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
        }
        Err(e) => println!(
            "cargo::warning=pkg-config could not find libmpv ({e}); `brew install mpv pkgconf`"
        ),
    }
}
