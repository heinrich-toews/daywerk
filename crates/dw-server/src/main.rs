// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Daywerk HTTP server: REST API for the web UI and the native apps.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Extension;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode, Uri, header::AUTHORIZATION};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;

use dw_core::{Account, AccountKind, Action, Annotation, Millis, Outcome, Role};
use dw_storage::{
    Balance, BookingQuery, BookingRow, DayView, EPOCH_DATE, NewUser, Org, Stats, StorageError,
    Store, TeamRow, User, WorkTimeModel,
};

// ---------------------------------------------------------------- state

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    web_root: String,
}

// ---------------------------------------------------------------- errors

enum ApiError {
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound => ApiError::NotFound,
            StorageError::Conflict(m) => ApiError::Conflict(m),
            StorageError::Import(m) => ApiError::BadRequest(m),
            StorageError::Sql(err) => {
                tracing::error!(%err, "database error");
                ApiError::Internal("database error".into())
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

fn now_ms() -> Millis {
    Utc::now().timestamp_millis()
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn new_token() -> String {
    let mut buf = [0u8; 32];
    if getrandom::getrandom(&mut buf).is_err() {
        // Extremely unlikely; fall back to a timestamp so we never panic on login.
        return sha256_hex(&format!("fallback-{}", now_ms()));
    }
    hex::encode(buf)
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
    let salt = SaltString::generate(&mut OsRng);
    argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(format!("hash: {e}")))
}

fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash)
        .map(|parsed| {
            argon2::Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn local_date(now: Millis, tz_name: &str) -> String {
    let tz: Tz = tz_name.parse().unwrap_or(chrono_tz::Europe::Berlin);
    match Utc.timestamp_millis_opt(now).single() {
        Some(dt) => dt.with_timezone(&tz).format("%Y-%m-%d").to_string(),
        None => Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string(),
    }
}

fn month_range(date: &str) -> (String, String) {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive());
    let start = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d);
    let (ny, nm) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    let end = NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|n| n.pred_opt())
        .unwrap_or(d);
    (
        start.format("%Y-%m-%d").to_string(),
        end.format("%Y-%m-%d").to_string(),
    )
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    let token = bearer(headers).ok_or(ApiError::Unauthorized)?;
    let hash = sha256_hex(token);
    state
        .store
        .session_user(&hash, now_ms())?
        .ok_or(ApiError::Unauthorized)
}

fn require_role(user: &User, minimum: Role) -> Result<(), ApiError> {
    if user.role >= minimum {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn can_view_user(actor: &User, target: &User) -> bool {
    match actor.role {
        Role::Admin => true,
        Role::Office => actor.org_id == target.org_id,
        Role::Employee => actor.id == target.id,
    }
}

// ---------------------------------------------------------------- DTOs

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    user: User,
    org: Org,
    accounts: Vec<Account>,
}

#[derive(Serialize)]
struct MeResponse {
    user: User,
    org: Org,
    accounts: Vec<Account>,
    balance_month: Balance,
    balance_total: Balance,
    target_minutes: i64,
    work_time_model: Option<WorkTimeModel>,
}

#[derive(Serialize)]
struct ActiveInfo {
    account_id: String,
    account_name: String,
    kind: AccountKind,
    started_at: Millis,
}

#[derive(Serialize)]
struct TodayResponse {
    now: Millis,
    date: String,
    user: User,
    org: Org,
    accounts: Vec<Account>,
    day: Option<DayView>,
    active: Option<ActiveInfo>,
    balance_month: Balance,
    balance_total: Balance,
    target_minutes: i64,
    work_time_model: Option<WorkTimeModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<Outcome>,
}

#[derive(Deserialize)]
struct PunchRequest {
    action: String,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    material: Option<String>,
}

const MAX_NOTE_LEN: usize = 2000;

fn clean_text(value: Option<String>) -> Result<String, ApiError> {
    let text = value.unwrap_or_default();
    let trimmed = text.trim();
    if trimmed.chars().count() > MAX_NOTE_LEN {
        return Err(ApiError::BadRequest(format!(
            "text is longer than {MAX_NOTE_LEN} characters"
        )));
    }
    Ok(trimmed.to_string())
}

#[derive(Deserialize)]
struct UpdateSegmentRequest {
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    material: Option<String>,
}

#[derive(Deserialize, Default)]
struct AccountParams {
    #[serde(default)]
    org_id: Option<String>,
}

#[derive(Deserialize)]
struct NewWorkTimeModelRequest {
    valid_from: String,
    weekly_hours: f64,
    workdays_per_week: i64,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct UpdateWorkTimeModelRequest {
    weekly_hours: f64,
    workdays_per_week: i64,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize, Default)]
struct BookingParams {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    org_id: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,
}

#[derive(Deserialize, Default)]
struct RangeQuery {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct TeamQuery {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    org_id: Option<String>,
}

#[derive(Deserialize)]
struct NewUserRequest {
    #[serde(default)]
    org_id: Option<String>,
    email: String,
    name: String,
    password: String,
    role: Role,
    #[serde(default)]
    locale: Option<String>,
    /// Optional first contract; falls back to the organisation default.
    #[serde(default)]
    weekly_hours: Option<f64>,
    #[serde(default)]
    workdays_per_week: Option<i64>,
}

#[derive(Deserialize)]
struct UpdateUserRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    role: Option<Role>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
struct ProfileRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
struct NewAccountRequest {
    name: String,
    kind: AccountKind,
    #[serde(default)]
    org_id: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    sort_order: i32,
    #[serde(default)]
    is_primary: bool,
}

#[derive(Deserialize)]
struct UpdateAccountRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    sort_order: Option<i32>,
    #[serde(default)]
    archived: Option<bool>,
    #[serde(default)]
    is_primary: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateOrgRequest {
    name: String,
    color_primary: String,
    color_accent: String,
    timezone: String,
    default_weekly_hours: f64,
    default_workdays_per_week: i64,
    #[serde(default)]
    logo_path: Option<String>,
}

#[derive(Deserialize)]
struct NewOrgRequest {
    name: String,
    slug: String,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    default_weekly_hours: Option<f64>,
    #[serde(default)]
    default_workdays_per_week: Option<i64>,
}

#[derive(Deserialize)]
struct AdjustmentRequest {
    user_id: String,
    date: String,
    minutes: i64,
    #[serde(default)]
    reason: String,
}

// ---------------------------------------------------------------- payload helpers

fn today_payload(
    state: &AppState,
    user: &User,
    outcome: Option<Outcome>,
) -> Result<TodayResponse, ApiError> {
    let now = now_ms();
    let org = state.store.org(&user.org_id)?.ok_or(ApiError::NotFound)?;
    let date = local_date(now, &org.timezone);
    let snap = state.store.snapshot(&user.id, &user.org_id, &date)?;

    // Active accounts, plus the one a running timer is still on: archiving an
    // account mid-shift must not make the running segment disappear. It drops
    // out as soon as the segment is closed.
    let running_account = snap
        .segments
        .iter()
        .find(|s| s.ended_at.is_none())
        .map(|s| s.account_id.clone());
    let accounts: Vec<Account> = snap
        .accounts
        .iter()
        .filter(|a| !a.archived || running_account.as_deref() == Some(a.id.as_str()))
        .cloned()
        .collect();

    let totals = dw_core::totals(&snap.segments, &snap.accounts, now);
    let active = snap
        .segments
        .iter()
        .find(|s| s.ended_at.is_none())
        .map(|seg| {
            let account = snap.accounts.iter().find(|a| a.id == seg.account_id);
            ActiveInfo {
                account_id: seg.account_id.clone(),
                account_name: account.map(|a| a.name.clone()).unwrap_or_default(),
                kind: account.map(|a| a.kind).unwrap_or(AccountKind::Work),
                started_at: seg.started_at,
            }
        });

    let day_date = snap
        .day
        .as_ref()
        .map(|d| d.date.clone())
        .unwrap_or_else(|| date.clone());
    let work_time_model = state.store.work_time_model_at(&user.id, &day_date)?;
    let target_minutes = work_time_model
        .as_ref()
        .map(|m| m.daily_target_minutes())
        .unwrap_or_else(|| org.default_daily_minutes());

    let segments = snap.segments;
    let day = snap.day.map(|d| DayView {
        day: d,
        segments: segments.clone(),
        totals,
        target_minutes,
    });

    let (from, to) = month_range(&date);
    let balance_month = state.store.balance(&user.id, &from, &to, now)?;
    let balance_total = state.store.balance(&user.id, EPOCH_DATE, &to, now)?;

    Ok(TodayResponse {
        now,
        date,
        user: user.clone(),
        org: org.clone(),
        accounts,
        day,
        active,
        balance_month,
        balance_total,
        target_minutes,
        work_time_model,
        outcome,
    })
}

fn range_for(
    state: &AppState,
    user: &User,
    from: Option<&str>,
    to: Option<&str>,
) -> (String, String) {
    let org = state.store.org(&user.org_id).ok().flatten();
    let tz = org
        .map(|o| o.timezone)
        .unwrap_or_else(|| "Europe/Berlin".into());
    let today = local_date(now_ms(), &tz);
    let (month_from, month_to) = month_range(&today);
    (
        from.map(str::to_string).unwrap_or(month_from),
        to.map(str::to_string).unwrap_or(month_to),
    )
}

fn resolve_target_user(
    state: &AppState,
    actor: &User,
    requested: Option<&str>,
) -> Result<User, ApiError> {
    match requested {
        None => Ok(actor.clone()),
        Some(id) if id == actor.id => Ok(actor.clone()),
        Some(id) => {
            let target = state.store.user(id)?.ok_or(ApiError::NotFound)?;
            if !can_view_user(actor, &target) {
                return Err(ApiError::Forbidden);
            }
            Ok(target)
        }
    }
}

// ---------------------------------------------------------------- handlers

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let (user, hash) = state
        .store
        .user_by_email(&req.email)?
        .ok_or(ApiError::Unauthorized)?;
    if !user.active || !verify_password(&req.password, &hash) {
        return Err(ApiError::Unauthorized);
    }
    let token = new_token();
    let now = now_ms();
    state
        .store
        .create_session(&sha256_hex(&token), &user.id, now)?;

    let org = state.store.org(&user.org_id)?.ok_or(ApiError::NotFound)?;
    let accounts = state.store.accounts(&user.org_id, false)?;
    Ok(Json(LoginResponse {
        token,
        user,
        org,
        accounts,
    }))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    if let Some(token) = bearer(&headers) {
        state.store.delete_session(&sha256_hex(token))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let org = state.store.org(&user.org_id)?.ok_or(ApiError::NotFound)?;
    let now = now_ms();
    let date = local_date(now, &org.timezone);
    let (from, to) = month_range(&date);
    let work_time_model = state.store.work_time_model_at(&user.id, &date)?;
    let target_minutes = work_time_model
        .as_ref()
        .map(|m| m.daily_target_minutes())
        .unwrap_or_else(|| org.default_daily_minutes());
    Ok(Json(MeResponse {
        // Includes archived accounts: the history shows their names.
        accounts: state.store.accounts(&user.org_id, true)?,
        balance_month: state.store.balance(&user.id, &from, &to, now)?,
        balance_total: state.store.balance(&user.id, EPOCH_DATE, &to, now)?,
        target_minutes,
        work_time_model,
        user,
        org,
    }))
}

async fn today(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TodayResponse>, ApiError> {
    let user = require_auth(&state, &headers)?;
    Ok(Json(today_payload(&state, &user, None)?))
}

async fn punch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PunchRequest>,
) -> Result<Json<TodayResponse>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let action = match req.action.as_str() {
        "start" => Action::Start {
            account_id: req
                .account_id
                .clone()
                .ok_or_else(|| ApiError::BadRequest("account_id is required for start".into()))?,
        },
        "pause" => Action::Pause,
        "stop" => Action::Stop,
        other => return Err(ApiError::BadRequest(format!("unknown action: {other}"))),
    };

    let org = state.store.org(&user.org_id)?.ok_or(ApiError::NotFound)?;
    let date = local_date(now_ms(), &org.timezone);
    let source = req.source.as_deref().unwrap_or("web");
    let annotation = Annotation {
        note: clean_text(req.note)?,
        material: clean_text(req.material)?,
    };

    let (_, outcome) =
        state
            .store
            .punch_with(&user, &action, now_ms(), &date, source, &annotation)?;

    if matches!(
        outcome,
        Outcome::Stopped { .. } | Outcome::StoppedFromBreak { .. }
    ) {
        let _ = state.store.audit(
            &user.org_id,
            &user.id,
            "day_stopped",
            "workday",
            &date,
            "{}",
            now_ms(),
        );
    }

    Ok(Json(today_payload(&state, &user, Some(outcome))?))
}

async fn days(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RangeQuery>,
) -> Result<Json<Vec<DayView>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let target = resolve_target_user(&state, &actor, q.user_id.as_deref())?;
    let (from, to) = range_for(&state, &target, q.from.as_deref(), q.to.as_deref());
    Ok(Json(state.store.days_between(
        &target.id,
        &target.org_id,
        &from,
        &to,
        now_ms(),
    )?))
}

async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RangeQuery>,
) -> Result<Json<Stats>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let target = resolve_target_user(&state, &actor, q.user_id.as_deref())?;
    let (from, to) = range_for(&state, &target, q.from.as_deref(), q.to.as_deref());
    Ok(Json(state.store.stats(&target.id, &from, &to, now_ms())?))
}

/// Balances of every worker. Office sees its own organisation, admin sees all.
async fn team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<TeamQuery>,
) -> Result<Json<Vec<TeamRow>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    let org_scope = match actor.role {
        Role::Admin => q.org_id.as_deref(),
        _ => Some(actor.org_id.as_str()),
    };

    let (from, to) = range_for(&state, &actor, q.from.as_deref(), q.to.as_deref());
    Ok(Json(state.store.team(org_scope, &from, &to, now_ms())?))
}

async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<TeamQuery>,
) -> Result<Json<Vec<User>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    match actor.role {
        Role::Admin => match q.org_id.as_deref() {
            Some(id) => Ok(Json(state.store.users_of_org(id)?)),
            None => Ok(Json(state.store.all_users()?)),
        },
        _ => Ok(Json(state.store.users_of_org(&actor.org_id)?)),
    }
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewUserRequest>,
) -> Result<Json<User>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    let org_id = req.org_id.unwrap_or_else(|| actor.org_id.clone());
    if state.store.org(&org_id)?.is_none() {
        return Err(ApiError::BadRequest("unknown organisation".into()));
    }
    if req.password.len() < 6 {
        return Err(ApiError::BadRequest(
            "password must have at least 6 characters".into(),
        ));
    }

    let user = state.store.create_user(&NewUser {
        org_id: org_id.clone(),
        email: req.email,
        name: req.name,
        password_hash: hash_password(&req.password)?,
        role: req.role,
        locale: req.locale.unwrap_or_else(|| "de".into()),
    })?;

    // Give the new employee a contract from day one, so the balance never has
    // to guess. Falls back to the organisation default.
    let org = state.store.org(&org_id)?.ok_or(ApiError::NotFound)?;
    let weekly_hours = req.weekly_hours.unwrap_or(org.default_weekly_hours);
    let workdays = req
        .workdays_per_week
        .unwrap_or(org.default_workdays_per_week);
    state.store.add_work_time_model(
        &user.id,
        "1970-01-01",
        weekly_hours,
        workdays,
        "Initial",
        &actor.id,
        now_ms(),
    )?;

    Ok(Json(user))
}

async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<User>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    let current = state.store.user(&id)?.ok_or(ApiError::NotFound)?;
    let name = req.name.unwrap_or(current.name);
    let role = req.role.unwrap_or(current.role);
    let locale = req.locale.unwrap_or(current.locale);

    // Nobody may lock themselves out, and the instance must keep one active
    // administrator.
    let loses_admin = current.role.is_admin() && !role.is_admin();
    let loses_active = current.active && req.active == Some(false);
    if id == actor.id && (loses_admin || loses_active) {
        return Err(ApiError::Conflict(
            "you cannot deactivate or demote your own account".into(),
        ));
    }
    // Only the last *administrator* is protected; ordinary employees can be
    // deactivated freely.
    if current.role.is_admin()
        && (loses_admin || loses_active)
        && state.store.count_active_admins()? <= 1
    {
        return Err(ApiError::Conflict(
            "the last administrator must stay active and keep the admin role".into(),
        ));
    }

    let active = req.active.unwrap_or(current.active);

    let user = state.store.update_user(&id, &name, role, &locale, active)?;
    if let Some(password) = req.password {
        if password.len() < 6 {
            return Err(ApiError::BadRequest("password too short".into()));
        }
        state.store.set_password(&id, &hash_password(&password)?)?;
    }
    Ok(Json(user))
}

async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    if id == actor.id {
        return Err(ApiError::Conflict(
            "you cannot delete your own account".into(),
        ));
    }
    let user = state.store.user(&id)?.ok_or(ApiError::NotFound)?;
    state.store.delete_user(&id)?;
    let _ = state.store.audit(
        &user.org_id,
        &actor.id,
        "user_deleted",
        "user",
        &id,
        "{}",
        now_ms(),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ProfileRequest>,
) -> Result<Json<User>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let name = req.name.unwrap_or(user.name);
    let locale = req.locale.unwrap_or(user.locale);
    let updated = state
        .store
        .update_user(&user.id, &name, user.role, &locale, user.active)?;
    if let Some(password) = req.password {
        if password.len() < 6 {
            return Err(ApiError::BadRequest("password too short".into()));
        }
        state
            .store
            .set_password(&user.id, &hash_password(&password)?)?;
    }
    Ok(Json(updated))
}

async fn list_accounts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AccountParams>,
) -> Result<Json<Vec<Account>>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let include_archived = user.role >= Role::Office;
    let org_id = match user.role {
        Role::Admin => params.org_id.unwrap_or_else(|| user.org_id.clone()),
        _ => user.org_id.clone(),
    };
    Ok(Json(state.store.accounts(&org_id, include_archived)?))
}

async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewAccountRequest>,
) -> Result<Json<Account>, ApiError> {
    let actor = require_auth(&state, &headers)?;

    // Employees may add work accounts for their own organisation. Only an
    // administrator may touch the break account or promote a primary account.
    let (kind, sort_order, is_primary) = if actor.role.is_admin() {
        (req.kind, req.sort_order, req.is_primary)
    } else {
        if req.kind != AccountKind::Work {
            return Err(ApiError::Forbidden);
        }
        (AccountKind::Work, 100, false)
    };

    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(ApiError::BadRequest(
            "name must be between 1 and 60 characters".into(),
        ));
    }

    let org_id = if actor.role.is_admin() {
        req.org_id.unwrap_or_else(|| actor.org_id.clone())
    } else {
        actor.org_id.clone()
    };

    let account = state.store.create_account(
        &org_id,
        name,
        kind,
        req.icon.as_deref(),
        sort_order,
        is_primary,
    )?;
    let _ = state.store.audit(
        &org_id,
        &actor.id,
        "account_created",
        "account",
        &account.id,
        "{}",
        now_ms(),
    );
    Ok(Json(account))
}

async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateAccountRequest>,
) -> Result<Json<Account>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    let current = state.store.account(&id)?.ok_or(ApiError::NotFound)?;
    if actor.role != Role::Admin && current.org_id != actor.org_id {
        return Err(ApiError::Forbidden);
    }

    // Only an administrator may move the primary flag.
    if req.is_primary.is_some() && !actor.role.is_admin() {
        return Err(ApiError::Forbidden);
    }

    let archived = req.archived.unwrap_or(current.archived);
    if archived && !current.archived {
        if current.is_primary {
            return Err(ApiError::Conflict(
                "the primary time account cannot be archived".into(),
            ));
        }
        if current.kind == AccountKind::Break {
            return Err(ApiError::Conflict(
                "the break account cannot be archived".into(),
            ));
        }
    }

    let account = state.store.update_account(
        &id,
        &req.name.unwrap_or(current.name),
        req.icon.or(current.icon).as_deref(),
        req.sort_order.unwrap_or(current.sort_order),
        archived,
        req.is_primary.unwrap_or(current.is_primary),
    )?;

    if account.archived != current.archived {
        let _ = state.store.audit(
            &account.org_id,
            &actor.id,
            if account.archived {
                "account_archived"
            } else {
                "account_restored"
            },
            "account",
            &id,
            "{}",
            now_ms(),
        );
    }

    Ok(Json(account))
}

async fn get_org(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Org>, ApiError> {
    let user = require_auth(&state, &headers)?;
    Ok(Json(
        state.store.org(&user.org_id)?.ok_or(ApiError::NotFound)?,
    ))
}

async fn list_orgs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Org>>, ApiError> {
    let user = require_auth(&state, &headers)?;
    require_role(&user, Role::Admin)?;
    Ok(Json(state.store.orgs()?))
}

async fn create_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewOrgRequest>,
) -> Result<Json<Org>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    let org = state.store.create_org(
        &req.name,
        &req.slug,
        req.timezone.as_deref().unwrap_or("Europe/Berlin"),
        req.default_weekly_hours.unwrap_or(40.0),
        req.default_workdays_per_week.unwrap_or(5),
    )?;
    state.store.ensure_default_accounts(&org.id)?;
    Ok(Json(org))
}

async fn update_org_by_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<Json<Org>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    Ok(Json(state.store.update_org(
        &id,
        &req.name,
        &req.color_primary,
        &req.color_accent,
        &req.timezone,
        req.default_weekly_hours,
        req.default_workdays_per_week,
        req.logo_path.as_deref(),
    )?))
}

async fn update_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<Json<Org>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    Ok(Json(state.store.update_org(
        &actor.org_id,
        &req.name,
        &req.color_primary,
        &req.color_accent,
        &req.timezone,
        req.default_weekly_hours,
        req.default_workdays_per_week,
        req.logo_path.as_deref(),
    )?))
}

async fn update_segment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateSegmentRequest>,
) -> Result<Json<dw_core::Segment>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let segment = state.store.segment(&id)?.ok_or(ApiError::NotFound)?;
    let day = state
        .store
        .workday_by_id(&segment.workday_id)?
        .ok_or(ApiError::NotFound)?;
    let owner = state
        .store
        .user(&segment.user_id)?
        .ok_or(ApiError::NotFound)?;

    let allowed = match actor.role {
        Role::Admin => true,
        Role::Office => owner.org_id == actor.org_id,
        Role::Employee => {
            owner.id == actor.id
                && state
                    .store
                    .org(&owner.org_id)?
                    .map(|org| local_date(now_ms(), &org.timezone) == day.date)
                    .unwrap_or(false)
        }
    };
    if !allowed {
        return Err(ApiError::Forbidden);
    }

    let note = match req.note {
        Some(value) => clean_text(Some(value))?,
        None => segment.note.clone(),
    };
    let material = match req.material {
        Some(value) => clean_text(Some(value))?,
        None => segment.material.clone(),
    };

    let updated = state
        .store
        .update_segment_note(&id, &note, &material, &actor.id, now_ms())?;
    let _ = state.store.audit(
        &owner.org_id,
        &actor.id,
        "segment_note_updated",
        "segment",
        &id,
        "{}",
        now_ms(),
    );
    Ok(Json(updated))
}

/// Full text search over notes and materials. Office is scoped to its own
/// organisation, admin may search everything.
async fn bookings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BookingParams>,
) -> Result<Json<Vec<BookingRow>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    if let Some(user_id) = params.user_id.as_deref() {
        let target = state.store.user(user_id)?.ok_or(ApiError::NotFound)?;
        if !can_view_user(&actor, &target) {
            return Err(ApiError::Forbidden);
        }
    }

    let org_scope = match actor.role {
        Role::Admin => params.org_id.as_deref(),
        _ => Some(actor.org_id.as_str()),
    };
    let (from, to) = range_for(&state, &actor, params.from.as_deref(), params.to.as_deref());
    let terms: Vec<String> = params
        .q
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect();

    Ok(Json(state.store.search_bookings(&BookingQuery {
        org_id: org_scope,
        user_id: params.user_id.as_deref(),
        account_id: params.account_id.as_deref(),
        from: &from,
        to: &to,
        terms: &terms,
        limit: params.limit.unwrap_or(50).clamp(1, 200),
        offset: params.offset.unwrap_or(0).max(0),
    })?))
}

async fn list_work_time_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<WorkTimeModel>>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    let target = state.store.user(&id)?.ok_or(ApiError::NotFound)?;
    if !can_view_user(&actor, &target) {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(state.store.work_time_models(&id)?))
}

async fn create_work_time_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<NewWorkTimeModelRequest>,
) -> Result<Json<WorkTimeModel>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    let target = state.store.user(&id)?.ok_or(ApiError::NotFound)?;
    NaiveDate::parse_from_str(&req.valid_from, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("valid_from must be YYYY-MM-DD".into()))?;

    let model = state.store.add_work_time_model(
        &target.id,
        &req.valid_from,
        req.weekly_hours,
        req.workdays_per_week,
        &req.note,
        &actor.id,
        now_ms(),
    )?;
    let _ = state.store.audit(
        &target.org_id,
        &actor.id,
        "work_time_model_created",
        "work_time_model",
        &model.id,
        &format!(
            "{{\"valid_from\":\"{}\",\"weekly_hours\":{},\"workdays_per_week\":{}}}",
            model.valid_from, model.weekly_hours, model.workdays_per_week
        ),
        now_ms(),
    );
    Ok(Json(model))
}

async fn update_work_time_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateWorkTimeModelRequest>,
) -> Result<Json<WorkTimeModel>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    let model = state.store.update_work_time_model(
        &id,
        req.weekly_hours,
        req.workdays_per_week,
        &req.note,
    )?;
    let _ = state.store.audit(
        &actor.org_id,
        &actor.id,
        "work_time_model_updated",
        "work_time_model",
        &id,
        "{}",
        now_ms(),
    );
    Ok(Json(model))
}

async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    let account = state.store.account(&id)?.ok_or(ApiError::NotFound)?;
    state.store.delete_account(&id)?;
    let _ = state.store.audit(
        &account.org_id,
        &actor.id,
        "account_deleted",
        "account",
        &id,
        "{}",
        now_ms(),
    );
    Ok(StatusCode::NO_CONTENT)
}

fn fmt_hm(ms: Millis, tz_name: &str) -> String {
    let tz: Tz = tz_name.parse().unwrap_or(chrono_tz::Europe::Berlin);
    match Utc.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.with_timezone(&tz).format("%H:%M").to_string(),
        None => String::new(),
    }
}

fn fmt_stamp(ms: Millis, tz_name: &str) -> String {
    let tz: Tz = tz_name.parse().unwrap_or(chrono_tz::Europe::Berlin);
    match Utc.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.with_timezone(&tz).format("%d.%m.%Y %H:%M").to_string(),
        None => String::new(),
    }
}

/// `2026-10-21` -> `21.10.2026`
fn de_date(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3 {
        format!("{}.{}.{}", parts[2], parts[1], parts[0])
    } else {
        iso.to_string()
    }
}

fn build_report(
    state: &AppState,
    actor: &User,
    org_id: Option<&str>,
    from: &str,
    to: &str,
) -> Result<dw_export::Report, ApiError> {
    let now = now_ms();
    let org_scope = match actor.role {
        Role::Admin => org_id,
        _ => Some(actor.org_id.as_str()),
    };

    let own_org = state.store.org(&actor.org_id)?.ok_or(ApiError::NotFound)?;
    let (title, timezone) = match org_scope {
        Some(id) => match state.store.org(id)? {
            Some(org) => (org.name, org.timezone),
            None => (own_org.name, own_org.timezone),
        },
        None => ("Alle Organisationen".to_string(), own_org.timezone),
    };

    let rows = state.store.team(org_scope, from, to, now)?;
    let mut workers = Vec::with_capacity(rows.len());

    for row in rows {
        let org = state
            .store
            .org(&row.user.org_id)?
            .ok_or(ApiError::NotFound)?;
        let accounts = state.store.accounts(&row.user.org_id, true)?;
        let days = state
            .store
            .days_between(&row.user.id, &row.user.org_id, from, to, now)?;

        let mut day_reports = Vec::with_capacity(days.len());
        for day in days {
            let mut segments = Vec::with_capacity(day.segments.len());
            for segment in &day.segments {
                let account = accounts.iter().find(|a| a.id == segment.account_id);
                segments.push(dw_export::SegmentReport {
                    account: account.map(|a| a.name.clone()).unwrap_or_default(),
                    start: fmt_hm(segment.started_at, &org.timezone),
                    end: segment
                        .ended_at
                        .map(|end| fmt_hm(end, &org.timezone))
                        .unwrap_or_default(),
                    duration_ms: segment.ended_at.unwrap_or(now) - segment.started_at,
                    note: segment.note.clone(),
                    material: segment.material.clone(),
                });
            }
            let first_start = segments
                .first()
                .map(|s| s.start.clone())
                .unwrap_or_default();
            let last_end = segments.last().map(|s| s.end.clone()).unwrap_or_default();

            day_reports.push(dw_export::DayReport {
                date: de_date(&day.day.date),
                first_start,
                last_end,
                work_ms: day.totals.work_ms,
                break_ms: day.totals.break_ms,
                target_ms: day.target_minutes * 60_000,
                balance_ms: day.totals.work_ms - day.target_minutes * 60_000,
                segments,
            });
        }
        day_reports.reverse(); // chronological in the report

        workers.push(dw_export::WorkerReport {
            name: row.user.name.clone(),
            email: row.user.email.clone(),
            org_name: row.org_name.clone(),
            days_count: row.period.days,
            work_ms: row.period.worked_ms,
            break_ms: row.period.break_ms,
            target_ms: row.period.target_ms,
            balance_ms: row.period.balance_ms,
            total_balance_ms: row.total.balance_ms,
            days: day_reports,
        });
    }

    Ok(dw_export::Report {
        org_name: title,
        from: de_date(from),
        to: de_date(to),
        generated_at: fmt_stamp(now, &timezone),
        workers,
    })
}

fn download_response(bytes: Vec<u8>, mime: &str, filename: &str) -> Result<Response, ApiError> {
    Ok((
        [
            (CONTENT_TYPE, mime.to_string()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn export_xlsx(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<TeamQuery>,
) -> Result<Response, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;
    let (from, to) = range_for(&state, &actor, q.from.as_deref(), q.to.as_deref());
    let report = build_report(&state, &actor, q.org_id.as_deref(), &from, &to)?;
    let bytes =
        dw_export::render_xlsx(&report).map_err(|e| ApiError::Internal(format!("xlsx: {e}")))?;
    download_response(
        bytes,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        &format!("daywerk-{from}_{to}.xlsx"),
    )
}

async fn export_pdf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<TeamQuery>,
) -> Result<Response, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;
    let (from, to) = range_for(&state, &actor, q.from.as_deref(), q.to.as_deref());
    let report = build_report(&state, &actor, q.org_id.as_deref(), &from, &to)?;
    let bytes =
        dw_export::render_pdf(&report).map_err(|e| ApiError::Internal(format!("pdf: {e}")))?;
    download_response(
        bytes,
        "application/pdf",
        &format!("daywerk-{from}_{to}.pdf"),
    )
}

async fn add_adjustment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdjustmentRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Office)?;

    let target = state.store.user(&req.user_id)?.ok_or(ApiError::NotFound)?;
    if !can_view_user(&actor, &target) {
        return Err(ApiError::Forbidden);
    }
    state.store.add_adjustment(
        &target.id,
        &req.date,
        req.minutes,
        &req.reason,
        &actor.id,
        now_ms(),
    )?;
    Ok(StatusCode::NO_CONTENT)
}

async fn export_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    Ok(Json(state.store.export_json()?))
}

async fn import_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    Ok(Json(state.store.import_json(&body)?))
}

// ---------------------------------------------------------------- static files

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serve a file from the web root, or fall back to `index.html` so client side
/// routes work after a page reload.
async fn static_or_index(Extension(state): Extension<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if !path.is_empty() && !path.split('/').any(|part| part == ".." || part == ".") {
        let full = Path::new(&state.web_root).join(path);
        if full.is_file() {
            if let Ok(bytes) = std::fs::read(&full) {
                let mime = mime_for(&full);
                return ([(CONTENT_TYPE, mime)], bytes).into_response();
            }
        }
    }

    // Read the shell on every request: Vite emits a new hashed bundle name on
    // each build, so a cached index.html would point at files that no longer
    // exist after a rebuild.
    match std::fs::read(Path::new(&state.web_root).join("index.html")) {
        Ok(html) => ([(CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// ---------------------------------------------------------------- bootstrap

fn bootstrap(store: &Store) -> Result<(), String> {
    if store.orgs().map_err(|e| e.to_string())?.is_empty() {
        let org = store
            .create_org("Daywerk", "default", "Europe/Berlin", 40.0, 5)
            .map_err(|e| e.to_string())?;
        store
            .ensure_default_accounts(&org.id)
            .map_err(|e| e.to_string())?;

        let email = std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@daywerk.local".into());
        let password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "daywerk".into());
        let hash = hash_password(&password).map_err(|_| "cannot hash password".to_string())?;
        let user = store
            .create_user(&NewUser {
                org_id: org.id,
                email: email.clone(),
                name: "Administrator".into(),
                password_hash: hash,
                role: Role::Admin,
                locale: "de".into(),
            })
            .map_err(|e| e.to_string())?;
        tracing::info!(%email, user_id = %user.id, "created first administrator");
    }
    Ok(())
}

// ---------------------------------------------------------------- main

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dw_server=info,tower_http=warn".into()),
        )
        .init();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "data/daywerk.db".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9095);
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0".into());
    let web_root = std::env::var("WEB_ROOT").unwrap_or_else(|_| "apps/web-ui/dist".into());

    let store = Arc::new(Store::open(&db_path).expect("open database"));
    bootstrap(&store).expect("bootstrap");
    tracing::info!(db = %db_path, "database ready");

    let state = AppState {
        store,
        web_root: web_root.clone(),
    };
    let extension_state = state.clone();

    let api: Router = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/today", get(today))
        .route("/api/punch", post(punch))
        .route("/api/days", get(days))
        .route("/api/bookings", get(bookings))
        .route("/api/segments/{id}", patch(update_segment))
        .route("/api/stats", get(stats))
        .route("/api/team", get(team))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{id}", patch(update_user).delete(delete_user))
        .route(
            "/api/users/{id}/work-time-models",
            get(list_work_time_models).post(create_work_time_model),
        )
        .route("/api/work-time-models/{id}", patch(update_work_time_model))
        .route("/api/profile", put(update_profile))
        .route("/api/accounts", get(list_accounts).post(create_account))
        .route(
            "/api/accounts/{id}",
            patch(update_account).delete(delete_account),
        )
        .route("/api/org", get(get_org).put(update_org))
        .route("/api/orgs", get(list_orgs).post(create_org))
        .route("/api/orgs/{id}", patch(update_org_by_id))
        .route("/api/adjustments", post(add_adjustment))
        .route("/api/export", get(export_backup))
        .route("/api/export/timesheet.xlsx", get(export_xlsx))
        .route("/api/export/timesheet.pdf", get(export_pdf))
        .route("/api/import", post(import_backup))
        .with_state(state);

    let app = if Path::new(&web_root).is_dir() {
        tracing::info!(root = %web_root, "serving web UI");
        api.fallback(static_or_index)
    } else {
        tracing::warn!(root = %web_root, "no web UI build found; API only");
        api
    }
    .layer(CorsLayer::permissive())
    .layer(Extension(extension_state));

    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .expect("valid BIND/PORT combination");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind port");
    tracing::info!("Daywerk listening on http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
