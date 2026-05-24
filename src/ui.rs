use crate::app::{App, SidebarItem, View};
use crate::filters;
use crate::input::{EditTarget, Focus, Mode};
use crate::models::{fmt_local_dt, Priority, Status};
use chrono::Local;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Min(20),
            Constraint::Length(38),
        ])
        .split(outer[0]);

    draw_sidebar(f, app, main[0]);
    draw_tasks(f, app, main[1]);
    draw_details(f, app, main[2]);
    draw_bottom(f, app, outer[1]);

    if app.mode == Mode::Help {
        draw_help_overlay(f, area);
    }
}

// ---------- sidebar ----------

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let items = app.sidebar_items();
    let active_label = active_sidebar_label(app);

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (text, style) = match item {
                SidebarItem::SectionHeader(s) => (
                    s.to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
                SidebarItem::View(v) => {
                    let label = match v {
                        View::All => "  All".to_string(),
                        View::Today => "  Today".to_string(),
                        View::Done => "  Done".to_string(),
                        View::Log => "  Log".to_string(),
                        View::Project(_) | View::Tag(_) => "  ?".to_string(),
                    };
                    style_for_active(label, active_label.as_deref(), i, app)
                }
                SidebarItem::Project(p) => {
                    let label = format!("  # {}", p.name);
                    style_for_active(label, active_label.as_deref(), i, app)
                }
                SidebarItem::Tag(t) => {
                    let label = format!("  @ {t}");
                    style_for_active(label, active_label.as_deref(), i, app)
                }
            };
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let mut state = ListState::default();
    if matches!(app.focus, Focus::Sidebar) {
        state.select(Some(app.sidebar_cursor));
    }

    let border_style = focus_border(app.focus, Focus::Sidebar);
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Views ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, &mut state);
}

fn style_for_active(
    label: String,
    active: Option<&str>,
    _idx: usize,
    _app: &App,
) -> (String, Style) {
    let is_active = active.map(|a| a == label.trim()).unwrap_or(false);
    let mut s = Style::default();
    if is_active {
        s = s.fg(Color::Cyan).add_modifier(Modifier::BOLD);
    }
    (label, s)
}

fn active_sidebar_label(app: &App) -> Option<String> {
    match &app.view {
        View::All => Some("All".into()),
        View::Today => Some("Today".into()),
        View::Done => Some("Done".into()),
        View::Log => Some("Log".into()),
        View::Project(pid) => app
            .projects
            .iter()
            .find(|p| p.id == *pid)
            .map(|p| format!("# {}", p.name)),
        View::Tag(t) => Some(format!("@ {t}")),
    }
}

// ---------- tasks pane ----------

fn draw_tasks(f: &mut Frame, app: &mut App, area: Rect) {
    let today = app.today_date();
    let rows = app.visible_rows();

    let title = format!(" {} ({}) ", app.view.label(&app.projects), rows.len());

    let items: Vec<ListItem> = if matches!(app.view, View::Log) {
        log_rows(app)
    } else {
        rows.iter()
            .map(|row| {
                let mut spans: Vec<Span> = Vec::new();

                // Indent for subtasks.
                if row.depth > 0 {
                    spans.push(Span::raw("  ".repeat(row.depth)));
                }

                // Expand/collapse marker.
                if row.has_children {
                    spans.push(Span::styled(
                        if row.expanded { "▾ " } else { "▸ " },
                        Style::default().fg(Color::Yellow),
                    ));
                } else {
                    spans.push(Span::raw("  "));
                }

                // Status checkbox.
                let status_style = match row.task.status {
                    Status::Done => Style::default().fg(Color::Green),
                    Status::Doing => Style::default().fg(Color::Yellow),
                    Status::Waiting => Style::default().fg(Color::Magenta),
                    Status::Todo => Style::default(),
                };
                spans.push(Span::styled(row.task.status.symbol(), status_style));
                spans.push(Span::raw(" "));

                // Priority.
                let pri_style = match row.task.priority {
                    Priority::High => Style::default().fg(Color::Red),
                    Priority::Medium => Style::default().fg(Color::Yellow),
                    Priority::Low => Style::default().fg(Color::Blue),
                };
                spans.push(Span::styled(
                    format!("[{}] ", row.task.priority.symbol()),
                    pri_style,
                ));

                // Title (struck out if done, red if overdue).
                let mut title_style = Style::default();
                if row.task.is_done() {
                    title_style = title_style
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::CROSSED_OUT);
                } else if row.task.is_overdue(today) {
                    title_style = title_style.fg(Color::Red);
                }
                spans.push(Span::styled(row.task.title.clone(), title_style));

                // Due date.
                if let Some(d) = row.task.due_date {
                    let style = if !row.task.is_done() && d < today {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(format!("({d})"), style));
                }

                // Tags.
                if !row.task.tags.is_empty() {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("[{}]", row.task.tags.join(",")),
                        Style::default().fg(Color::Cyan),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.task_cursor.min(items.len() - 1)));
    }

    let border_style = focus_border(app.focus, Focus::Tasks);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("» ");
    f.render_stateful_widget(list, area, &mut state);
}

/// Render the work log as date-grouped completed tasks.
fn log_rows(app: &App) -> Vec<ListItem<'static>> {
    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<chrono::NaiveDate, Vec<&crate::models::Task>> = BTreeMap::new();
    for t in &app.tasks {
        if let Some(c) = t.completed_at {
            let d = c.with_timezone(&Local).date_naive();
            by_day.entry(d).or_default().push(t);
        }
    }
    let mut out = Vec::new();
    // newest first
    for (day, tasks) in by_day.into_iter().rev() {
        out.push(ListItem::new(Line::from(Span::styled(
            format!("─── {day} ───"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))));
        for t in tasks {
            out.push(ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("[x] ", Style::default().fg(Color::Green)),
                Span::raw(t.title.clone()),
            ])));
        }
    }
    if out.is_empty() {
        out.push(ListItem::new(Span::styled(
            "(no completed tasks yet)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    out
}

// ---------- details pane ----------

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let border_style = focus_border(app.focus, Focus::Details);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ")
        .border_style(border_style);

    let Some(task) = app.selected_task() else {
        let p = Paragraph::new(Span::styled(
            "no task selected",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    };

    let project_name = task
        .project_id
        .and_then(|pid| app.projects.iter().find(|p| p.id == pid))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".into());

    let due_str = task
        .due_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| "—".into());

    let tags_str = if task.tags.is_empty() {
        "—".into()
    } else {
        task.tags.join(", ")
    };

    let completed = task
        .completed_at
        .map(fmt_local_dt)
        .unwrap_or_else(|| "—".into());

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("title:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                task.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("status:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(task.status.as_str()),
        ]),
        Line::from(vec![
            Span::styled("priority: ", Style::default().fg(Color::DarkGray)),
            Span::raw(task.priority.as_str()),
        ]),
        Line::from(vec![
            Span::styled("project:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(project_name),
        ]),
        Line::from(vec![
            Span::styled("due:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(due_str),
        ]),
        Line::from(vec![
            Span::styled("tags:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(tags_str),
        ]),
        Line::from(vec![
            Span::styled("created:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(fmt_local_dt(task.created_at)),
        ]),
        Line::from(vec![
            Span::styled("updated:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(fmt_local_dt(task.updated_at)),
        ]),
        Line::from(vec![
            Span::styled("completed:", Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::raw(completed),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "description",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if task.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "(empty — press D to edit)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for l in task.description.lines() {
            lines.push(Line::from(l.to_string()));
        }
    }

    // Today summary if Today view.
    if matches!(app.view, View::Today) {
        let today = app.today_date();
        let today_tasks = filters::today_view(&app.tasks, today);
        let overdue = today_tasks.iter().filter(|t| t.is_overdue(today)).count();
        let due_today = today_tasks.iter().filter(|t| t.due_today(today)).count();
        let doing = today_tasks
            .iter()
            .filter(|t| t.status == Status::Doing)
            .count();
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "today summary",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(format!("  overdue: {overdue}")));
        lines.push(Line::from(format!("  due today: {due_today}")));
        lines.push(Line::from(format!("  doing: {doing}")));
    }

    let p = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

// ---------- bottom bar ----------

fn draw_bottom(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Top line: mode-driven input or status / hints.
    let top: Line = match app.mode {
        Mode::Command => Line::from(vec![
            Span::styled(
                ":",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.input_buffer.clone()),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.input_buffer.clone()),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ]),
        Mode::Insert => {
            let prefix = match app.edit_target {
                Some(EditTarget::NewTask) => "new task: ",
                Some(EditTarget::NewSubtask { .. }) => "new subtask: ",
                Some(EditTarget::EditTitle { .. }) => "title: ",
                Some(EditTarget::EditDescription { .. }) => "description: ",
                None => "input: ",
            };
            Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(app.input_buffer.clone()),
                Span::styled("_", Style::default().fg(Color::Green)),
            ])
        }
        Mode::Normal | Mode::Help => {
            let msg = app
                .status_message
                .clone()
                .unwrap_or_else(|| key_hints().into());
            Line::from(Span::raw(msg))
        }
    };

    // Bottom line: mode, search, pomodoro indicator.
    let mut bits: Vec<Span> = vec![Span::styled(
        format!(" {} ", app.mode.label()),
        Style::default()
            .bg(mode_color(app.mode))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )];

    bits.push(Span::raw(" "));
    bits.push(Span::styled(
        format!("view: {}", app.view.label(&app.projects)),
        Style::default().fg(Color::Cyan),
    ));

    if let Some(q) = &app.search_query {
        bits.push(Span::raw("  "));
        bits.push(Span::styled(
            format!("search: {q}"),
            Style::default().fg(Color::Magenta),
        ));
    }

    if let Some(pomo) = &app.pomodoro {
        let label = if pomo.is_done() {
            "DONE".into()
        } else {
            pomo.remaining_str()
        };
        bits.push(Span::raw("  "));
        bits.push(Span::styled(
            format!("🍅 {label}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let bottom = Line::from(bits);

    f.render_widget(Paragraph::new(top), chunks[0]);
    f.render_widget(Paragraph::new(bottom).alignment(Alignment::Left), chunks[1]);
}

fn mode_color(m: Mode) -> Color {
    match m {
        Mode::Normal => Color::LightBlue,
        Mode::Insert => Color::LightGreen,
        Mode::Command => Color::LightYellow,
        Mode::Search => Color::LightMagenta,
        Mode::Help => Color::LightCyan,
    }
}

fn focus_border(focus: Focus, target: Focus) -> Style {
    if focus == target {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn key_hints() -> &'static str {
    "j/k move  l details  a add  o subtask  i edit  space done  s status  p pri  dd del  / search  : cmd  ? help  q quit"
}

// ---------- help overlay ----------

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 80, area);
    f.render_widget(Clear, popup);
    let lines = vec![
        Line::from(Span::styled(
            "todotui — help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  j / k         move down / up"),
        Line::from("  h / l         left pane / right pane (or collapse / expand)"),
        Line::from("  gg / G        top / bottom of list"),
        Line::from("  tab           cycle focus (sidebar → tasks → details)"),
        Line::from("  enter         expand/collapse, or select sidebar item"),
        Line::from(""),
        Line::from(Span::styled(
            "task actions (normal mode)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  a             add new task"),
        Line::from("  o             add subtask under selected"),
        Line::from("  i             edit selected task title"),
        Line::from("  D             edit selected task description"),
        Line::from("  space         toggle complete"),
        Line::from("  s             cycle status (todo→doing→waiting→done)"),
        Line::from("  p             cycle priority"),
        Line::from("  dd            delete selected task (cascades to subtasks)"),
        Line::from(""),
        Line::from(Span::styled(
            "search / command",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  /             search by title/description"),
        Line::from("  c             clear search"),
        Line::from("  :             command bar"),
        Line::from("  esc           return to normal mode"),
        Line::from(""),
        Line::from(Span::styled(
            "commands",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  :q                              quit"),
        Line::from("  :due today|tomorrow|YYYY-MM-DD  set due date"),
        Line::from("  :pri high|medium|low            set priority"),
        Line::from("  :status todo|doing|waiting|done set status"),
        Line::from("  :desc <text>                    set description"),
        Line::from("  :addtag <name>  :rmtag <name>   manage tags (predefined only)"),
        Line::from("  :project <name|none>            set project of selected task"),
        Line::from("  :newproject <name>              create a new project"),
        Line::from("  :export today|all|visible       export markdown"),
        Line::from("  :export project <name> [file]   export project (optional file)"),
        Line::from("  :pomo / :pomostop               start / stop focus timer"),
        Line::from(""),
        Line::from(Span::styled(
            "press esc / q / ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let p = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help "),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
