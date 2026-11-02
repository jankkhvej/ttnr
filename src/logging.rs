//! File logging with `tracing` (never to the terminal UI's stdout).
//!
//! Daily-rotated `ttnr.YYYY-MM-DD.log` in the state directory, last 7
//! kept. Filter via `TTNR_LOG` (`tracing_subscriber` `Targets` syntax, e.g.
//! `debug` or `info,mpv=warn`); default `info`.

use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::paths;

/// Install the file logger. Logging problems never stop the app: they are
/// reported once on stderr and logging stays off.
pub fn init() {
    let Some(dir) = paths::state_dir() else {
        eprintln!("ttnr: no $HOME/$XDG_STATE_HOME – file logging disabled");
        return;
    };
    let appender = std::fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|()| {
            RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("ttnr")
                .filename_suffix("log")
                .max_log_files(7)
                .build(&dir)
                .map_err(|e| e.to_string())
        });
    let appender = match appender {
        Ok(a) => a,
        Err(e) => {
            eprintln!("ttnr: file logging disabled ({}): {e}", dir.display());
            return;
        }
    };
    let filter = std::env::var("TTNR_LOG")
        .ok()
        .and_then(|s| s.parse::<Targets>().ok())
        .unwrap_or_else(|| Targets::new().with_default(Level::INFO));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(appender))
        .with(filter)
        .try_init();
}
