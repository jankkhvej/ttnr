//! `stations.toml` for the CLI: loading and `ttnr stations`.

use anyhow::{Context, Result};
use ttnr_core::ListQuery;
use ttnr_sources::StationsSource;

use crate::paths;

/// Load `$XDG_CONFIG_HOME/ttnr/stations.toml` (missing = empty).
pub fn load() -> Result<StationsSource> {
    let Some(dir) = paths::config_dir() else {
        return Ok(StationsSource::default());
    };
    StationsSource::load(&dir.join("stations.toml")).context("cannot load stations")
}

/// Print the configured stations, optionally filtered.
pub fn print(search: Option<String>) -> Result<()> {
    let list = load()?;
    let q = ListQuery {
        search,
        limit: None,
    };
    let rows: Vec<_> = list
        .stations()
        .iter()
        .filter(|s| {
            q.search.as_deref().is_none_or(|n| {
                let n = n.to_lowercase();
                s.name.to_lowercase().contains(&n) || s.id.to_lowercase().contains(&n)
            })
        })
        .collect();
    if rows.is_empty() {
        println!(
            "no stations (configure ~/.config/ttnr/stations.toml; see examples/stations.toml)"
        );
        return Ok(());
    }
    let id_w = rows.iter().map(|s| s.id.chars().count()).max().unwrap_or(0);
    let src_w = rows
        .iter()
        .map(|s| s.source.chars().count())
        .max()
        .unwrap_or(0);
    for s in rows {
        println!("{:id_w$}  {:src_w$}  {}", s.id, s.source, s.name);
    }
    Ok(())
}
