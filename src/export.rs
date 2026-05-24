use crate::models::{build_tree, Task, TaskNode};

/// Render a list of tasks as a Markdown checklist. Subtasks are indented under
/// their parents, using build_tree so parent/child relationships are honored.
pub fn render_markdown(title: &str, tasks: &[Task]) -> String {
    let tree = build_tree(tasks);
    let mut out = String::new();
    out.push_str(&format!("## {title}\n\n"));
    if tree.is_empty() {
        out.push_str("_(no tasks)_\n");
        return out;
    }
    for node in &tree {
        write_node(&mut out, node, 0);
    }
    out
}

fn write_node(out: &mut String, node: &TaskNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let mark = if node.task.is_done() { "x" } else { " " };
    out.push_str(&format!("{indent}- [{mark}] {}", node.task.title));
    if let Some(d) = node.task.due_date {
        out.push_str(&format!(" (due: {d})"));
    }
    out.push('\n');
    for child in &node.children {
        write_node(out, child, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Status, Task};

    fn t(id: i64, parent: Option<i64>, title: &str, done: bool) -> Task {
        let mut x = Task::new_now(title);
        x.id = id;
        x.parent_id = parent;
        if done {
            x.status = Status::Done;
        }
        x
    }

    #[test]
    fn export_indents_subtasks() {
        let tasks = vec![
            t(1, None, "論文誌執筆", false),
            t(2, Some(1), "関連研究を書く", false),
            t(3, Some(1), "実験結果を整理する", false),
            t(4, None, "PBL資料作り", false),
        ];
        let md = render_markdown("Today", &tasks);
        assert!(md.contains("## Today"));
        assert!(md.contains("- [ ] 論文誌執筆"));
        assert!(md.contains("  - [ ] 関連研究を書く"));
        assert!(md.contains("  - [ ] 実験結果を整理する"));
        assert!(md.contains("- [ ] PBL資料作り"));
    }

    #[test]
    fn export_marks_done() {
        let tasks = vec![t(1, None, "done thing", true)];
        let md = render_markdown("All", &tasks);
        assert!(md.contains("- [x] done thing"));
    }

    #[test]
    fn export_empty_shows_placeholder() {
        let md = render_markdown("Today", &[]);
        assert!(md.contains("_(no tasks)_"));
    }
}
