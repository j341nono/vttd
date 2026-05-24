use crate::error::AppResult;
use crate::models::{Priority, Project, Status, Task};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::str::FromStr;

pub struct Db {
    conn: Connection,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id INTEGER,
    project_id INTEGER,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'todo',
    priority TEXT NOT NULL DEFAULT 'medium',
    due_date TEXT,
    tags TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (parent_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
"#;

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Open an in-memory db (used for tests).
    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    // ---------- projects ----------

    pub fn ensure_projects(&self, names: &[String]) -> AppResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        for name in names {
            tx.execute(
                "INSERT OR IGNORE INTO projects (name) VALUES (?1)",
                params![name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_projects(&self) -> AppResult<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM projects ORDER BY id ASC")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn project_id_by_name(&self, name: &str) -> AppResult<Option<i64>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    }

    // ---------- tasks ----------

    pub fn count_tasks(&self) -> AppResult<i64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        Ok(n)
    }

    pub fn insert_task(&self, t: &Task) -> AppResult<i64> {
        self.conn.execute(
            "INSERT INTO tasks
             (parent_id, project_id, title, description, status, priority,
              due_date, tags, created_at, updated_at, completed_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                t.parent_id,
                t.project_id,
                t.title,
                t.description,
                t.status.as_str(),
                t.priority.as_str(),
                t.due_date.map(|d| d.to_string()),
                join_tags(&t.tags),
                t.created_at.to_rfc3339(),
                t.updated_at.to_rfc3339(),
                t.completed_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_task(&self, t: &Task) -> AppResult<()> {
        self.conn.execute(
            "UPDATE tasks SET
                parent_id=?1, project_id=?2, title=?3, description=?4,
                status=?5, priority=?6, due_date=?7, tags=?8,
                updated_at=?9, completed_at=?10
             WHERE id=?11",
            params![
                t.parent_id,
                t.project_id,
                t.title,
                t.description,
                t.status.as_str(),
                t.priority.as_str(),
                t.due_date.map(|d| d.to_string()),
                join_tags(&t.tags),
                t.updated_at.to_rfc3339(),
                t.completed_at.map(|d| d.to_rfc3339()),
                t.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_task(&self, id: i64) -> AppResult<()> {
        // ON DELETE CASCADE handles subtasks.
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_tasks(&self) -> AppResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, project_id, title, description, status,
                    priority, due_date, tags, created_at, updated_at, completed_at
             FROM tasks
             ORDER BY id ASC",
        )?;
        let rows = stmt
            .query_map([], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_task(&self, id: i64) -> AppResult<Option<Task>> {
        let row = self
            .conn
            .query_row(
                "SELECT id, parent_id, project_id, title, description, status,
                        priority, due_date, tags, created_at, updated_at, completed_at
                 FROM tasks WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()?;
        Ok(row)
    }

    /// Seed sample data if the tasks table is empty. Returns true if data was inserted.
    pub fn seed_if_empty(&self) -> AppResult<bool> {
        if self.count_tasks()? > 0 {
            return Ok(false);
        }
        let paper_project_id = self.project_id_by_name("論文誌")?;
        let pbl_project_id = self.project_id_by_name("PBL")?;
        let research_project_id = self.project_id_by_name("研究")?;

        let mut paper = Task::new_now("論文誌執筆");
        paper.project_id = paper_project_id;
        paper.priority = Priority::High;
        paper.tags = vec!["研究".into()];
        let paper_id = self.insert_task(&paper)?;

        let mut pbl = Task::new_now("PBL資料作り");
        pbl.project_id = pbl_project_id;
        pbl.priority = Priority::Medium;
        pbl.tags = vec!["授業".into()];
        self.insert_task(&pbl)?;

        let mut coauth = Task::new_now("共著者用資料作り");
        coauth.project_id = paper_project_id;
        coauth.tags = vec!["研究".into()];
        self.insert_task(&coauth)?;

        let mut paper_read = Task::new_now("論文読み会の論文を読む");
        paper_read.project_id = research_project_id;
        paper_read.tags = vec!["研究".into(), "読書".into()];
        self.insert_task(&paper_read)?;

        let sub_titles = ["関連研究を書く", "実験設定を書く", "結果表を整える"];
        for title in sub_titles {
            let mut s = Task::new_now(title);
            s.parent_id = Some(paper_id);
            s.project_id = paper_project_id;
            s.tags = vec!["研究".into()];
            self.insert_task(&s)?;
        }

        Ok(true)
    }
}

fn row_to_task(r: &rusqlite::Row) -> rusqlite::Result<Task> {
    let status_str: String = r.get(5)?;
    let priority_str: String = r.get(6)?;
    let due_str: Option<String> = r.get(7)?;
    let tags_str: String = r.get(8)?;
    let created_str: String = r.get(9)?;
    let updated_str: String = r.get(10)?;
    let completed_str: Option<String> = r.get(11)?;

    let status = Status::from_str(&status_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(StringErr(e)),
        )
    })?;
    let priority = Priority::from_str(&priority_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            Box::new(StringErr(e)),
        )
    })?;
    let due_date = match due_str {
        Some(s) if !s.is_empty() => {
            Some(NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?)
        }
        _ => None,
    };
    let created_at = parse_dt(&created_str, 9)?;
    let updated_at = parse_dt(&updated_str, 10)?;
    let completed_at = match completed_str {
        Some(s) if !s.is_empty() => Some(parse_dt(&s, 11)?),
        _ => None,
    };

    Ok(Task {
        id: r.get(0)?,
        parent_id: r.get(1)?,
        project_id: r.get(2)?,
        title: r.get(3)?,
        description: r.get(4)?,
        status,
        priority,
        due_date,
        tags: split_tags(&tags_str),
        created_at,
        updated_at,
        completed_at,
    })
}

fn parse_dt(s: &str, col: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
        })
}

fn join_tags(tags: &[String]) -> String {
    tags.join(",")
}

fn split_tags(s: &str) -> Vec<String> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    }
}

#[derive(Debug)]
struct StringErr(String);
impl std::fmt::Display for StringErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for StringErr {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_list_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        db.ensure_projects(&["A".to_string()]).unwrap();
        let pid = db.project_id_by_name("A").unwrap();
        let mut t = Task::new_now("hello");
        t.project_id = pid;
        t.tags = vec!["x".into(), "y".into()];
        let id = db.insert_task(&t).unwrap();
        let got = db.get_task(id).unwrap().unwrap();
        assert_eq!(got.title, "hello");
        assert_eq!(got.tags, vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn cascade_deletes_subtasks() {
        let db = Db::open_in_memory().unwrap();
        let parent = Task::new_now("p");
        let pid = db.insert_task(&parent).unwrap();
        let mut child = Task::new_now("c");
        child.parent_id = Some(pid);
        let cid = db.insert_task(&child).unwrap();
        db.delete_task(pid).unwrap();
        assert!(db.get_task(cid).unwrap().is_none());
    }

    #[test]
    fn seed_runs_only_when_empty() {
        let db = Db::open_in_memory().unwrap();
        db.ensure_projects(&["論文誌".into(), "PBL".into(), "研究".into()])
            .unwrap();
        assert!(db.seed_if_empty().unwrap());
        assert!(!db.seed_if_empty().unwrap());
    }
}
