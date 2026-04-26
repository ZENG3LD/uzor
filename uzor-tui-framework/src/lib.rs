//! uzor-tui: TUI rendering framework with cell-level diff.
//!
//! Provides flicker-free terminal rendering via double-buffered screen
//! that only writes changed cells. Includes a vt100 bridge for mirroring
//! PTY output from CLI tools.

pub mod style;
pub mod cell;
pub mod buffer;
pub mod backend;
pub mod screen;
pub mod vt100_bridge;
pub mod rect;
pub mod layout;
pub mod text;
pub mod border;
pub mod widget;
pub mod factory;

pub use style::{Color, Modifier, Style};
pub use cell::Cell;
pub use compact_str::CompactString;
pub use buffer::TerminalBuffer;
pub use backend::{Backend, CrosstermBackend};
pub use screen::Screen;
pub use vt100_bridge::vt100_to_buffer;
pub use rect::Rect;
pub use layout::{Constraint, Direction, split, split_equal};
pub use text::{Span, Line, Text, Alignment, Wrap};
pub use border::{BorderType, BorderChars, Borders, render_border, render_border_with_title};
pub use widget::{Widget, StatefulWidget, Block, Paragraph, List, ListState, Tabs, StatusBar};
