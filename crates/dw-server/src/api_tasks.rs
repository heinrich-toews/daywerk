// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! HTTP API for the task world: projects, tasks, subtasks, comments and
//! manually logged effort.
//!
//! Everything is scoped to the actor's organisation. Writing projects is for
//! office and admin; tasks are open to everyone, because the office is small and
//! the board is shared.

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use dw_storage::{
    Project, ProjectRow, Task, TaskComment, TaskFilter, TaskRow, TaskStats, TaskTimeEntry,
};

use crate::{ApiError, AppState, require_auth, require_role};
use dw_core::Role;

#[derive(Deserialize, Default)]
pub struct ProjectParams {
    #[serde(default)]
    pub include_archived: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct TaskParams {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub mine: Option<bool>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NewProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// Empty string clears the link.
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct NewTaskRequest {
    pub project_id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
}

#[derive(Deserialize)]
pub struct NewCommentRequest {
    pub body: String,
}

#[derive(Deserialize)]
pub struct NewTimeEntryRequest {
    pub minutes: i64,
    #[serde(default)]
    pub note: String,
    pub spent_on: String,
}

#[derive(Serialize)]
pub struct ProjectDetail {
    pub project: ProjectRow,
    pub tasks: Vec<TaskRow>,
    /// Open time accounts that can be linked.
    pub accounts: Vec<AccountOption>,
}

#[derive(Serialize)]
pub struct AccountOption {
    pub id: String,
    pub name: String,
    pub archived: bool,
}

#[derive(Serialize)]
pub struct TaskDetail {
    pub task: TaskRow,
    pub subtasks: Vec<Task>,
    pub comments: Vec<TaskComment>,
    pub time_entries: Vec<TaskTimeEntry>,
}

#[derive(Serialize)]
pub struct Member {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct DeletePreview {
    pub tasks: i64,
    pub subtasks: i64,
    pub comments: i64,
    pub minutes: i64,
    pub account_name: Option<String>,
}

fn project_row(state: &AppState, org_id: &str, id: &str) -> Result<ProjectRow, ApiError> {
    state
        .store
        .projects(org_id, true)?
        .into_iter()
        .find(|row| row.project.id == id)
        .ok_or(ApiError::NotFound)
}

fn load_project(state: &AppState, org_id: &str, id: &str) -> Result<Project, ApiError> {
    let project = state.store.project(id)?.ok_or(ApiError::NotFound)?;
    if project.org_id != org_id {
        return Err(ApiError::Forbidden);
    }
    Ok(project)
}

fn can_edit_task(actor_id: &str, role: Role, task: &Task) -> bool {
    role.sees_all()
        || task.created_by.as_deref() == Some(actor_id)
        || task.assignee_id.as_deref() == Some(actor_id)
}

/// Anyone in the organisation may be assigned a task, so everyone needs the
/// list of names — `GET /api/users` stays restricted.
pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Member>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let members = state
        .store
        .users_of_org(&actor.org_id)?
        .into_iter()
        .filter(|user| user.active)
        .map(|user| Member {
            id: user.id,
            name: user.name,
        })
        .collect();
    Ok(Json(members))
}

// ---------------------------------------------------------------- projects

pub async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ProjectParams>,
) -> Result<Json<Vec<ProjectRow>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    Ok(Json(state.store.projects(
        &actor.org_id,
        params.include_archived.unwrap_or(false),
    )?))
}

pub async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    let account_id = match req.account_id.as_deref() {
        Some(id) if !id.is_empty() => {
            let account = state.store.account(id)?.ok_or(ApiError::NotFound)?;
            if account.org_id != actor.org_id {
                return Err(ApiError::Forbidden);
            }
            Some(account.id)
        }
        _ => None,
    };

    let project = state.store.create_project(
        &actor.org_id,
        &req.name,
        &req.description,
        req.color.as_deref(),
        account_id.as_deref(),
        &actor.id,
        crate::now_ms(),
    )?;
    let _ = state.store.audit(
        &actor.org_id,
        &actor.id,
        "project_created",
        "project",
        &project.id,
        "{}",
        crate::now_ms(),
    );
    Ok(Json(project))
}

pub async fn get_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ProjectDetail>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let project = load_project(&state, &actor.org_id, &id)?;

    let tasks = state.store.tasks(&TaskFilter {
        org_id: &actor.org_id,
        project_id: Some(&project.id),
        parents_only: true,
        limit: 500,
        ..Default::default()
    })?;

    let accounts = state
        .store
        .accounts(&actor.org_id, false)?
        .into_iter()
        .map(|a| AccountOption {
            id: a.id,
            name: a.name,
            archived: a.archived,
        })
        .collect();

    Ok(Json(ProjectDetail {
        project: project_row(&state, &actor.org_id, &id)?,
        tasks,
        accounts,
    }))
}

pub async fn update_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;
    let current = load_project(&state, &actor.org_id, &id)?;

    // `account_id: ""` clears the link, `None` keeps it.
    let account_id = match req.account_id {
        None => current.account_id.clone(),
        Some(value) if value.is_empty() => None,
        Some(value) => {
            let account = state.store.account(&value)?.ok_or(ApiError::NotFound)?;
            if account.org_id != actor.org_id {
                return Err(ApiError::Forbidden);
            }
            Some(account.id)
        }
    };

    let project = state.store.update_project(
        &id,
        req.name.as_deref().unwrap_or(&current.name),
        req.description.as_deref().unwrap_or(&current.description),
        req.color.as_deref().or(current.color.as_deref()),
        account_id.as_deref(),
        req.status.as_deref().unwrap_or(&current.status),
    )?;
    let _ = state.store.audit(
        &actor.org_id,
        &actor.id,
        "project_updated",
        "project",
        &id,
        "{}",
        crate::now_ms(),
    );
    Ok(Json(project))
}

pub async fn project_delete_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<DeletePreview>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;
    let project = load_project(&state, &actor.org_id, &id)?;
    let (tasks, subtasks, comments, minutes) = state.store.project_delete_preview(&id)?;
    let account_name = match &project.account_id {
        Some(account_id) => state.store.account(account_id)?.map(|a| a.name),
        None => None,
    };
    Ok(Json(DeletePreview {
        tasks,
        subtasks,
        comments,
        minutes,
        account_name,
    }))
}

pub async fn delete_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;
    let project = load_project(&state, &actor.org_id, &id)?;

    state.store.delete_project(&id)?;
    let _ = state.store.audit(
        &actor.org_id,
        &actor.id,
        "project_deleted",
        "project",
        &id,
        &format!("{{\"name\":\"{}\"}}", project.name.replace('"', "'")),
        crate::now_ms(),
    );
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- tasks

pub async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<TaskParams>,
) -> Result<Json<Vec<TaskRow>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let assignee = if params.mine.unwrap_or(false) {
        Some(actor.id.clone())
    } else {
        params.assignee_id.clone()
    };

    Ok(Json(state.store.tasks(&TaskFilter {
        org_id: &actor.org_id,
        project_id: params.project_id.as_deref(),
        assignee_id: assignee.as_deref(),
        status: params.status.as_deref(),
        priority: params.priority.as_deref(),
        q: params.q.as_deref(),
        parents_only: true,
        limit: params.limit.unwrap_or(200),
    })?))
}

pub async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewTaskRequest>,
) -> Result<Json<Task>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    load_project(&state, &actor.org_id, &req.project_id)?;

    if let Some(assignee) = req.assignee_id.as_deref() {
        let user = state.store.user(assignee)?.ok_or(ApiError::NotFound)?;
        if user.org_id != actor.org_id {
            return Err(ApiError::Forbidden);
        }
    }

    let task = state.store.create_task(
        &req.project_id,
        req.parent_id.as_deref(),
        &req.title,
        &req.description,
        req.assignee_id.as_deref(),
        req.start_date.as_deref(),
        req.due_date.as_deref(),
        req.priority.as_deref().unwrap_or("normal"),
        req.estimate_minutes,
        &actor.id,
        crate::now_ms(),
    )?;
    Ok(Json(task))
}

pub async fn get_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<TaskDetail>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&id)?.ok_or(ApiError::NotFound)?;
    let project = load_project(&state, &actor.org_id, &task.project_id)?;
    let _ = project;

    Ok(Json(TaskDetail {
        task: state.store.task_row(&id)?.ok_or(ApiError::NotFound)?,
        subtasks: state.store.subtasks(&id)?,
        comments: state.store.task_comments(&id)?,
        time_entries: state.store.task_time_entries(&id)?,
    }))
}

pub async fn update_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<Task>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;
    if !can_edit_task(&actor.id, actor.role, &task) {
        return Err(ApiError::Forbidden);
    }

    if let Some(assignee) = req.assignee_id.as_deref() {
        let user = state.store.user(assignee)?.ok_or(ApiError::NotFound)?;
        if user.org_id != actor.org_id {
            return Err(ApiError::Forbidden);
        }
    }

    Ok(Json(state.store.update_task(
        &id,
        &req.title,
        &req.description,
        req.assignee_id.as_deref(),
        req.start_date.as_deref(),
        req.due_date.as_deref(),
        req.priority.as_deref().unwrap_or(&task.priority),
        req.status.as_deref().unwrap_or(&task.status),
        req.estimate_minutes,
        crate::now_ms(),
    )?))
}

pub async fn delete_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;
    if !can_edit_task(&actor.id, actor.role, &task) {
        return Err(ApiError::Forbidden);
    }
    state.store.delete_task(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- comments

pub async fn add_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NewCommentRequest>,
) -> Result<Json<TaskComment>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;

    Ok(Json(state.store.add_task_comment(
        &id,
        &actor.id,
        &req.body,
        crate::now_ms(),
    )?))
}

pub async fn delete_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((task_id, comment_id)): AxumPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&task_id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;

    let comment = state
        .store
        .comment(&comment_id)?
        .ok_or(ApiError::NotFound)?;
    if comment.author_id.as_deref() != Some(actor.id.as_str()) && !actor.role.sees_all() {
        return Err(ApiError::Forbidden);
    }
    state.store.delete_task_comment(&comment_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- effort

pub async fn add_time_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NewTimeEntryRequest>,
) -> Result<Json<TaskTimeEntry>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;

    Ok(Json(state.store.add_task_time_entry(
        &id,
        &actor.id,
        req.minutes,
        &req.note,
        &req.spent_on,
        crate::now_ms(),
    )?))
}

pub async fn delete_time_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((task_id, entry_id)): AxumPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let task = state.store.task(&task_id)?.ok_or(ApiError::NotFound)?;
    load_project(&state, &actor.org_id, &task.project_id)?;

    let entry = state
        .store
        .time_entry(&entry_id)?
        .ok_or(ApiError::NotFound)?;
    if entry.user_id.as_deref() != Some(actor.id.as_str()) && !actor.role.sees_all() {
        return Err(ApiError::Forbidden);
    }
    state.store.delete_task_time_entry(&entry_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- statistics

#[derive(Deserialize, Default)]
pub struct StatsParams {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

pub async fn task_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<StatsParams>,
) -> Result<Json<TaskStats>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let org = state.store.org(&actor.org_id)?.ok_or(ApiError::NotFound)?;
    let today = crate::local_date(crate::now_ms(), &org.timezone);
    let (from, to) = crate::month_range(&today);
    let from = params.from.unwrap_or(from);
    let to = params.to.unwrap_or(to);

    Ok(Json(state.store.task_stats(&actor.org_id, &from, &to)?))
}
