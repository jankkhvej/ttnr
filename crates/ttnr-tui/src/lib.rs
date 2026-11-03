//! ratatui user interface for ttnr: views, input handling and keymap.
//! Renders the core `AppState` through a theme; never blocks on network
//! or audio.

pub mod fx;
pub mod keys;
pub mod printout;
pub mod serial;
pub mod view;

pub use fx::{Fx, FxFrame};
pub use keys::{Keymap, KeymapError};
pub use printout::Printout;
pub use serial::SlowScreen;
pub use view::{Extras, draw, draw_with, draw_with_printout};
