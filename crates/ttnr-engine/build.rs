//! Locate libmpv via pkg-config.
//!
//! `libmpv2-sys` only emits `-lmpv` without a search path, and Homebrew's
//! prefix on Apple Silicon (`/opt/homebrew/lib`) is not on the default
//! linker path, so we add the search path from `mpv.pc` here.

fn main() {
    if let Err(e) = pkg_config::Config::new()
        .atleast_version("2.2")
        .cargo_metadata(true)
        .probe("mpv")
    {
        println!(
            "cargo::warning=pkg-config could not find libmpv ({e}); install it with `brew install mpv pkgconf`"
        );
    }
}
