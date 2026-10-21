// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Daywerk HTTP server: REST API for the web UI and the native apps.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Extension;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode, Uri, header::AUTHORIZATION};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;

use dw_core::{Account, AccountKind, Action, Millis, Outcome, Role};
use dw_storage::{
    Balance, DayView, EPOCH_DATE, NewUser, Org, Stats, StorageError, Store, TeamRow, User,
};

// ---------------------------------------------------------------- state

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    web_root: String,
    index_html: Option<Arc<String>>,
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
    #[serde(default)]
    daily_target_minutes: Option<i64>,
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
    daily_target_minutes: Option<i64>,
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
    daily_target_minutes: i64,
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
    daily_target_minutes: Option<i64>,
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
    let accounts = state.store.accounts(&user.org_id, false)?;

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

    let segments = snap.segments;
    let day = snap.day.map(|d| DayView {
        day: d,
        segments: segments.clone(),
        totals,
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
        target_minutes: user.target_minutes(&org),
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
    Ok(Json(MeResponse {
        accounts: state.store.accounts(&user.org_id, false)?,
        balance_month: state.store.balance(&user.id, &from, &to, now)?,
        balance_total: state.store.balance(&user.id, EPOCH_DATE, &to, now)?,
        target_minutes: user.target_minutes(&org),
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

    let (_, outcome) = state.store.punch(&user, &action, now_ms(), &date, source)?;

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
        org_id,
        email: req.email,
        name: req.name,
        password_hash: hash_password(&req.password)?,
        role: req.role,
        locale: req.locale.unwrap_or_else(|| "de".into()),
        daily_target_minutes: req.daily_target_minutes.filter(|v| *v > 0),
    })?;
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
    let target = match req.daily_target_minutes {
        Some(0) => None,
        Some(value) => Some(value),
        None => current.daily_target_minutes,
    };
    let active = req.active.unwrap_or(current.active);

    let user = state
        .store
        .update_user(&id, &name, role, &locale, target, active)?;
    if let Some(password) = req.password {
        if password.len() < 6 {
            return Err(ApiError::BadRequest("password too short".into()));
        }
        state.store.set_password(&id, &hash_password(&password)?)?;
    }
    Ok(Json(user))
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ProfileRequest>,
) -> Result<Json<User>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let name = req.name.unwrap_or(user.name);
    let locale = req.locale.unwrap_or(user.locale);
    let updated = state.store.update_user(
        &user.id,
        &name,
        user.role,
        &locale,
        user.daily_target_minutes,
        user.active,
    )?;
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
) -> Result<Json<Vec<Account>>, ApiError> {
    let user = require_auth(&state, &headers)?;
    let include_archived = user.role >= Role::Office;
    Ok(Json(state.store.accounts(&user.org_id, include_archived)?))
}

async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<NewAccountRequest>,
) -> Result<Json<Account>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;
    Ok(Json(state.store.create_account(
        &actor.org_id,
        &req.name,
        req.kind,
        req.icon.as_deref(),
        req.sort_order,
        req.is_primary,
    )?))
}

async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateAccountRequest>,
) -> Result<Json<Account>, ApiError> {
    let actor = require_auth(&state, &headers)?;
    require_role(&actor, Role::Admin)?;

    let current = state.store.account(&id)?.ok_or(ApiError::NotFound)?;
    if current.org_id != actor.org_id {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(state.store.update_account(
        &id,
        &req.name.unwrap_or(current.name),
        req.icon.or(current.icon).as_deref(),
        req.sort_order.unwrap_or(current.sort_order),
        req.archived.unwrap_or(current.archived),
        req.is_primary.unwrap_or(current.is_primary),
    )?))
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
        req.daily_target_minutes.unwrap_or(480),
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
        req.daily_target_minutes,
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
        req.daily_target_minutes,
        req.logo_path.as_deref(),
    )?))
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

    match &state.index_html {
        Some(html) => (
            [(CONTENT_TYPE, "text/html; charset=utf-8")],
            html.as_ref().as_bytes().to_vec(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// ---------------------------------------------------------------- bootstrap

fn bootstrap(store: &Store) -> Result<(), String> {
    if store.orgs().map_err(|e| e.to_string())?.is_empty() {
        let org = store
            .create_org("Daywerk", "default", "Europe/Berlin", 480)
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
                daily_target_minutes: None,
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

    let index_html = std::fs::read_to_string(Path::new(&web_root).join("index.html"))
        .ok()
        .map(Arc::new);
    let state = AppState {
        store,
        web_root: web_root.clone(),
        index_html,
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
        .route("/api/stats", get(stats))
        .route("/api/team", get(team))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{id}", patch(update_user))
        .route("/api/profile", put(update_profile))
        .route("/api/accounts", get(list_accounts).post(create_account))
        .route("/api/accounts/{id}", patch(update_account))
        .route("/api/org", get(get_org).put(update_org))
        .route("/api/orgs", get(list_orgs).post(create_org))
        .route("/api/orgs/{id}", patch(update_org_by_id))
        .route("/api/adjustments", post(add_adjustment))
        .route("/api/export", get(export_backup))
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
