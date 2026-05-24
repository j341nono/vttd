# todotui

A Vim-friendly terminal ToDo manager for researchers and developers, built
with Rust and [Ratatui](https://ratatui.rs/).

`todotui` lets you keep your tasks in the terminal instead of pasting them
into a Slack DM. Tasks live in a local SQLite database; tags and projects
live in a TOML config you control.

## Features

- Vim-style modal interface (Normal / Insert / Command / Search / Help).
- Tasks with id, parent, project, status (todo / doing / waiting / done),
  priority (high / medium / low), due date, tags, description, and timestamps.
- **Subtasks** rendered as an indented, collapsible tree.
- **Projects** with a left-sidebar filter.
- **Predefined tags** declared in `config.toml` — no accidental tag creation.
- Views: All, Today, Done, Log, plus per-project and per-tag views.
  - Today view = overdue + due-today + doing tasks.
  - Log view = completed tasks grouped by date (useful for daily reports).
- Substring search across title + description (`/`).
- `:` command bar for actions on the selected task (status, priority, due,
  tags, project) and bulk operations (export, new project, pomodoro).
- **Markdown export** of any view (`:export today`, `:export all`,
  `:export project <name> [file]`).
- Optional **pomodoro timer** that runs in the bottom bar (`:pomo`).

## Installation

Requires a Rust toolchain (stable). The `rusqlite` dependency is built with
the `bundled` feature, so you do not need a system SQLite library.

```sh
git clone <this repo>
cd vttd
cargo build --release
./target/release/todotui
```

Or just run from source:

```sh
cargo run
```

## Data paths

`todotui` follows XDG-style layout on every supported platform:

- Config: `~/.config/todotui/config.toml`
- Database: `~/.local/share/todotui/tasks.db`

Both are created on first launch with a default config and sample tasks.

## Keybindings (Normal mode)

| Key      | Action                                                      |
| -------- | ----------------------------------------------------------- |
| `j` / `k` | Move down / up                                              |
| `h` / `l` | Left pane / right pane (or collapse / expand a parent)      |
| `gg` / `G` | Jump to top / bottom of the list                           |
| `tab`    | Cycle focus: sidebar → tasks → details                      |
| `enter`  | Expand / collapse a parent task, or activate sidebar item   |
| `a`      | Add a new task                                              |
| `o`      | Add a subtask under the selected task                       |
| `i`      | Edit the selected task's title                              |
| `D`      | Edit the selected task's description                        |
| `space`  | Toggle complete / incomplete                                |
| `s`      | Cycle status (todo → doing → waiting → done)                |
| `p`      | Cycle priority (high → medium → low)                        |
| `dd`     | Delete the selected task (subtasks cascade)                 |
| `/`      | Enter search mode                                           |
| `c`      | Clear the active search                                     |
| `:`      | Enter command mode                                          |
| `?`      | Open help overlay                                           |
| `esc`    | Leave Insert / Command / Search / Help and return to Normal |
| `q`      | Quit                                                        |
| `Ctrl-c` | Force quit                                                  |

Keybindings live in `App::on_key`. A `keys` table is reserved in
`config.toml` so user-defined remappings can be added later without
refactoring the input layer.

## Commands

All commands are entered after pressing `:`.

| Command                                        | Description                                                |
| ---------------------------------------------- | ---------------------------------------------------------- |
| `:q` / `:quit` / `:wq`                         | Quit                                                       |
| `:help`                                        | Open help overlay                                          |
| `:clear`                                       | Clear search + return to the **All** view                  |
| `:due today|tomorrow|YYYY-MM-DD|none`          | Set due date on the selected task                          |
| `:pri high|medium|low`                         | Set priority on the selected task                          |
| `:status todo|doing|waiting|done`              | Set status on the selected task                            |
| `:desc <text>`                                 | Set description on the selected task (empty clears it)     |
| `:addtag <name>`                               | Add a predefined tag to the selected task                  |
| `:rmtag <name>`                                | Remove a tag from the selected task                        |
| `:project <name|none>`                         | Assign the selected task to a project (or clear it)        |
| `:newproject <name>`                           | Create a new project                                       |
| `:export today|all|visible`                    | Export Markdown (saved to `~/.local/share/todotui/last_export.md` if no path) |
| `:export today|all|visible <file>`             | Export Markdown to a specific file                         |
| `:export project <name> [file]`                | Export a project's open tasks                              |
| `:pomo`                                        | Start a focus timer on the selected task                   |
| `:pomostop`                                    | Stop the focus timer                                       |

The Markdown export uses checklist syntax:

```markdown
## Today

- [ ] 論文誌執筆
  - [ ] 関連研究を書く
  - [ ] 実験結果を整理する
- [ ] PBL資料作り
```

## Configuration

The config file is regenerated with defaults if it does not exist. The
default looks like:

```toml
# todotui configuration
# Tags are predefined here; the app will not create tags on the fly.

[tags]
default = ["研究", "授業", "開発", "就活", "生活", "読書"]

[projects]
default = ["論文誌", "PBL", "研究", "個人開発"]

[ui]
show_done_in_all = false
week_days = 7

[keys]
# Reserved for future user remapping.

[pomodoro]
minutes = 25
```

- `[tags].default` — the **only** tags `:addtag` will accept. Edit the list
  and restart `todotui` to add more.
- `[projects].default` — projects ensured on every startup. Additional
  projects created via `:newproject` persist in the database.
- `[ui].show_done_in_all` — include done tasks in the All / Project / Tag
  views (Done view always shows them).
- `[ui].week_days` — horizon for the "this week" due filter (default 7).
- `[pomodoro].minutes` — duration of `:pomo`.

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
cargo run
```

The codebase is intentionally split into small modules so it stays easy to
extend:

| Module        | Responsibility                                          |
| ------------- | ------------------------------------------------------- |
| `main.rs`     | Terminal setup, event loop, tick scheduling             |
| `app.rs`      | Application state, key dispatch, command execution      |
| `ui.rs`       | Ratatui rendering (sidebar, tasks, details, bottom bar) |
| `input.rs`    | Mode and focus types, edit-target metadata              |
| `db.rs`       | SQLite schema, CRUD, sample-data seeding                |
| `models.rs`   | `Task` / `Project` / `Status` / `Priority`, tree helpers |
| `config.rs`   | TOML config loading, default values, path resolution    |
| `commands.rs` | `:` command parser                                      |
| `filters.rs`  | Task filtering and the "today" view                     |
| `export.rs`   | Markdown rendering                                      |
| `pomodoro.rs` | Focus-timer state                                       |
| `error.rs`    | `AppError` / `AppResult`                                |

## License

MIT.
