use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Todo,
    Doing,
    Waiting,
    Done,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Doing => "doing",
            Status::Waiting => "waiting",
            Status::Done => "done",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Status::Todo => "[ ]",
            Status::Doing => "[~]",
            Status::Waiting => "[?]",
            Status::Done => "[x]",
        }
    }

    pub fn cycle_next(&self) -> Status {
        match self {
            Status::Todo => Status::Doing,
            Status::Doing => Status::Waiting,
            Status::Waiting => Status::Done,
            Status::Done => Status::Todo,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "todo" => Ok(Status::Todo),
            "doing" => Ok(Status::Doing),
            "waiting" => Ok(Status::Waiting),
            "done" => Ok(Status::Done),
            other => Err(format!("unknown status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Priority::High => "H",
            Priority::Medium => "M",
            Priority::Low => "L",
        }
    }

    pub fn cycle_next(&self) -> Priority {
        match self {
            Priority::High => Priority::Medium,
            Priority::Medium => Priority::Low,
            Priority::Low => Priority::High,
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Priority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "high" | "h" => Ok(Priority::High),
            "medium" | "med" | "m" => Ok(Priority::Medium),
            "low" | "l" => Ok(Priority::Low),
            other => Err(format!("unknown priority: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub project_id: Option<i64>,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub due_date: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new_now(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            parent_id: None,
            project_id: None,
            title: title.into(),
            description: String::new(),
            status: Status::Todo,
            priority: Priority::Medium,
            due_date: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    pub fn is_done(&self) -> bool {
        self.status == Status::Done
    }

    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        match self.due_date {
            Some(d) => d < today && !self.is_done(),
            None => false,
        }
    }

    pub fn due_today(&self, today: NaiveDate) -> bool {
        matches!(self.due_date, Some(d) if d == today)
    }

    pub fn due_within_days(&self, today: NaiveDate, days: i64) -> bool {
        match self.due_date {
            Some(d) => {
                let diff = (d - today).num_days();
                diff >= 0 && diff <= days
            }
            None => false,
        }
    }

    #[allow(dead_code)] // formatting helper kept for future UI use
    pub fn due_date_str(&self) -> String {
        self.due_date.map(|d| d.to_string()).unwrap_or_default()
    }
}

/// A node in the task tree: a task plus its children.
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub task: Task,
    pub children: Vec<TaskNode>,
}

/// Flat representation of a tree row used for rendering.
#[derive(Debug, Clone)]
pub struct FlatRow {
    pub task: Task,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// Build a tree from a flat task list. Tasks whose parent is not in the
/// provided list (filtered out, for example) are treated as roots so they
/// remain visible.
pub fn build_tree(tasks: &[Task]) -> Vec<TaskNode> {
    use std::collections::HashMap;
    let ids: std::collections::HashSet<i64> = tasks.iter().map(|t| t.id).collect();

    let mut children_map: HashMap<i64, Vec<Task>> = HashMap::new();
    let mut roots: Vec<Task> = Vec::new();

    for t in tasks {
        match t.parent_id {
            Some(p) if ids.contains(&p) => {
                children_map.entry(p).or_default().push(t.clone());
            }
            _ => roots.push(t.clone()),
        }
    }

    fn build(task: Task, map: &mut std::collections::HashMap<i64, Vec<Task>>) -> TaskNode {
        let children = map.remove(&task.id).unwrap_or_default();
        let mut child_nodes: Vec<TaskNode> = children.into_iter().map(|c| build(c, map)).collect();
        child_nodes.sort_by(|a, b| a.task.id.cmp(&b.task.id));
        TaskNode {
            task,
            children: child_nodes,
        }
    }

    let mut nodes: Vec<TaskNode> = roots
        .into_iter()
        .map(|r| build(r, &mut children_map))
        .collect();
    nodes.sort_by(|a, b| a.task.id.cmp(&b.task.id));
    nodes
}

/// Flatten a tree honoring per-task expanded state. If a task id is not in
/// `expanded`, it defaults to expanded.
pub fn flatten_tree(
    nodes: &[TaskNode],
    expanded: &std::collections::HashMap<i64, bool>,
) -> Vec<FlatRow> {
    let mut out = Vec::new();
    fn walk(
        nodes: &[TaskNode],
        depth: usize,
        expanded: &std::collections::HashMap<i64, bool>,
        out: &mut Vec<FlatRow>,
    ) {
        for n in nodes {
            let is_expanded = *expanded.get(&n.task.id).unwrap_or(&true);
            out.push(FlatRow {
                task: n.task.clone(),
                depth,
                has_children: !n.children.is_empty(),
                expanded: is_expanded,
            });
            if is_expanded {
                walk(&n.children, depth + 1, expanded, out);
            }
        }
    }
    walk(nodes, 0, expanded, &mut out);
    out
}

/// Convenience to format a UTC datetime as local "YYYY-MM-DD HH:MM".
pub fn fmt_local_dt(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: i64, parent: Option<i64>, title: &str) -> Task {
        let mut x = Task::new_now(title);
        x.id = id;
        x.parent_id = parent;
        x
    }

    #[test]
    fn build_tree_two_levels() {
        let tasks = vec![
            t(1, None, "root1"),
            t(2, Some(1), "child"),
            t(3, None, "root2"),
            t(4, Some(2), "grandchild"),
        ];
        let nodes = build_tree(&tasks);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].task.id, 1);
        assert_eq!(nodes[0].children.len(), 1);
        assert_eq!(nodes[0].children[0].children.len(), 1);
    }

    #[test]
    fn build_tree_promotes_orphans() {
        // child whose parent isn't in the list should become a root.
        let tasks = vec![t(1, Some(99), "orphan"), t(2, None, "root")];
        let nodes = build_tree(&tasks);
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn flatten_respects_collapse() {
        let tasks = vec![t(1, None, "p"), t(2, Some(1), "c")];
        let nodes = build_tree(&tasks);
        let mut expanded = std::collections::HashMap::new();
        let flat = flatten_tree(&nodes, &expanded);
        assert_eq!(flat.len(), 2);
        expanded.insert(1, false);
        let flat = flatten_tree(&nodes, &expanded);
        assert_eq!(flat.len(), 1);
    }

    #[test]
    fn status_priority_roundtrip() {
        for s in ["todo", "doing", "waiting", "done"] {
            assert_eq!(s.parse::<Status>().unwrap().as_str(), s);
        }
        for s in ["high", "medium", "low"] {
            assert_eq!(s.parse::<Priority>().unwrap().as_str(), s);
        }
    }
}
