use crate::commands::{self, Command, ExportScope};
use crate::config::{Config, Paths};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::export::render_markdown;
use crate::filters::{self, TaskFilter};
use crate::input::{EditTarget, Focus, Mode};
use crate::models::{build_tree, flatten_tree, FlatRow, Priority, Project, Status, Task};
use crate::pomodoro::Pomodoro;
use chrono::{Duration, Local, NaiveDate, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    All,
    Today,
    Done,
    Log,
    Project(i64),
    Tag(String),
}

impl View {
    pub fn label(&self, projects: &[Project]) -> String {
        match self {
            View::All => "All".into(),
            View::Today => "Today".into(),
            View::Done => "Done".into(),
            View::Log => "Log".into(),
            View::Project(id) => projects
                .iter()
                .find(|p| p.id == *id)
                .map(|p| format!("Project: {}", p.name))
                .unwrap_or_else(|| "Project: ?".into()),
            View::Tag(t) => format!("Tag: {t}"),
        }
    }
}

/// One row in the sidebar list.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    View(View),
    Project(Project),
    Tag(String),
    SectionHeader(&'static str),
}

pub struct App {
    pub db: Db,
    pub config: Config,
    pub paths: Paths,

    pub tasks: Vec<Task>,
    pub projects: Vec<Project>,

    pub mode: Mode,
    pub focus: Focus,
    pub view: View,
    pub expanded: HashMap<i64, bool>,

    pub task_cursor: usize,
    pub sidebar_cursor: usize,

    pub input_buffer: String,
    pub edit_target: Option<EditTarget>,
    pub search_query: Option<String>,

    pub status_message: Option<String>,
    pub pomodoro: Option<Pomodoro>,

    pub pending_g: bool,
    pub pending_d: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(db: Db, config: Config, paths: Paths) -> AppResult<Self> {
        let tasks = db.list_tasks()?;
        let projects = db.list_projects()?;
        let mut app = Self {
            db,
            config,
            paths,
            tasks,
            projects,
            mode: Mode::Normal,
            focus: Focus::Tasks,
            view: View::All,
            expanded: HashMap::new(),
            task_cursor: 0,
            sidebar_cursor: 0,
            input_buffer: String::new(),
            edit_target: None,
            search_query: None,
            status_message: None,
            pomodoro: None,
            pending_g: false,
            pending_d: false,
            should_quit: false,
        };
        app.normalize_cursor();
        Ok(app)
    }

    // ------------------------------------------------------------------
    // Derived state
    // ------------------------------------------------------------------

    pub fn today_date(&self) -> NaiveDate {
        Local::now().date_naive()
    }

    pub fn sidebar_items(&self) -> Vec<SidebarItem> {
        let mut items = vec![
            SidebarItem::SectionHeader("Views"),
            SidebarItem::View(View::All),
            SidebarItem::View(View::Today),
            SidebarItem::View(View::Done),
            SidebarItem::View(View::Log),
        ];
        if !self.projects.is_empty() {
            items.push(SidebarItem::SectionHeader("Projects"));
            for p in &self.projects {
                items.push(SidebarItem::Project(p.clone()));
            }
        }
        if !self.config.tags.default.is_empty() {
            items.push(SidebarItem::SectionHeader("Tags"));
            for t in &self.config.tags.default {
                items.push(SidebarItem::Tag(t.clone()));
            }
        }
        items
    }

    /// Selectable sidebar indices (skip section headers).
    pub fn sidebar_selectable_indices(&self) -> Vec<usize> {
        self.sidebar_items()
            .iter()
            .enumerate()
            .filter_map(|(i, it)| match it {
                SidebarItem::SectionHeader(_) => None,
                _ => Some(i),
            })
            .collect()
    }

    /// The currently visible flat rows (after view + filter + search).
    pub fn visible_rows(&self) -> Vec<FlatRow> {
        let today = self.today_date();
        let week_days = self.config.ui.week_days;
        let tasks = match &self.view {
            View::All => filters::apply(
                &self.tasks,
                &TaskFilter {
                    query: self.search_query.clone(),
                    include_done: self.config.ui.show_done_in_all,
                    ..Default::default()
                },
                today,
                week_days,
            ),
            View::Today => {
                let mut t = filters::today_view(&self.tasks, today);
                if let Some(q) = &self.search_query {
                    let q = q.to_lowercase();
                    t.retain(|x| {
                        format!("{} {}", x.title, x.description)
                            .to_lowercase()
                            .contains(&q)
                    });
                }
                t
            }
            View::Done => filters::apply(
                &self.tasks,
                &TaskFilter {
                    status: Some(Status::Done),
                    query: self.search_query.clone(),
                    ..Default::default()
                },
                today,
                week_days,
            ),
            View::Log => {
                // Work log: completed tasks across all time, newest first.
                let mut done: Vec<Task> = self
                    .tasks
                    .iter()
                    .filter(|t| t.completed_at.is_some())
                    .cloned()
                    .collect();
                done.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
                if let Some(q) = &self.search_query {
                    let q = q.to_lowercase();
                    done.retain(|x| {
                        format!("{} {}", x.title, x.description)
                            .to_lowercase()
                            .contains(&q)
                    });
                }
                done
            }
            View::Project(pid) => filters::apply(
                &self.tasks,
                &TaskFilter {
                    project_id: Some(*pid),
                    query: self.search_query.clone(),
                    include_done: self.config.ui.show_done_in_all,
                    ..Default::default()
                },
                today,
                week_days,
            ),
            View::Tag(tag) => filters::apply(
                &self.tasks,
                &TaskFilter {
                    tag: Some(tag.clone()),
                    query: self.search_query.clone(),
                    include_done: self.config.ui.show_done_in_all,
                    ..Default::default()
                },
                today,
                week_days,
            ),
        };
        let tree = build_tree(&tasks);
        flatten_tree(&tree, &self.expanded)
    }

    pub fn selected_task(&self) -> Option<Task> {
        let rows = self.visible_rows();
        rows.get(self.task_cursor).map(|r| r.task.clone())
    }

    fn normalize_cursor(&mut self) {
        let n = self.visible_rows().len();
        if n == 0 {
            self.task_cursor = 0;
        } else if self.task_cursor >= n {
            self.task_cursor = n - 1;
        }
    }

    fn reload_tasks(&mut self) -> AppResult<()> {
        self.tasks = self.db.list_tasks()?;
        self.normalize_cursor();
        Ok(())
    }

    fn reload_projects(&mut self) -> AppResult<()> {
        self.projects = self.db.list_projects()?;
        Ok(())
    }

    fn set_status<S: Into<String>>(&mut self, s: S) {
        self.status_message = Some(s.into());
    }

    // ------------------------------------------------------------------
    // Key handling
    // ------------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) -> AppResult<()> {
        // Ctrl-c always quits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return Ok(());
        }

        match self.mode {
            Mode::Normal => self.on_key_normal(key),
            Mode::Insert => self.on_key_insert(key),
            Mode::Command => self.on_key_command_or_search(key, true),
            Mode::Search => self.on_key_command_or_search(key, false),
            Mode::Help => self.on_key_help(key),
        }
    }

    fn on_key_help(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_key_command_or_search(&mut self, key: KeyEvent, is_command: bool) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                let buf = std::mem::take(&mut self.input_buffer);
                self.mode = Mode::Normal;
                if is_command {
                    match commands::parse(&buf) {
                        Ok(cmd) => self.execute_command(cmd)?,
                        Err(e) => self.set_status(format!("err: {e}")),
                    }
                } else {
                    let q = buf.trim();
                    if q.is_empty() {
                        self.search_query = None;
                    } else {
                        self.search_query = Some(q.to_string());
                    }
                    self.normalize_cursor();
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn on_key_insert(&mut self, key: KeyEvent) -> AppResult<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input_buffer.clear();
                self.edit_target = None;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => self.commit_insert()?,
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn commit_insert(&mut self) -> AppResult<()> {
        let buf = std::mem::take(&mut self.input_buffer).trim().to_string();
        let target = self.edit_target.take();
        self.mode = Mode::Normal;
        let Some(target) = target else {
            return Ok(());
        };
        match target {
            EditTarget::NewTask => {
                if buf.is_empty() {
                    self.set_status("cancelled (empty title)");
                    return Ok(());
                }
                let mut t = Task::new_now(buf);
                // Inherit project context from the current view.
                if let View::Project(pid) = &self.view {
                    t.project_id = Some(*pid);
                }
                if let View::Tag(tag) = &self.view {
                    t.tags.push(tag.clone());
                }
                let id = self.db.insert_task(&t)?;
                self.reload_tasks()?;
                self.move_cursor_to_task(id);
                self.set_status("task added");
            }
            EditTarget::NewSubtask { parent_id } => {
                if buf.is_empty() {
                    self.set_status("cancelled (empty title)");
                    return Ok(());
                }
                let parent = self
                    .db
                    .get_task(parent_id)?
                    .ok_or_else(|| AppError::NotFound(format!("parent {parent_id}")))?;
                let mut t = Task::new_now(buf);
                t.parent_id = Some(parent_id);
                t.project_id = parent.project_id;
                let id = self.db.insert_task(&t)?;
                self.expanded.insert(parent_id, true);
                self.reload_tasks()?;
                self.move_cursor_to_task(id);
                self.set_status("subtask added");
            }
            EditTarget::EditTitle { task_id } => {
                if buf.is_empty() {
                    self.set_status("title unchanged (empty input)");
                    return Ok(());
                }
                if let Some(mut t) = self.db.get_task(task_id)? {
                    t.title = buf;
                    t.updated_at = Utc::now();
                    self.db.update_task(&t)?;
                    self.reload_tasks()?;
                    self.set_status("title updated");
                }
            }
            EditTarget::EditDescription { task_id } => {
                if let Some(mut t) = self.db.get_task(task_id)? {
                    t.description = buf;
                    t.updated_at = Utc::now();
                    self.db.update_task(&t)?;
                    self.reload_tasks()?;
                    self.set_status("description updated");
                }
            }
        }
        Ok(())
    }

    fn on_key_normal(&mut self, key: KeyEvent) -> AppResult<()> {
        // Two-key sequences first.
        if self.pending_g {
            self.pending_g = false;
            if let KeyCode::Char('g') = key.code {
                self.cursor_top();
                return Ok(());
            }
            // fall through to single-key handling
        }
        if self.pending_d {
            self.pending_d = false;
            if let KeyCode::Char('d') = key.code {
                self.delete_selected()?;
                return Ok(());
            }
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.input_buffer.clear();
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.input_buffer.clear();
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('h') | KeyCode::Left => self.move_left(),
            KeyCode::Char('l') | KeyCode::Right => self.move_right(),
            KeyCode::Char('g') => {
                self.pending_g = true;
            }
            KeyCode::Char('G') => self.cursor_bottom(),
            KeyCode::Char('a') => self.begin_new_task(),
            KeyCode::Char('o') => self.begin_new_subtask(),
            KeyCode::Char('i') => self.begin_edit_title(),
            KeyCode::Char('D') => self.begin_edit_description(),
            KeyCode::Char('d') => {
                self.pending_d = true;
            }
            KeyCode::Char(' ') => self.toggle_complete()?,
            KeyCode::Char('s') => self.cycle_status()?,
            KeyCode::Char('p') => self.cycle_priority()?,
            KeyCode::Char('c') => {
                self.search_query = None;
                self.set_status("filters cleared");
                self.normalize_cursor();
            }
            KeyCode::Enter => self.on_enter()?,
            _ => {}
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Movement
    // ------------------------------------------------------------------

    fn move_down(&mut self) {
        match self.focus {
            Focus::Tasks => {
                let n = self.visible_rows().len();
                if n > 0 && self.task_cursor + 1 < n {
                    self.task_cursor += 1;
                }
            }
            Focus::Sidebar => self.sidebar_move(1),
            Focus::Details => {}
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Tasks => {
                if self.task_cursor > 0 {
                    self.task_cursor -= 1;
                }
            }
            Focus::Sidebar => self.sidebar_move(-1),
            Focus::Details => {}
        }
    }

    fn move_left(&mut self) {
        match self.focus {
            Focus::Tasks => {
                // Collapse current row, or jump to sidebar if already collapsed.
                if let Some(t) = self.selected_task() {
                    let rows = self.visible_rows();
                    if let Some(row) = rows.get(self.task_cursor) {
                        if row.has_children && row.expanded {
                            self.expanded.insert(t.id, false);
                            return;
                        }
                    }
                }
                self.focus = Focus::Sidebar;
            }
            Focus::Details => self.focus = Focus::Tasks,
            Focus::Sidebar => {}
        }
    }

    fn move_right(&mut self) {
        match self.focus {
            Focus::Sidebar => self.focus = Focus::Tasks,
            Focus::Tasks => {
                if let Some(t) = self.selected_task() {
                    let rows = self.visible_rows();
                    if let Some(row) = rows.get(self.task_cursor) {
                        if row.has_children && !row.expanded {
                            self.expanded.insert(t.id, true);
                            return;
                        }
                    }
                }
                self.focus = Focus::Details;
            }
            Focus::Details => {}
        }
    }

    fn cursor_top(&mut self) {
        match self.focus {
            Focus::Tasks => self.task_cursor = 0,
            Focus::Sidebar => {
                if let Some(&first) = self.sidebar_selectable_indices().first() {
                    self.sidebar_cursor = first;
                }
            }
            Focus::Details => {}
        }
    }

    fn cursor_bottom(&mut self) {
        match self.focus {
            Focus::Tasks => {
                let n = self.visible_rows().len();
                if n > 0 {
                    self.task_cursor = n - 1;
                }
            }
            Focus::Sidebar => {
                if let Some(&last) = self.sidebar_selectable_indices().last() {
                    self.sidebar_cursor = last;
                }
            }
            Focus::Details => {}
        }
    }

    fn sidebar_move(&mut self, delta: i32) {
        let sel = self.sidebar_selectable_indices();
        if sel.is_empty() {
            return;
        }
        let cur_pos = sel
            .iter()
            .position(|&i| i == self.sidebar_cursor)
            .unwrap_or(0);
        let new_pos = ((cur_pos as i32 + delta).rem_euclid(sel.len() as i32)) as usize;
        self.sidebar_cursor = sel[new_pos];
    }

    fn on_enter(&mut self) -> AppResult<()> {
        match self.focus {
            Focus::Sidebar => {
                let items = self.sidebar_items();
                if let Some(item) = items.get(self.sidebar_cursor) {
                    match item {
                        SidebarItem::View(v) => {
                            self.view = v.clone();
                            self.task_cursor = 0;
                            self.focus = Focus::Tasks;
                        }
                        SidebarItem::Project(p) => {
                            self.view = View::Project(p.id);
                            self.task_cursor = 0;
                            self.focus = Focus::Tasks;
                        }
                        SidebarItem::Tag(t) => {
                            self.view = View::Tag(t.clone());
                            self.task_cursor = 0;
                            self.focus = Focus::Tasks;
                        }
                        SidebarItem::SectionHeader(_) => {}
                    }
                }
            }
            Focus::Tasks => {
                // Expand/collapse, or focus details if no children.
                if let Some(t) = self.selected_task() {
                    let rows = self.visible_rows();
                    if let Some(row) = rows.get(self.task_cursor) {
                        if row.has_children {
                            let cur = *self.expanded.get(&t.id).unwrap_or(&true);
                            self.expanded.insert(t.id, !cur);
                        } else {
                            self.focus = Focus::Details;
                        }
                    }
                }
            }
            Focus::Details => {}
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Task actions
    // ------------------------------------------------------------------

    fn begin_new_task(&mut self) {
        self.mode = Mode::Insert;
        self.edit_target = Some(EditTarget::NewTask);
        self.input_buffer.clear();
    }

    fn begin_new_subtask(&mut self) {
        let Some(t) = self.selected_task() else {
            self.set_status("no task selected");
            return;
        };
        self.mode = Mode::Insert;
        self.edit_target = Some(EditTarget::NewSubtask { parent_id: t.id });
        self.input_buffer.clear();
    }

    fn begin_edit_title(&mut self) {
        let Some(t) = self.selected_task() else {
            self.set_status("no task selected");
            return;
        };
        self.mode = Mode::Insert;
        self.edit_target = Some(EditTarget::EditTitle { task_id: t.id });
        self.input_buffer = t.title;
    }

    fn begin_edit_description(&mut self) {
        let Some(t) = self.selected_task() else {
            self.set_status("no task selected");
            return;
        };
        self.mode = Mode::Insert;
        self.edit_target = Some(EditTarget::EditDescription { task_id: t.id });
        self.input_buffer = t.description;
    }

    fn delete_selected(&mut self) -> AppResult<()> {
        let Some(t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        self.db.delete_task(t.id)?;
        self.reload_tasks()?;
        self.set_status("task deleted");
        Ok(())
    }

    fn toggle_complete(&mut self) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        let became_done;
        if t.is_done() {
            t.status = Status::Todo;
            t.completed_at = None;
            became_done = false;
        } else {
            t.status = Status::Done;
            t.completed_at = Some(Utc::now());
            became_done = true;
        }
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.report_status_change(&t, became_done);
        Ok(())
    }

    fn cycle_status(&mut self) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        let was_done = t.is_done();
        t.status = t.status.cycle_next();
        let became_done = t.is_done() && !was_done;
        t.completed_at = if t.is_done() { Some(Utc::now()) } else { None };
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.report_status_change(&t, became_done);
        Ok(())
    }

    /// True for views that hide done tasks by default (so the user knows
    /// where a task went when it disappears).
    fn current_view_hides_done(&self) -> bool {
        match self.view {
            View::All | View::Project(_) | View::Tag(_) => !self.config.ui.show_done_in_all,
            View::Today => true, // today_view already excludes done
            View::Done | View::Log => false,
        }
    }

    fn report_status_change(&mut self, t: &Task, became_done: bool) {
        if became_done && self.current_view_hides_done() {
            self.set_status(format!(
                "'{}' → done (hidden here; see Done view, or set [ui] show_done_in_all = true)",
                t.title
            ));
        } else {
            self.set_status(format!("'{}' → {}", t.title, t.status.as_str()));
        }
    }

    fn cycle_priority(&mut self) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        t.priority = t.priority.cycle_next();
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        Ok(())
    }

    fn move_cursor_to_task(&mut self, id: i64) {
        let rows = self.visible_rows();
        if let Some(pos) = rows.iter().position(|r| r.task.id == id) {
            self.task_cursor = pos;
        }
    }

    // ------------------------------------------------------------------
    // Commands
    // ------------------------------------------------------------------

    fn execute_command(&mut self, cmd: Command) -> AppResult<()> {
        match cmd {
            Command::Quit => self.should_quit = true,
            Command::Write => self.set_status("ok (changes already saved)"),
            Command::Help => self.mode = Mode::Help,
            Command::ClearFilters => {
                self.search_query = None;
                self.view = View::All;
                self.set_status("filters cleared");
                self.normalize_cursor();
            }
            Command::Export { scope, file } => self.cmd_export(scope, file)?,
            Command::AddTag(tag) => self.cmd_add_tag(tag)?,
            Command::RemoveTag(tag) => self.cmd_remove_tag(tag)?,
            Command::Due(s) => self.cmd_due(&s)?,
            Command::Priority(s) => self.cmd_priority(&s)?,
            Command::Status(s) => self.cmd_status(&s)?,
            Command::Description(s) => self.cmd_description(&s)?,
            Command::SetTaskProject(name) => self.cmd_set_project(&name)?,
            Command::NewProject(name) => self.cmd_new_project(&name)?,
            Command::Pomo => self.cmd_pomo()?,
            Command::PomoStop => {
                self.pomodoro = None;
                self.set_status("pomodoro stopped");
            }
        }
        Ok(())
    }

    fn cmd_export(&mut self, scope: ExportScope, file: Option<String>) -> AppResult<()> {
        let today = self.today_date();
        let (title, tasks) = match scope {
            ExportScope::Today => ("Today".to_string(), filters::today_view(&self.tasks, today)),
            ExportScope::All => ("All".to_string(), self.tasks.clone()),
            ExportScope::Visible => {
                let rows = self.visible_rows();
                let label = self.view.label(&self.projects);
                let tasks: Vec<Task> = rows.into_iter().map(|r| r.task).collect();
                (label, tasks)
            }
            ExportScope::Project(name) => {
                let pid = self
                    .projects
                    .iter()
                    .find(|p| p.name == name)
                    .map(|p| p.id)
                    .ok_or_else(|| AppError::NotFound(format!("project {name}")))?;
                let tasks: Vec<Task> = self
                    .tasks
                    .iter()
                    .filter(|t| t.project_id == Some(pid) && !t.is_done())
                    .cloned()
                    .collect();
                (format!("Project: {name}"), tasks)
            }
        };
        let md = render_markdown(&title, &tasks);
        match file {
            Some(path) => {
                fs::write(&path, &md)?;
                self.set_status(format!("exported to {path}"));
            }
            None => {
                // Buffer it onto the status line; full md also printed on quit
                // would clobber the screen, so the recommended path is to use a
                // filename for now.
                let path = self.paths.data_dir.join("last_export.md");
                fs::write(&path, &md)?;
                self.set_status(format!("exported to {}", path.display()));
            }
        }
        Ok(())
    }

    fn cmd_add_tag(&mut self, tag: String) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        if !self.config.tags.default.iter().any(|x| x == &tag) {
            return Err(AppError::InvalidInput(format!(
                "tag '{tag}' not in predefined tags (config.toml)"
            )));
        }
        if !t.tags.contains(&tag) {
            t.tags.push(tag);
            t.updated_at = Utc::now();
            self.db.update_task(&t)?;
            self.reload_tasks()?;
            self.set_status("tag added");
        } else {
            self.set_status("tag already present");
        }
        Ok(())
    }

    fn cmd_remove_tag(&mut self, tag: String) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        let before = t.tags.len();
        t.tags.retain(|x| x != &tag);
        if t.tags.len() != before {
            t.updated_at = Utc::now();
            self.db.update_task(&t)?;
            self.reload_tasks()?;
            self.set_status("tag removed");
        } else {
            self.set_status("tag not present");
        }
        Ok(())
    }

    fn cmd_due(&mut self, s: &str) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        let new_due = parse_due(s, self.today_date())?;
        t.due_date = new_due;
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.set_status("due updated");
        Ok(())
    }

    fn cmd_priority(&mut self, s: &str) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        let p: Priority = s.parse().map_err(|e: String| AppError::InvalidInput(e))?;
        t.priority = p;
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.set_status("priority updated");
        Ok(())
    }

    fn cmd_status(&mut self, s: &str) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        let st: Status = s.parse().map_err(|e: String| AppError::InvalidInput(e))?;
        t.status = st;
        t.completed_at = if t.is_done() { Some(Utc::now()) } else { None };
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.set_status("status updated");
        Ok(())
    }

    fn cmd_description(&mut self, s: &str) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        t.description = s.to_string();
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.set_status("description updated");
        Ok(())
    }

    fn cmd_set_project(&mut self, name: &str) -> AppResult<()> {
        let Some(mut t) = self.selected_task() else {
            return Ok(());
        };
        if name == "none" {
            t.project_id = None;
        } else {
            let pid = self
                .projects
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.id)
                .ok_or_else(|| AppError::NotFound(format!("project {name}")))?;
            t.project_id = Some(pid);
        }
        t.updated_at = Utc::now();
        self.db.update_task(&t)?;
        self.reload_tasks()?;
        self.set_status("project updated");
        Ok(())
    }

    fn cmd_new_project(&mut self, name: &str) -> AppResult<()> {
        self.db.ensure_projects(&[name.to_string()])?;
        self.reload_projects()?;
        self.set_status(format!("project '{name}' created"));
        Ok(())
    }

    fn cmd_pomo(&mut self) -> AppResult<()> {
        let Some(t) = self.selected_task() else {
            self.set_status("no task selected");
            return Ok(());
        };
        self.pomodoro = Some(Pomodoro::new(t.id, self.config.pomodoro.minutes));
        self.set_status(format!(
            "pomodoro started ({} min) on '{}'",
            self.config.pomodoro.minutes, t.title
        ));
        Ok(())
    }
}

/// Parse the user-typed due value into an optional date.
fn parse_due(s: &str, today: NaiveDate) -> AppResult<Option<NaiveDate>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") || s.is_empty() {
        return Ok(None);
    }
    if s.eq_ignore_ascii_case("today") {
        return Ok(Some(today));
    }
    if s.eq_ignore_ascii_case("tomorrow") {
        return Ok(Some(today + Duration::days(1)));
    }
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")?;
    Ok(Some(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_due_handles_keywords() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        assert_eq!(parse_due("today", today).unwrap(), Some(today));
        assert_eq!(
            parse_due("tomorrow", today).unwrap(),
            Some(today + Duration::days(1))
        );
        assert_eq!(parse_due("none", today).unwrap(), None);
        assert_eq!(
            parse_due("2026-06-01", today).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
        );
        assert!(parse_due("not-a-date", today).is_err());
    }
}
