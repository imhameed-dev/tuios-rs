//! Events flowing into the single-threaded app loop.

use crate::input::Key;
use crate::layout::WindowId;

#[derive(Debug)]
pub enum InputEvent {
    Key(Key),
    Paste(String),
    /// New terminal size as (columns, rows).
    Resize(u16, u16),
    /// Keyboard input ended (terminal closed): shut down.
    Eof,
}

#[derive(Debug)]
pub enum AppEvent {
    Input(InputEvent),
    /// Bytes a pane's process wrote.
    Output(WindowId, Vec<u8>),
    /// A pane's process ended (its PTY reached end of file).
    Exit(WindowId),
}
