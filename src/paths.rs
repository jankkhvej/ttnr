//! XDG-style locations for ttnr's files (ARCHITECTURE.md, "Config & files";
//! XDG Base Directory spec 0.8, docs/RESEARCH.md).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `$XDG_STATE_HOME/ttnr`, else `$HOME/.local/state/ttnr` (logs live here).
pub fn state_dir() -> Option<PathBuf> {
    xdg_dir(
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME"),
        ".local/state",
    )
}

/// Resolve an XDG base directory for ttnr. Per the spec, an unset, empty or
/// relative `$XDG_*` value is ignored in favour of `$HOME/<fallback>`.
fn xdg_dir(xdg: Option<OsString>, home: Option<OsString>, fallback: &str) -> Option<PathBuf> {
    if let Some(base) = xdg.filter(|x| Path::new(x).is_absolute()) {
        return Some(PathBuf::from(base).join("ttnr"));
    }
    let home = home.filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join(fallback).join("ttnr"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(s.into())
    }

    #[test]
    fn xdg_value_wins_when_absolute() {
        assert_eq!(
            xdg_dir(os("/x/state"), os("/home/u"), ".local/state"),
            Some(PathBuf::from("/x/state/ttnr"))
        );
    }

    #[test]
    fn unset_empty_or_relative_xdg_falls_back_to_home() {
        for xdg in [None, os(""), os("rel/state")] {
            assert_eq!(
                xdg_dir(xdg, os("/home/u"), ".local/state"),
                Some(PathBuf::from("/home/u/.local/state/ttnr"))
            );
        }
    }

    #[test]
    fn no_home_no_dir() {
        assert_eq!(xdg_dir(None, None, ".local/state"), None);
        assert_eq!(xdg_dir(None, os(""), ".local/state"), None);
    }
}
