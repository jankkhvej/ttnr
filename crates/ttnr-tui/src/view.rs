//! Rendering [`AppState`] (minimal main view; full views and themes follow
//! in M3 tasks 2 and 4).

use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ttnr_core::app::{AppState, Status};

/// Draw the whole UI for `state`.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let mut lines = vec![Line::from(
        state
            .station
            .as_ref()
            .map_or("no station – press s or o", |s| s.name.as_str())
            .to_owned(),
    )];
    lines.push(Line::from(status_text(&state.status)));
    if let Some(now) = &state.now {
        let text = match (&now.artist, &now.title) {
            (Some(a), Some(t)) => format!("{a} – {t}"),
            (None, Some(t)) => t.clone(),
            _ => String::new(),
        };
        lines.push(Line::from(text));
    }
    if let Some(info) = &state.stream {
        lines.push(Line::from(info.summary()));
    }
    lines.push(Line::from(format!(
        "vol {}%{}",
        state.volume,
        if state.muted { " (muted)" } else { "" }
    )));
    frame.render_widget(Paragraph::new(lines), frame.area());
}

/// Short status text.
pub fn status_text(status: &Status) -> String {
    match status {
        Status::Idle => "idle".into(),
        Status::Resolving => "resolving…".into(),
        Status::Loading => "loading…".into(),
        Status::Buffering(Some(p)) => format!("buffering {p}%"),
        Status::Buffering(None) => "buffering…".into(),
        Status::Playing => "playing".into(),
        Status::Paused => "paused".into(),
        Status::Reconnecting(why) => format!("reconnecting: {why}"),
        Status::Ended => "stream ended".into(),
        Status::Error(e) => format!("error: {e}"),
    }
}
