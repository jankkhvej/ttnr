//! ratatui user interface for ttnr: views, input handling and keymap.
//! Renders the core `AppState` through a theme; never blocks on network
//! or audio.

pub mod keys;
pub mod view;

pub use keys::{Keymap, KeymapError};
pub use view::draw;
