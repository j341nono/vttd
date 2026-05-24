use crate::error::{AppError, AppResult};

/// Parsed `:` command from the command bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    Write,
    Help,
    Export {
        scope: ExportScope,
        file: Option<String>,
    },
    /// Clear active filters / search.
    ClearFilters,
    /// Add a (predefined) tag to the selected task.
    AddTag(String),
    /// Remove a tag from the selected task.
    RemoveTag(String),
    /// Set due date on the selected task. "YYYY-MM-DD", "today", "tomorrow", or "none".
    Due(String),
    /// Set priority on the selected task.
    Priority(String),
    /// Set status on the selected task.
    Status(String),
    /// Set description on the selected task. Empty argument clears it.
    Description(String),
    /// Set the project of the selected task by project name. "none" clears it.
    SetTaskProject(String),
    /// Create a new project.
    NewProject(String),
    /// Start a pomodoro on the selected task.
    Pomo,
    /// Stop the current pomodoro.
    PomoStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportScope {
    Today,
    All,
    Visible,
    Project(String),
}

pub fn parse(input: &str) -> AppResult<Command> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::InvalidCommand("empty".into()));
    }
    let mut parts = input.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match head {
        "q" | "quit" => Ok(Command::Quit),
        "w" | "write" => Ok(Command::Write),
        "wq" => Ok(Command::Quit),
        "help" => Ok(Command::Help),
        "export" => parse_export(rest),
        "clear" => Ok(Command::ClearFilters),
        "addtag" => require_rest(rest, "usage: :addtag <name>").map(Command::AddTag),
        "rmtag" => require_rest(rest, "usage: :rmtag <name>").map(Command::RemoveTag),
        "due" => require_rest(rest, "usage: :due YYYY-MM-DD|today|tomorrow|none").map(Command::Due),
        "pri" | "priority" => {
            require_rest(rest, "usage: :pri high|medium|low").map(Command::Priority)
        }
        "status" => {
            require_rest(rest, "usage: :status todo|doing|waiting|done").map(Command::Status)
        }
        "desc" | "description" => Ok(Command::Description(rest.to_string())),
        "project" | "proj" => {
            require_rest(rest, "usage: :project <name|none>").map(Command::SetTaskProject)
        }
        "newproject" | "newproj" => {
            require_rest(rest, "usage: :newproject <name>").map(Command::NewProject)
        }
        "pomo" => Ok(Command::Pomo),
        "pomostop" => Ok(Command::PomoStop),
        other => Err(AppError::InvalidCommand(other.into())),
    }
}

fn require_rest(rest: &str, msg: &str) -> AppResult<String> {
    if rest.is_empty() {
        Err(AppError::InvalidCommand(msg.into()))
    } else {
        Ok(rest.to_string())
    }
}

fn parse_export(rest: &str) -> AppResult<Command> {
    if rest.is_empty() {
        return Ok(Command::Export {
            scope: ExportScope::Visible,
            file: None,
        });
    }
    let mut tokens = rest.split_whitespace();
    let scope_tok = tokens.next().unwrap();
    let scope = match scope_tok {
        "today" => ExportScope::Today,
        "all" => ExportScope::All,
        "visible" => ExportScope::Visible,
        "project" => {
            let name = tokens.next().ok_or_else(|| {
                AppError::InvalidCommand("usage: :export project <name> [file]".into())
            })?;
            ExportScope::Project(name.to_string())
        }
        other => {
            return Err(AppError::InvalidCommand(format!(
                "unknown export scope: {other}"
            )))
        }
    };
    let file = tokens.next().map(|s| s.to_string());
    Ok(Command::Export { scope, file })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic() {
        assert_eq!(parse("q").unwrap(), Command::Quit);
        assert_eq!(parse("quit").unwrap(), Command::Quit);
        assert_eq!(parse("help").unwrap(), Command::Help);
    }

    #[test]
    fn parses_export_today() {
        assert_eq!(
            parse("export today").unwrap(),
            Command::Export {
                scope: ExportScope::Today,
                file: None,
            }
        );
    }

    #[test]
    fn parses_export_all_to_file() {
        assert_eq!(
            parse("export all out.md").unwrap(),
            Command::Export {
                scope: ExportScope::All,
                file: Some("out.md".to_string()),
            }
        );
    }

    #[test]
    fn parses_export_project() {
        assert_eq!(
            parse("export project 論文誌").unwrap(),
            Command::Export {
                scope: ExportScope::Project("論文誌".to_string()),
                file: None,
            }
        );
    }

    #[test]
    fn parses_due_priority_status() {
        assert_eq!(parse("due today").unwrap(), Command::Due("today".into()));
        assert_eq!(parse("pri high").unwrap(), Command::Priority("high".into()));
        assert_eq!(
            parse("status doing").unwrap(),
            Command::Status("doing".into())
        );
    }

    #[test]
    fn rejects_unknown() {
        assert!(parse("foobar").is_err());
        assert!(parse("").is_err());
        assert!(parse("addtag").is_err());
    }
}
