//! Input modes and key-binding scaffolding.
//!
//! Right now the actual key→action mapping lives in `app.rs` because actions
//! need access to app state. The types here document the modes and define a
//! place where configurable keybindings could be loaded from `config.toml` in
//! the future.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Normal,
    /// Editing a single text field (title, description, etc.)
    Insert,
    /// `:` command bar.
    Command,
    /// `/` search bar.
    Search,
    /// `?` help overlay.
    Help,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Search => "SEARCH",
            Mode::Help => "HELP",
        }
    }
}

/// Which top-level pane currently has focus. Tab cycles between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Tasks,
    Details,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Focus::Sidebar => Focus::Tasks,
            Focus::Tasks => Focus::Details,
            Focus::Details => Focus::Sidebar,
        }
    }
}

/// What the current Insert-mode text buffer is editing. Lets us know where to
/// commit the buffer when the user presses Enter.
#[derive(Debug, Clone)]
pub enum EditTarget {
    /// Add a new top-level task with this title.
    NewTask,
    /// Add a new subtask under the given parent id.
    NewSubtask { parent_id: i64 },
    /// Edit the title of the task with this id.
    EditTitle { task_id: i64 },
    /// Edit description.
    EditDescription { task_id: i64 },
}
