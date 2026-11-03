//! ratatui user interface for ttnr: views, input handling and keymap.
//! Renders the core `AppState` through a theme; never blocks on network
//! or audio.

pub mod keys;
pub mod printout;
pub mod view;

pub use keys::{Keymap, KeymapError};
pub use printout::Printout;
pub use view::{draw, draw_with_printout};
