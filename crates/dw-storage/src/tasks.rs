// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Projects, tasks, subtasks, comments and manually logged task effort.
//!
//! Everything here is scoped by organisation: a project belongs to exactly one
//! organisation, a task belongs to a project. The only link to the time world
//! is `projects.account_id`, which lets a project report the hours booked on a
//! time account. No punch data is written from this module.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{EPOCH_DATE, Result, StorageError, Store, escape_like, now_id};
use dw_core::Millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub description: String,
    pub color: Option<String>,
    /// Optional link to a time account; booked hours are read from there.
    pub account_id: Option<String>,
    pub status: String,
    pub created_by: Option<String>,
    pub created_at: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    /// Set for a subtask, always NULL for a top-level task.
    pub parent_id: Option<String>,
    pub title: String,
    pub description: String,
    pub assignee_id: Option<String>,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    pub priority: String,
    pub status: String,
    pub estimate_minutes: Option<i64>,
    pub created_by: Option<String>,
    pub created_at: Millis,
    pub updated_at: Millis,
    pub completed_at: Option<Millis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComment {
    pub id: String,
    pub task_id: String,
    pub author_id: Option<String>,
    pub author_name: String,
    pub body: String,
    pub created_at: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTimeEntry {
    pub id: String,
    pub task_id: String,
    pub user_id: Option<String>,
    pub user_name: String,
    pub minutes: i64,
    pub note: String,
    pub spent_on: String,
    pub created_at: Millis,
}

/// A task with the aggregates the lists need.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRow {
    #[serde(flatten)]
    pub task: Task,
    pub project_name: String,
    pub assignee_name: Option<String>,
    pub subtask_count: i64,
    pub done_subtasks: i64,
    /// Manually logged effort.
    pub spent_minutes: i64,
    pub comment_count: i64,
}

/// A project with its aggregates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRow {
    #[serde(flatten)]
    pub project: Project,
    pub account_name: Option<String>,
    pub task_count: i64,
    pub open_count: i64,
    pub done_count: i64,
    pub overdue_count: i64,
    /// Sum of the task estimates.
    pub estimate_minutes: i64,
    /// Sum of the manually logged task effort.
    pub spent_minutes: i64,
    /// Hours booked on the linked time account (all employees).
    pub booked_ms: Millis,
}

#[derive(Debug, Clone, Default)]
pub struct TaskFilter<'a> {
    pub org_id: &'a str,
    pub project_id: Option<&'a str>,
    pub assignee_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub priority: Option<&'a str>,
    /// Free text over title, description and comments.
    pub q: Option<&'a str>,
    /// Only top-level tasks (subtasks are shown inside their parent).
    pub parents_only: bool,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatsRow {
    pub id: String,
    pub name: String,
    pub task_count: i64,
    pub done_count: i64,
    pub overdue_count: i64,
    pub estimate_minutes: i64,
    pub spent_minutes: i64,
    pub booked_ms: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    pub from: String,
    pub to: String,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub done_tasks: i64,
    pub overdue_tasks: i64,
    pub estimate_minutes: i64,
    pub spent_minutes: i64,
    pub booked_ms: Millis,
    pub by_project: Vec<TaskStatsRow>,
    pub by_assignee: Vec<TaskStatsRow>,
}

const TASK_COLUMNS: &str = "t.id, t.project_id, t.parent_id, t.title, t.description, t.assignee_id,
     t.start_date, t.due_date, t.priority, t.status, t.estimate_minutes,
     t.created_by, t.created_at, t.updated_at, t.completed_at";

fn map_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        org_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        color: row.get(4)?,
        account_id: row.get(5)?,
        status: row.get(6)?,
        created_by: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        parent_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        assignee_id: row.get(5)?,
        start_date: row.get(6)?,
        due_date: row.get(7)?,
        priority: row.get(8)?,
        status: row.get(9)?,
        estimate_minutes: row.get(10)?,
        created_by: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        completed_at: row.get(14)?,
    })
}

fn map_task_row(row: &Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        task: map_task(row)?,
        project_name: row.get(15)?,
        assignee_name: row.get(16)?,
        subtask_count: row.get(17)?,
        done_subtasks: row.get(18)?,
        spent_minutes: row.get(19)?,
        comment_count: row.get(20)?,
    })
}

fn validate_task_fields(
    title: &str,
    start_date: Option<&str>,
    due_date: Option<&str>,
    estimate_minutes: Option<i64>,
) -> Result<()> {
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err(StorageError::Conflict(
            "title must be between 1 and 200 characters".into(),
        ));
    }
    if let (Some(start), Some(due)) = (start_date, due_date) {
        if due < start {
            return Err(StorageError::Conflict(
                "due date cannot be before the start date".into(),
            ));
        }
    }
    if let Some(minutes) = estimate_minutes {
        if !(0..=600_000).contains(&minutes) {
            return Err(StorageError::Conflict(
                "estimate must be between 0 and 1000 hours".into(),
            ));
        }
    }
    Ok(())
}

impl Store {
    // ---- projects ------------------------------------------------------

    pub fn projects(&self, org_id: &str, include_archived: bool) -> Result<Vec<ProjectRow>> {
        let conn = self.lock();
        let sql = format!(
            "SELECT p.id, p.org_id, p.name, p.description, p.color, p.account_id, p.status,
                    p.created_by, p.created_at,
                    a.name,
                    (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.parent_id IS NULL),
                    (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.parent_id IS NULL
                       AND t.status NOT IN ('done','cancelled')),
                    (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.status = 'done'),
                    (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.status NOT IN ('done','cancelled')
                       AND t.due_date IS NOT NULL AND t.due_date < date('now')),
                    (SELECT COALESCE(SUM(t.estimate_minutes),0) FROM tasks t WHERE t.project_id = p.id),
                    (SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e
                       JOIN tasks t2 ON t2.id = e.task_id WHERE t2.project_id = p.id)
               FROM projects p
               LEFT JOIN accounts a ON a.id = p.account_id
              WHERE p.org_id = ?1 {} 
              ORDER BY p.status, p.name COLLATE NOCASE",
            if include_archived { "" } else { "AND p.status = 'active'" }
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![org_id], |row| {
            Ok(ProjectRow {
                project: map_project(row)?,
                account_name: row.get(9)?,
                task_count: row.get(10)?,
                open_count: row.get(11)?,
                done_count: row.get(12)?,
                overdue_count: row.get(13)?,
                estimate_minutes: row.get(14)?,
                spent_minutes: row.get(15)?,
                booked_ms: 0,
            })
        })?;
        let mut out = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        // booked hours come from the linked time account
        for row in &mut out {
            if let Some(account_id) = &row.project.account_id {
                row.booked_ms = account_worked_ms(&conn, account_id, EPOCH_DATE, "9999-12-31", 0)?;
            }
        }
        Ok(out)
    }

    pub fn project(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, org_id, name, description, color, account_id, status, created_by, created_at
                   FROM projects WHERE id = ?1",
                params![id],
                map_project,
            )
            .optional()?)
    }

    pub fn create_project(
        &self,
        org_id: &str,
        name: &str,
        description: &str,
        color: Option<&str>,
        account_id: Option<&str>,
        created_by: &str,
        at: Millis,
    ) -> Result<Project> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 120 {
            return Err(StorageError::Conflict(
                "name must be between 1 and 120 characters".into(),
            ));
        }
        let id = now_id();
        self.lock()
            .execute(
                "INSERT INTO projects (id, org_id, name, description, color, account_id, status, created_by, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8)",
                params![id, org_id, trimmed, description.trim(), color, account_id, created_by, at],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StorageError::Conflict("a project with this name already exists".into())
                }
                other => StorageError::Sql(other),
            })?;
        self.project(&id)?.ok_or(StorageError::NotFound)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_project(
        &self,
        id: &str,
        name: &str,
        description: &str,
        color: Option<&str>,
        account_id: Option<&str>,
        status: &str,
    ) -> Result<Project> {
        if !matches!(status, "active" | "archived") {
            return Err(StorageError::Conflict("unknown project status".into()));
        }
        self.lock()
            .execute(
                "UPDATE projects SET name = ?2, description = ?3, color = ?4,
                        account_id = ?5, status = ?6 WHERE id = ?1",
                params![
                    id,
                    name.trim(),
                    description.trim(),
                    color,
                    account_id,
                    status
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StorageError::Conflict("a project with this name already exists".into())
                }
                other => StorageError::Sql(other),
            })?;
        self.project(id)?.ok_or(StorageError::NotFound)
    }

    /// Removes the project with all its tasks, subtasks, comments and time
    /// entries. The linked time account is only unlinked, never deleted.
    pub fn delete_project(&self, id: &str) -> Result<()> {
        let changed = self
            .lock()
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// What would be lost by deleting this project.
    pub fn project_delete_preview(&self, id: &str) -> Result<(i64, i64, i64, i64)> {
        let conn = self.lock();
        let tasks: i64 = conn.query_row(
            "SELECT count(*) FROM tasks WHERE project_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let subtasks: i64 = conn.query_row(
            "SELECT count(*) FROM tasks WHERE project_id = ?1 AND parent_id IS NOT NULL",
            params![id],
            |r| r.get(0),
        )?;
        let comments: i64 = conn.query_row(
            "SELECT count(*) FROM task_comments c JOIN tasks t ON t.id = c.task_id WHERE t.project_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let minutes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e
               JOIN tasks t ON t.id = e.task_id WHERE t.project_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        Ok((tasks, subtasks, comments, minutes))
    }

    // ---- tasks ---------------------------------------------------------

    pub fn tasks(&self, filter: &TaskFilter<'_>) -> Result<Vec<TaskRow>> {
        let mut sql = format!(
            "SELECT {TASK_COLUMNS}, p.name, u.name,
                    (SELECT count(*) FROM tasks c WHERE c.parent_id = t.id),
                    (SELECT count(*) FROM tasks c WHERE c.parent_id = t.id AND c.status = 'done'),
                    (SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e WHERE e.task_id = t.id),
                    (SELECT count(*) FROM task_comments cm WHERE cm.task_id = t.id)
               FROM tasks t
               JOIN projects p ON p.id = t.project_id
               LEFT JOIN users u ON u.id = t.assignee_id
              WHERE p.org_id = ?1"
        );
        let mut values: Vec<rusqlite::types::Value> =
            vec![rusqlite::types::Value::Text(filter.org_id.to_string())];

        fn add(
            sql: &mut String,
            values: &mut Vec<rusqlite::types::Value>,
            value: rusqlite::types::Value,
            text: &str,
        ) {
            values.push(value);
            sql.push_str(&text.replace("{}", &values.len().to_string()));
        }

        if filter.parents_only {
            sql.push_str(" AND t.parent_id IS NULL");
        }
        if let Some(project) = filter.project_id {
            add(
                &mut sql,
                &mut values,
                rusqlite::types::Value::Text(project.to_string()),
                " AND t.project_id = ?{}",
            );
        }
        if let Some(assignee) = filter.assignee_id {
            add(
                &mut sql,
                &mut values,
                rusqlite::types::Value::Text(assignee.to_string()),
                " AND t.assignee_id = ?{}",
            );
        }
        if let Some(status) = filter.status {
            add(
                &mut sql,
                &mut values,
                rusqlite::types::Value::Text(status.to_string()),
                " AND t.status = ?{}",
            );
        }
        if let Some(priority) = filter.priority {
            add(
                &mut sql,
                &mut values,
                rusqlite::types::Value::Text(priority.to_string()),
                " AND t.priority = ?{}",
            );
        }
        if let Some(q) = filter.q {
            let pattern = format!("%{}%", escape_like(&q.to_lowercase()));
            let index = values.len() + 1;
            values.push(rusqlite::types::Value::Text(pattern));
            sql.push_str(&format!(
                " AND (dw_lower(t.title) LIKE ?{index} ESCAPE '\\'
                    OR dw_lower(t.description) LIKE ?{index} ESCAPE '\\'
                    OR EXISTS (SELECT 1 FROM task_comments cm2
                                WHERE cm2.task_id = t.id
                                  AND dw_lower(cm2.body) LIKE ?{index} ESCAPE '\\'))"
            ));
        }

        sql.push_str(
            " ORDER BY (t.due_date IS NULL), t.due_date,
                 CASE t.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1
                                 WHEN 'normal' THEN 2 ELSE 3 END,
                 t.created_at DESC",
        );
        add(
            &mut sql,
            &mut values,
            rusqlite::types::Value::Integer(filter.limit.clamp(1, 500)),
            " LIMIT ?{}",
        );

        let conn = self.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), map_task_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn task(&self, id: &str) -> Result<Option<Task>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                &format!("SELECT {TASK_COLUMNS} FROM tasks t WHERE t.id = ?1"),
                params![id],
                map_task,
            )
            .optional()?)
    }

    pub fn task_row(&self, id: &str) -> Result<Option<TaskRow>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                &format!(
                    "SELECT {TASK_COLUMNS}, p.name, u.name,
                            (SELECT count(*) FROM tasks c WHERE c.parent_id = t.id),
                            (SELECT count(*) FROM tasks c WHERE c.parent_id = t.id AND c.status = 'done'),
                            (SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e WHERE e.task_id = t.id),
                            (SELECT count(*) FROM task_comments cm WHERE cm.task_id = t.id)
                       FROM tasks t
                       JOIN projects p ON p.id = t.project_id
                       LEFT JOIN users u ON u.id = t.assignee_id
                      WHERE t.id = ?1"
                ),
                params![id],
                map_task_row,
            )
            .optional()?)
    }

    pub fn subtasks(&self, parent_id: &str) -> Result<Vec<Task>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks t WHERE t.parent_id = ?1 ORDER BY t.created_at"
        ))?;
        let rows = stmt.query_map(params![parent_id], map_task)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Creates a task or a subtask. `parent_id` must point at a top-level task
    /// of the same project (one level only).
    #[allow(clippy::too_many_arguments)]
    pub fn create_task(
        &self,
        project_id: &str,
        parent_id: Option<&str>,
        title: &str,
        description: &str,
        assignee_id: Option<&str>,
        start_date: Option<&str>,
        due_date: Option<&str>,
        priority: &str,
        estimate_minutes: Option<i64>,
        created_by: &str,
        at: Millis,
    ) -> Result<Task> {
        validate_task_fields(title, start_date, due_date, estimate_minutes)?;
        if !matches!(priority, "low" | "normal" | "high" | "urgent") {
            return Err(StorageError::Conflict("unknown priority".into()));
        }
        if let Some(parent) = parent_id {
            let parent_task = self.task(parent)?.ok_or(StorageError::NotFound)?;
            if parent_task.project_id != project_id {
                return Err(StorageError::Conflict(
                    "subtask must belong to the same project".into(),
                ));
            }
            if parent_task.parent_id.is_some() {
                return Err(StorageError::Conflict(
                    "subtasks cannot be nested further".into(),
                ));
            }
        }

        let id = now_id();
        self.lock().execute(
            "INSERT INTO tasks (id, project_id, parent_id, title, description, assignee_id,
                                start_date, due_date, priority, status, estimate_minutes,
                                created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'open', ?10, ?11, ?12, ?12)",
            params![
                id,
                project_id,
                parent_id,
                title.trim(),
                description.trim(),
                assignee_id,
                start_date,
                due_date,
                priority,
                estimate_minutes,
                created_by,
                at
            ],
        )?;
        self.task(&id)?.ok_or(StorageError::NotFound)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_task(
        &self,
        id: &str,
        title: &str,
        description: &str,
        assignee_id: Option<&str>,
        start_date: Option<&str>,
        due_date: Option<&str>,
        priority: &str,
        status: &str,
        estimate_minutes: Option<i64>,
        at: Millis,
    ) -> Result<Task> {
        validate_task_fields(title, start_date, due_date, estimate_minutes)?;
        if !matches!(priority, "low" | "normal" | "high" | "urgent") {
            return Err(StorageError::Conflict("unknown priority".into()));
        }
        if !matches!(status, "open" | "in_progress" | "done" | "cancelled") {
            return Err(StorageError::Conflict("unknown status".into()));
        }

        let completed_at = if status == "done" { Some(at) } else { None };
        let changed = self.lock().execute(
            "UPDATE tasks SET title = ?2, description = ?3, assignee_id = ?4, start_date = ?5,
                    due_date = ?6, priority = ?7, status = ?8, estimate_minutes = ?9,
                    updated_at = ?10, completed_at = ?11
              WHERE id = ?1",
            params![
                id,
                title.trim(),
                description.trim(),
                assignee_id,
                start_date,
                due_date,
                priority,
                status,
                estimate_minutes,
                at,
                completed_at
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        self.task(id)?.ok_or(StorageError::NotFound)
    }

    pub fn delete_task(&self, id: &str) -> Result<()> {
        let changed = self
            .lock()
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    // ---- comments ------------------------------------------------------

    pub fn task_comments(&self, task_id: &str) -> Result<Vec<TaskComment>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.task_id, c.author_id, COALESCE(u.name, ''), c.body, c.created_at
               FROM task_comments c
               LEFT JOIN users u ON u.id = c.author_id
              WHERE c.task_id = ?1 ORDER BY c.created_at",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(TaskComment {
                id: row.get(0)?,
                task_id: row.get(1)?,
                author_id: row.get(2)?,
                author_name: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn comment(&self, id: &str) -> Result<Option<TaskComment>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT c.id, c.task_id, c.author_id, COALESCE(u.name, ''), c.body, c.created_at
                   FROM task_comments c LEFT JOIN users u ON u.id = c.author_id WHERE c.id = ?1",
                params![id],
                |row| {
                    Ok(TaskComment {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        author_id: row.get(2)?,
                        author_name: row.get(3)?,
                        body: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn add_task_comment(
        &self,
        task_id: &str,
        author_id: &str,
        body: &str,
        at: Millis,
    ) -> Result<TaskComment> {
        let text = body.trim();
        if text.is_empty() || text.chars().count() > 4000 {
            return Err(StorageError::Conflict(
                "comment must be between 1 and 4000 characters".into(),
            ));
        }
        let id = now_id();
        self.lock().execute(
            "INSERT INTO task_comments (id, task_id, author_id, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, task_id, author_id, text, at],
        )?;
        self.comment(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn delete_task_comment(&self, id: &str) -> Result<()> {
        let changed = self
            .lock()
            .execute("DELETE FROM task_comments WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    // ---- manually logged effort ----------------------------------------

    pub fn task_time_entries(&self, task_id: &str) -> Result<Vec<TaskTimeEntry>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT e.id, e.task_id, e.user_id, COALESCE(u.name, ''), e.minutes, e.note,
                    e.spent_on, e.created_at
               FROM task_time_entries e
               LEFT JOIN users u ON u.id = e.user_id
              WHERE e.task_id = ?1 ORDER BY e.spent_on DESC, e.created_at DESC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(TaskTimeEntry {
                id: row.get(0)?,
                task_id: row.get(1)?,
                user_id: row.get(2)?,
                user_name: row.get(3)?,
                minutes: row.get(4)?,
                note: row.get(5)?,
                spent_on: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn time_entry(&self, id: &str) -> Result<Option<TaskTimeEntry>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT e.id, e.task_id, e.user_id, COALESCE(u.name, ''), e.minutes, e.note,
                        e.spent_on, e.created_at
                   FROM task_time_entries e LEFT JOIN users u ON u.id = e.user_id WHERE e.id = ?1",
                params![id],
                |row| {
                    Ok(TaskTimeEntry {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        user_id: row.get(2)?,
                        user_name: row.get(3)?,
                        minutes: row.get(4)?,
                        note: row.get(5)?,
                        spent_on: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn add_task_time_entry(
        &self,
        task_id: &str,
        user_id: &str,
        minutes: i64,
        note: &str,
        spent_on: &str,
        at: Millis,
    ) -> Result<TaskTimeEntry> {
        if !(1..=1440).contains(&minutes) {
            return Err(StorageError::Conflict(
                "minutes must be between 1 and 1440".into(),
            ));
        }
        if spent_on.len() != 10 {
            return Err(StorageError::Conflict("spent_on must be YYYY-MM-DD".into()));
        }
        let id = now_id();
        self.lock().execute(
            "INSERT INTO task_time_entries (id, task_id, user_id, minutes, note, spent_on, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, task_id, user_id, minutes, note.trim(), spent_on, at],
        )?;
        self.time_entry(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn delete_task_time_entry(&self, id: &str) -> Result<()> {
        let changed = self
            .lock()
            .execute("DELETE FROM task_time_entries WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    // ---- statistics ----------------------------------------------------

    pub fn task_stats(&self, org_id: &str, from: &str, to: &str) -> Result<TaskStats> {
        let conn = self.lock();

        let (total, open, done, overdue, estimate) = conn.query_row(
            "SELECT count(*),
                    COALESCE(SUM(t.status NOT IN ('done','cancelled')),0),
                    COALESCE(SUM(t.status = 'done'),0),
                    COALESCE(SUM(t.status NOT IN ('done','cancelled')
                                 AND t.due_date IS NOT NULL AND t.due_date < date('now')),0),
                    COALESCE(SUM(t.estimate_minutes),0)
               FROM tasks t JOIN projects p ON p.id = t.project_id
              WHERE p.org_id = ?1 AND t.parent_id IS NULL",
            params![org_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;

        let spent: i64 = conn.query_row(
            "SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e
               JOIN tasks t ON t.id = e.task_id
               JOIN projects p ON p.id = t.project_id
              WHERE p.org_id = ?1 AND e.spent_on BETWEEN ?2 AND ?3",
            params![org_id, from, to],
            |r| r.get(0),
        )?;

        let booked: i64 = conn.query_row(
            "SELECT COALESCE(SUM(MIN(COALESCE(s.ended_at, ?4), ?4) - s.started_at),0)
               FROM segments s
               JOIN accounts a ON a.id = s.account_id
               JOIN projects p ON p.account_id = a.id
               JOIN workdays w ON w.id = s.workday_id
              WHERE p.org_id = ?1 AND w.date BETWEEN ?2 AND ?3",
            params![org_id, from, to, now_millis()],
            |r| r.get(0),
        )?;

        let by_project_rows = conn
            .prepare(
                "SELECT p.id, p.name,
                        (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.parent_id IS NULL),
                        (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.status = 'done'),
                        (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.status NOT IN ('done','cancelled')
                           AND t.due_date IS NOT NULL AND t.due_date < date('now')),
                        (SELECT COALESCE(SUM(t.estimate_minutes),0) FROM tasks t WHERE t.project_id = p.id),
                        (SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e
                           JOIN tasks t2 ON t2.id = e.task_id
                          WHERE t2.project_id = p.id AND e.spent_on BETWEEN ?2 AND ?3),
                        p.account_id
                   FROM projects p WHERE p.org_id = ?1 ORDER BY p.name COLLATE NOCASE",
            )?
            .query_map(params![org_id, from, to], |row| {
                let account_id: Option<String> = row.get(7)?;
                Ok((
                    TaskStatsRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        task_count: row.get(2)?,
                        done_count: row.get(3)?,
                        overdue_count: row.get(4)?,
                        estimate_minutes: row.get(5)?,
                        spent_minutes: row.get(6)?,
                        booked_ms: 0,
                    },
                    account_id,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut by_project: Vec<TaskStatsRow> = Vec::new();
        for (mut row, account_id) in by_project_rows {
            if let Some(account_id) = account_id {
                row.booked_ms = account_worked_ms(&conn, &account_id, from, to, now_millis())?;
            }
            by_project.push(row);
        }

        let by_assignee = conn
            .prepare(
                "SELECT COALESCE(u.id,''), COALESCE(u.name, 'Nicht zugewiesen'),
                        count(*),
                        COALESCE(SUM(t.status = 'done'),0),
                        COALESCE(SUM(t.status NOT IN ('done','cancelled')
                                     AND t.due_date IS NOT NULL AND t.due_date < date('now')),0),
                        COALESCE(SUM(t.estimate_minutes),0),
                        (SELECT COALESCE(SUM(e.minutes),0) FROM task_time_entries e
                           JOIN tasks t2 ON t2.id = e.task_id
                          WHERE t2.assignee_id IS u.id AND e.spent_on BETWEEN ?2 AND ?3)
                   FROM tasks t
                   JOIN projects p ON p.id = t.project_id
                   LEFT JOIN users u ON u.id = t.assignee_id
                  WHERE p.org_id = ?1 AND t.parent_id IS NULL
                  GROUP BY u.id ORDER BY u.name COLLATE NOCASE",
            )?
            .query_map(params![org_id, from, to], |row| {
                Ok(TaskStatsRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    task_count: row.get(2)?,
                    done_count: row.get(3)?,
                    overdue_count: row.get(4)?,
                    estimate_minutes: row.get(5)?,
                    spent_minutes: row.get(6)?,
                    booked_ms: 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(TaskStats {
            from: from.to_string(),
            to: to.to_string(),
            total_tasks: total,
            open_tasks: open,
            done_tasks: done,
            overdue_tasks: overdue,
            estimate_minutes: estimate,
            spent_minutes: spent,
            booked_ms: booked,
            by_project,
            by_assignee,
        })
    }
}

fn now_millis() -> Millis {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as Millis)
        .unwrap_or(0)
}

/// Hours booked on one time account within a date range, across all employees.
fn account_worked_ms(
    conn: &Connection,
    account_id: &str,
    from: &str,
    to: &str,
    now: Millis,
) -> Result<Millis> {
    let now = if now == 0 { now_millis() } else { now };
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(MIN(COALESCE(s.ended_at, ?4), ?4) - s.started_at), 0)
           FROM segments s
           JOIN workdays w ON w.id = s.workday_id
          WHERE s.account_id = ?1 AND w.date BETWEEN ?2 AND ?3",
        params![account_id, from, to, now],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewUser, Org, User};
    use dw_core::{AccountKind, Action, Role};

    const NOW: Millis = 1_760_000_000_000;
    const DATE: &str = "2026-10-21";

    fn setup() -> (Store, Org, User) {
        let store = Store::in_memory().unwrap();
        let org = store
            .create_org("Test GmbH", "test", "Europe/Berlin", 40.0, 5)
            .unwrap();
        store.ensure_default_accounts(&org.id).unwrap();
        let user = store
            .create_user(&NewUser {
                org_id: org.id.clone(),
                email: "a@example.org".into(),
                name: "Anna".into(),
                password_hash: "x".into(),
                role: Role::Employee,
                locale: "de".into(),
            })
            .unwrap();
        (store, org, user)
    }

    fn project(store: &Store, org: &Org, user: &User, name: &str) -> Project {
        store
            .create_project(&org.id, name, "Beschreibung", None, None, &user.id, NOW)
            .unwrap()
    }

    fn task(store: &Store, project: &Project, user: &User, title: &str) -> Task {
        store
            .create_task(
                &project.id,
                None,
                title,
                "",
                Some(&user.id),
                Some("2026-10-01"),
                Some("2026-10-31"),
                "normal",
                Some(120),
                &user.id,
                NOW,
            )
            .unwrap()
    }

    #[test]
    fn tasks_and_subtasks_carry_their_aggregates() {
        let (store, org, user) = setup();
        let project = project(&store, &org, &user, "Badsanierung");
        let parent = task(&store, &project, &user, "Fliesen legen");
        store
            .create_task(
                &project.id,
                Some(&parent.id),
                "Fliesenkleber holen",
                "",
                None,
                None,
                None,
                "low",
                None,
                &user.id,
                NOW,
            )
            .unwrap();

        let rows = store
            .tasks(&TaskFilter {
                org_id: &org.id,
                parents_only: true,
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subtask_count, 1);
        assert_eq!(rows[0].done_subtasks, 0);
        assert_eq!(rows[0].project_name, "Badsanierung");
        assert_eq!(rows[0].assignee_name.as_deref(), Some("Anna"));

        // subtasks are listed separately, not as top-level rows
        let subs = store.subtasks(&parent.id).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].title, "Fliesenkleber holen");
    }

    #[test]
    fn subtask_rules_are_enforced() {
        let (store, org, user) = setup();
        let p1 = project(&store, &org, &user, "P1");
        let p2 = project(&store, &org, &user, "P2");
        let parent = task(&store, &p1, &user, "Parent");
        let child = store
            .create_task(
                &p1.id,
                Some(&parent.id),
                "Child",
                "",
                None,
                None,
                None,
                "normal",
                None,
                &user.id,
                NOW,
            )
            .unwrap();

        // a subtask of a subtask is rejected
        let nested = store.create_task(
            &p1.id,
            Some(&child.id),
            "Grandchild",
            "",
            None,
            None,
            None,
            "normal",
            None,
            &user.id,
            NOW,
        );
        assert!(matches!(nested, Err(StorageError::Conflict(_))));

        // a subtask must stay in its project
        let foreign = store.create_task(
            &p2.id,
            Some(&parent.id),
            "Fremd",
            "",
            None,
            None,
            None,
            "normal",
            None,
            &user.id,
            NOW,
        );
        assert!(matches!(foreign, Err(StorageError::Conflict(_))));

        // due before start is rejected
        let bad_dates = store.create_task(
            &p1.id,
            None,
            "Datum",
            "",
            None,
            Some("2026-10-10"),
            Some("2026-10-01"),
            "normal",
            None,
            &user.id,
            NOW,
        );
        assert!(matches!(bad_dates, Err(StorageError::Conflict(_))));
    }

    #[test]
    fn effort_is_logged_manually_and_summed() {
        let (store, org, user) = setup();
        let project = project(&store, &org, &user, "P");
        let t = task(&store, &project, &user, "Aufgabe");

        let ninety = store
            .add_task_time_entry(&t.id, &user.id, 90, "vormittags", DATE, NOW)
            .unwrap();
        store
            .add_task_time_entry(&t.id, &user.id, 30, "", DATE, NOW)
            .unwrap();

        let entries = store.task_time_entries(&t.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].user_name, "Anna");

        let row = store.task_row(&t.id).unwrap().unwrap();
        assert_eq!(row.spent_minutes, 120);
        assert_eq!(row.task.estimate_minutes, Some(120));

        // zero minutes are rejected
        assert!(matches!(
            store.add_task_time_entry(&t.id, &user.id, 0, "", DATE, NOW),
            Err(StorageError::Conflict(_))
        ));

        store.delete_task_time_entry(&ninety.id).unwrap();
        assert_eq!(store.task_row(&t.id).unwrap().unwrap().spent_minutes, 30);
    }

    #[test]
    fn comments_are_stored_and_searchable() {
        let (store, org, user) = setup();
        let project = project(&store, &org, &user, "P");
        let t = task(&store, &project, &user, "Küche streichen");

        store
            .add_task_comment(&t.id, &user.id, "Farbe für Müller bestellt", NOW)
            .unwrap();
        let comments = store.task_comments(&t.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].author_name, "Anna");

        // search finds it through the comment, umlauts fold
        let by_comment = store
            .tasks(&TaskFilter {
                org_id: &org.id,
                q: Some("müller"),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_comment.len(), 1);

        let by_title = store
            .tasks(&TaskFilter {
                org_id: &org.id,
                q: Some("KÜCHE"),
                limit: 50,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_title.len(), 1);

        assert!(
            store
                .tasks(&TaskFilter {
                    org_id: &org.id,
                    q: Some("gibtsnicht"),
                    limit: 50,
                    ..Default::default()
                })
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_project_reports_the_hours_booked_on_its_time_account() {
        let (store, org, user) = setup();
        let account = store
            .create_account(&org.id, "Badsanierung", AccountKind::Work, None, 100, false)
            .unwrap();
        let project = store
            .create_project(
                &org.id,
                "Badsanierung",
                "",
                None,
                Some(&account.id),
                &user.id,
                NOW,
            )
            .unwrap();

        store
            .punch(
                &user,
                &Action::Start {
                    account_id: account.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 5_400_000, DATE, "test")
            .unwrap();

        let rows = store.projects(&org.id, false).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].account_name.as_deref(), Some("Badsanierung"));
        assert_eq!(rows[0].booked_ms, 5_400_000, "1.5 h booked on the account");
        assert_eq!(rows[0].project.id, project.id);
    }

    #[test]
    fn deleting_a_project_cascades_and_unlinks_the_account() {
        let (store, org, user) = setup();
        let account = store
            .create_account(&org.id, "Konto", AccountKind::Work, None, 100, false)
            .unwrap();
        let project = store
            .create_project(&org.id, "P", "", None, Some(&account.id), &user.id, NOW)
            .unwrap();
        let t = task(&store, &project, &user, "Aufgabe");
        store
            .add_task_comment(&t.id, &user.id, "Notiz", NOW)
            .unwrap();
        store
            .add_task_time_entry(&t.id, &user.id, 60, "", DATE, NOW)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: account.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 60_000, DATE, "test")
            .unwrap();

        let (tasks, _subtasks, comments, minutes) =
            store.project_delete_preview(&project.id).unwrap();
        assert_eq!((tasks, comments, minutes), (1, 1, 60));

        store.delete_project(&project.id).unwrap();

        assert!(store.project(&project.id).unwrap().is_none());
        assert!(store.task(&t.id).unwrap().is_none());
        assert!(store.task_comments(&t.id).unwrap().is_empty());
        // the time account and its booked segments survive
        assert!(store.account(&account.id).unwrap().is_some());
        let days = store
            .days_between(&user.id, &org.id, DATE, DATE, NOW)
            .unwrap();
        assert_eq!(days[0].segments.len(), 1);
    }

    #[test]
    fn statistics_group_by_project_and_assignee() {
        let (store, org, user) = setup();
        let p = project(&store, &org, &user, "P");
        let t = task(&store, &p, &user, "A");
        store
            .update_task(
                &t.id,
                "A",
                "",
                Some(&user.id),
                None,
                None,
                "high",
                "done",
                Some(60),
                NOW,
            )
            .unwrap();
        store
            .add_task_time_entry(&t.id, &user.id, 90, "", DATE, NOW)
            .unwrap();
        task(&store, &p, &user, "B");

        let stats = store
            .task_stats(&org.id, "2026-10-01", "2026-10-31")
            .unwrap();
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.done_tasks, 1);
        assert_eq!(stats.open_tasks, 1);
        assert_eq!(stats.estimate_minutes, 180);
        assert_eq!(stats.spent_minutes, 90);
        assert_eq!(stats.by_project.len(), 1);
        assert_eq!(stats.by_project[0].name, "P");
        assert_eq!(stats.by_project[0].done_count, 1);
        assert_eq!(stats.by_assignee.len(), 1);
        assert_eq!(stats.by_assignee[0].spent_minutes, 90);
    }
}
