use crate::models::{Priority, Status, Task};
use chrono::NaiveDate;

#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub project_id: Option<i64>,
    pub tag: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub due_category: Option<DueCategory>,
    pub query: Option<String>,
    pub include_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // used via TaskFilter::due_category from future UI/commands
pub enum DueCategory {
    Overdue,
    Today,
    ThisWeek,
}

impl TaskFilter {
    pub fn matches(&self, t: &Task, today: NaiveDate, week_days: i64) -> bool {
        if !self.include_done && t.is_done() {
            // Allow done tasks through only if user explicitly filters status=done.
            if self.status != Some(Status::Done) {
                return false;
            }
        }
        if let Some(pid) = self.project_id {
            if t.project_id != Some(pid) {
                return false;
            }
        }
        if let Some(tag) = &self.tag {
            if !t.tags.iter().any(|x| x == tag) {
                return false;
            }
        }
        if let Some(st) = self.status {
            if t.status != st {
                return false;
            }
        }
        if let Some(pr) = self.priority {
            if t.priority != pr {
                return false;
            }
        }
        match self.due_category {
            Some(DueCategory::Overdue) => {
                if !t.is_overdue(today) {
                    return false;
                }
            }
            Some(DueCategory::Today) => {
                if !t.due_today(today) {
                    return false;
                }
            }
            Some(DueCategory::ThisWeek) => {
                if !t.due_within_days(today, week_days) {
                    return false;
                }
            }
            None => {}
        }
        if let Some(q) = &self.query {
            let q = q.to_lowercase();
            let hay = format!("{} {}", t.title, t.description).to_lowercase();
            if !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Apply a filter to a flat task list.
pub fn apply(tasks: &[Task], filter: &TaskFilter, today: NaiveDate, week_days: i64) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| filter.matches(t, today, week_days))
        .cloned()
        .collect()
}

/// Convenience: "today view" returns overdue + due-today + doing tasks.
pub fn today_view(tasks: &[Task], today: NaiveDate) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| {
            !t.is_done() && (t.is_overdue(today) || t.due_today(today) || t.status == Status::Doing)
        })
        .cloned()
        .collect()
}

/// Tasks completed on a given local date.
#[allow(dead_code)] // exposed for future log-by-day views and tests
pub fn completed_on(tasks: &[Task], date: NaiveDate) -> Vec<Task> {
    use chrono::Local;
    tasks
        .iter()
        .filter(|t| {
            t.completed_at
                .map(|c| c.with_timezone(&Local).date_naive() == date)
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;
    use chrono::Duration;

    fn mk(title: &str, status: Status, due: Option<NaiveDate>) -> Task {
        let mut t = Task::new_now(title);
        t.status = status;
        t.due_date = due;
        t
    }

    #[test]
    fn filter_query_matches_title() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let tasks = vec![
            mk("write paper", Status::Todo, None),
            mk("buy milk", Status::Todo, None),
        ];
        let f = TaskFilter {
            query: Some("paper".into()),
            ..Default::default()
        };
        let out = apply(&tasks, &f, today, 7);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "write paper");
    }

    #[test]
    fn filter_excludes_done_by_default() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let tasks = vec![mk("a", Status::Todo, None), mk("b", Status::Done, None)];
        let out = apply(&tasks, &TaskFilter::default(), today, 7);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "a");
    }

    #[test]
    fn filter_includes_done_when_status_filter_set() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let tasks = vec![mk("a", Status::Todo, None), mk("b", Status::Done, None)];
        let f = TaskFilter {
            status: Some(Status::Done),
            ..Default::default()
        };
        let out = apply(&tasks, &f, today, 7);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "b");
    }

    #[test]
    fn today_view_picks_overdue_and_today() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let yesterday = today - Duration::days(1);
        let next_week = today + Duration::days(7);
        let tasks = vec![
            mk("over", Status::Todo, Some(yesterday)),
            mk("now", Status::Todo, Some(today)),
            mk("future", Status::Todo, Some(next_week)),
            mk("doing", Status::Doing, None),
        ];
        let out = today_view(&tasks, today);
        assert_eq!(out.len(), 3);
        assert!(out.iter().any(|t| t.title == "over"));
        assert!(out.iter().any(|t| t.title == "now"));
        assert!(out.iter().any(|t| t.title == "doing"));
    }
}
