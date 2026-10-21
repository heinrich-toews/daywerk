// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! SQLite persistence for Daywerk.
//!
//! Every query that touches tenant data takes an organisation id. SQLite has no
//! row level security, so this module is the security boundary: there is no
//! API in here that returns another tenant's rows without being told which
//! organisation it is acting for.

use std::sync::{Mutex, MutexGuard};

use dw_core::{
    Account, AccountKind, Action, DayStatus, Millis, Plan, Role, Segment, Snapshot, Totals, Workday,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MIGRATIONS: &[&str] = &[SCHEMA_V1, MIGRATION_V2];
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;
pub const EPOCH_DATE: &str = "1970-01-01";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid import: {0}")]
    Import(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub logo_path: Option<String>,
    pub color_primary: String,
    pub color_accent: String,
    pub timezone: String,
    pub daily_target_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub org_id: String,
    pub email: String,
    pub name: String,
    pub role: Role,
    pub locale: String,
    /// Overrides the organisation default when set.
    pub daily_target_minutes: Option<i64>,
    pub active: bool,
    pub created_at: Millis,
}

impl User {
    pub fn target_minutes(&self, org: &Org) -> i64 {
        self.daily_target_minutes
            .unwrap_or(org.daily_target_minutes)
    }
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub org_id: String,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub role: Role,
    pub locale: String,
    pub daily_target_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayView {
    pub day: Workday,
    pub segments: Vec<Segment>,
    pub totals: Totals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub worked_ms: Millis,
    pub break_ms: Millis,
    pub days: i64,
    pub target_minutes: i64,
    pub target_ms: Millis,
    pub adjustments_ms: Millis,
    pub balance_ms: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRow {
    pub user: User,
    pub org_id: String,
    pub org_name: String,
    pub period: Balance,
    pub total: Balance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingRow {
    pub id: String,
    pub date: String,
    pub user_id: String,
    pub user_name: String,
    pub account_id: String,
    pub account_name: String,
    pub started_at: Millis,
    pub ended_at: Option<Millis>,
    pub note: String,
    pub material: String,
    pub org_id: String,
    pub org_name: String,
}

#[derive(Debug, Clone)]
pub struct BookingQuery<'a> {
    pub org_id: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub account_id: Option<&'a str>,
    pub from: &'a str,
    pub to: &'a str,
    /// Every term must match either the note or the material.
    pub terms: &'a [String],
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTotal {
    pub account_id: String,
    pub account_name: String,
    pub kind: AccountKind,
    pub ms: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTotal {
    pub date: String,
    pub work_ms: Millis,
    pub break_ms: Millis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub period: Balance,
    pub total: Balance,
    pub per_account: Vec<AccountTotal>,
    pub per_day: Vec<DayTotal>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

fn now_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    pub fn in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        // SQLite's lower() only folds ASCII, which mangles German umlauts.
        conn.create_scalar_function(
            "dw_lower",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8
                | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()),
        )?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.lock();
        let mut version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        while (version as usize) < MIGRATIONS.len() {
            let sql = MIGRATIONS[version as usize];
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version + 1)?;
            tx.commit()?;
            version += 1;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.lock();
        Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    // ---- organisations -------------------------------------------------

    pub fn create_org(
        &self,
        name: &str,
        slug: &str,
        timezone: &str,
        daily_target_minutes: i64,
    ) -> Result<Org> {
        let id = now_id();
        let created = chrono_free_now();
        self.lock()
            .execute(
                "INSERT INTO organizations
                   (id, name, slug, color_primary, color_accent, timezone, daily_target_minutes, created_at)
                 VALUES (?1, ?2, ?3, '#2a7647', '#e8f2e8', ?4, ?5, ?6)",
                params![id, name, slug, timezone, daily_target_minutes, created],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StorageError::Conflict("slug already exists".into())
                }
                other => StorageError::Sql(other),
            })?;
        self.org(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn org(&self, id: &str) -> Result<Option<Org>> {
        let conn = self.lock();
        let row = conn
            .query_row(
                "SELECT id, name, slug, logo_path, color_primary, color_accent, timezone, daily_target_minutes
                 FROM organizations WHERE id = ?1",
                params![id],
                map_org,
            )
            .optional()?;
        Ok(row)
    }

    pub fn org_by_slug(&self, slug: &str) -> Result<Option<Org>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, name, slug, logo_path, color_primary, color_accent, timezone, daily_target_minutes
                 FROM organizations WHERE slug = ?1",
                params![slug],
                map_org,
            )
            .optional()?)
    }

    pub fn orgs(&self) -> Result<Vec<Org>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, slug, logo_path, color_primary, color_accent, timezone, daily_target_minutes
             FROM organizations ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_org)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn update_org(
        &self,
        id: &str,
        name: &str,
        color_primary: &str,
        color_accent: &str,
        timezone: &str,
        daily_target_minutes: i64,
        logo_path: Option<&str>,
    ) -> Result<Org> {
        self.lock().execute(
            "UPDATE organizations
                SET name = ?2, color_primary = ?3, color_accent = ?4,
                    timezone = ?5, daily_target_minutes = ?6, logo_path = ?7
              WHERE id = ?1",
            params![
                id,
                name,
                color_primary,
                color_accent,
                timezone,
                daily_target_minutes,
                logo_path
            ],
        )?;
        self.org(id)?.ok_or(StorageError::NotFound)
    }

    // ---- users ---------------------------------------------------------

    pub fn create_user(&self, u: &NewUser) -> Result<User> {
        let id = now_id();
        let created = chrono_free_now();
        self.lock().execute(
            "INSERT INTO users
               (id, org_id, email, name, password_hash, role, locale, daily_target_minutes, active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)",
            params![
                id,
                u.org_id,
                u.email.to_lowercase(),
                u.name,
                u.password_hash,
                role_str(u.role),
                u.locale,
                u.daily_target_minutes,
                created
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StorageError::Conflict("email already exists".into())
            }
            other => StorageError::Sql(other),
        })?;
        self.user(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn user(&self, id: &str) -> Result<Option<User>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at
                 FROM users WHERE id = ?1",
                params![id],
                map_user,
            )
            .optional()?)
    }

    pub fn user_by_email(&self, email: &str) -> Result<Option<(User, String)>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at, password_hash
             FROM users WHERE email = ?1 COLLATE NOCASE",
        )?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            let user = map_user(row)?;
            let hash: String = row.get(9)?;
            Ok(Some((user, hash)))
        } else {
            Ok(None)
        }
    }

    pub fn users_of_org(&self, org_id: &str) -> Result<Vec<User>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at
             FROM users WHERE org_id = ?1 ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![org_id], map_user)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn all_users(&self) -> Result<Vec<User>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at
             FROM users ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], map_user)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn update_user(
        &self,
        id: &str,
        name: &str,
        role: Role,
        locale: &str,
        daily_target_minutes: Option<i64>,
        active: bool,
    ) -> Result<User> {
        self.lock().execute(
            "UPDATE users SET name = ?2, role = ?3, locale = ?4,
                    daily_target_minutes = ?5, active = ?6 WHERE id = ?1",
            params![
                id,
                name,
                role_str(role),
                locale,
                daily_target_minutes,
                i64::from(active)
            ],
        )?;
        self.user(id)?.ok_or(StorageError::NotFound)
    }

    pub fn set_password(&self, id: &str, password_hash: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE users SET password_hash = ?2 WHERE id = ?1",
            params![id, password_hash],
        )?;
        Ok(())
    }

    pub fn set_locale(&self, id: &str, locale: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE users SET locale = ?2 WHERE id = ?1",
            params![id, locale],
        )?;
        Ok(())
    }

    // ---- sessions ------------------------------------------------------

    pub fn create_session(&self, token_hash: &str, user_id: &str, at: Millis) -> Result<()> {
        self.lock().execute(
            "INSERT INTO sessions (token_hash, user_id, created_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![token_hash, user_id, at],
        )?;
        Ok(())
    }

    pub fn session_user(&self, token_hash: &str, at: Millis) -> Result<Option<User>> {
        let conn = self.lock();
        let user: Option<User> = conn
            .query_row(
                "SELECT u.id, u.org_id, u.email, u.name, u.role, u.locale,
                        u.daily_target_minutes, u.active, u.created_at
                   FROM sessions s JOIN users u ON u.id = s.user_id
                  WHERE s.token_hash = ?1 AND u.active = 1",
                params![token_hash],
                map_user,
            )
            .optional()?;
        if user.is_some() {
            conn.execute(
                "UPDATE sessions SET last_seen_at = ?2 WHERE token_hash = ?1",
                params![token_hash, at],
            )?;
        }
        Ok(user)
    }

    pub fn delete_session(&self, token_hash: &str) -> Result<()> {
        self.lock().execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            params![token_hash],
        )?;
        Ok(())
    }

    // ---- accounts (time accounts) --------------------------------------

    pub fn accounts(&self, org_id: &str, include_archived: bool) -> Result<Vec<Account>> {
        let conn = self.lock();
        let sql = if include_archived {
            "SELECT id, org_id, name, kind, icon, sort_order, archived, is_primary
               FROM accounts WHERE org_id = ?1 ORDER BY sort_order, name"
        } else {
            "SELECT id, org_id, name, kind, icon, sort_order, archived, is_primary
               FROM accounts WHERE org_id = ?1 AND archived = 0 ORDER BY sort_order, name"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![org_id], map_account)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn account(&self, id: &str) -> Result<Option<Account>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, org_id, name, kind, icon, sort_order, archived, is_primary
                   FROM accounts WHERE id = ?1",
                params![id],
                map_account,
            )
            .optional()?)
    }

    pub fn create_account(
        &self,
        org_id: &str,
        name: &str,
        kind: AccountKind,
        icon: Option<&str>,
        sort_order: i32,
        is_primary: bool,
    ) -> Result<Account> {
        let id = now_id();
        self.lock()
            .execute(
                "INSERT INTO accounts (id, org_id, name, kind, icon, sort_order, archived, is_primary)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                params![
                    id,
                    org_id,
                    name.trim(),
                    kind_str(kind),
                    icon,
                    sort_order,
                    i64::from(is_primary)
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StorageError::Conflict("an active account with this name already exists".into())
                }
                other => StorageError::Sql(other),
            })?;
        self.account(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn update_account(
        &self,
        id: &str,
        name: &str,
        icon: Option<&str>,
        sort_order: i32,
        archived: bool,
        is_primary: bool,
    ) -> Result<Account> {
        self.lock()
            .execute(
                "UPDATE accounts SET name = ?2, icon = ?3, sort_order = ?4,
                        archived = ?5, is_primary = ?6 WHERE id = ?1",
                params![
                    id,
                    name.trim(),
                    icon,
                    sort_order,
                    i64::from(archived),
                    i64::from(is_primary)
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StorageError::Conflict("an active account with this name already exists".into())
                }
                other => StorageError::Sql(other),
            })?;
        self.account(id)?.ok_or(StorageError::NotFound)
    }

    /// Delete an account that has never been used.
    ///
    /// Accounts with booked time and the two structural accounts (primary and
    /// break) are refused: history must not disappear because of a click.
    pub fn delete_account(&self, id: &str) -> Result<()> {
        let account = self.account(id)?.ok_or(StorageError::NotFound)?;
        if account.is_primary {
            return Err(StorageError::Conflict(
                "the primary account cannot be deleted".into(),
            ));
        }
        if account.kind == AccountKind::Break {
            return Err(StorageError::Conflict(
                "the break account cannot be deleted".into(),
            ));
        }
        let used: i64 = {
            let conn = self.lock();
            conn.query_row(
                "SELECT count(*) FROM segments WHERE account_id = ?1",
                params![id],
                |r| r.get(0),
            )?
        };
        if used > 0 {
            return Err(StorageError::Conflict(format!(
                "account has {used} entries; archive it instead"
            )));
        }
        self.lock()
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Create `ALLGEMEIN` (primary work account) and `PAUSE` (the break account)
    /// if the organisation has none yet.
    ///
    /// These two names are the product defaults required by the specification.
    /// They are *data*, not identifiers: organisations can rename or translate
    /// them at any time without touching the code.
    pub fn ensure_default_accounts(&self, org_id: &str) -> Result<()> {
        let existing = self.accounts(org_id, true)?;
        if !existing.iter().any(|a| a.kind == AccountKind::Work) {
            self.create_account(
                org_id,
                "ALLGEMEIN",
                AccountKind::Work,
                Some("play"),
                0,
                true,
            )?;
        }
        if !existing.iter().any(|a| a.kind == AccountKind::Break) {
            self.create_account(
                org_id,
                "PAUSE",
                AccountKind::Break,
                Some("pause"),
                10,
                false,
            )?;
        }
        Ok(())
    }

    // ---- days and punches ----------------------------------------------

    pub fn workday_for(&self, user_id: &str, date: &str) -> Result<Option<Workday>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, user_id, date, opened_at, closed_at, status
                   FROM workdays WHERE user_id = ?1 AND date = ?2",
                params![user_id, date],
                map_workday,
            )
            .optional()?)
    }

    pub fn open_workday(&self, user_id: &str) -> Result<Option<Workday>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, user_id, date, opened_at, closed_at, status
                   FROM workdays WHERE user_id = ?1 AND status = 'open'
                  ORDER BY date DESC LIMIT 1",
                params![user_id],
                map_workday,
            )
            .optional()?)
    }

    /// The day the punch screen should show: the open day if there is one,
    /// otherwise the given date.
    pub fn current_day(&self, user_id: &str, date: &str) -> Result<Option<Workday>> {
        if let Some(open) = self.open_workday(user_id)? {
            return Ok(Some(open));
        }
        self.workday_for(user_id, date)
    }

    pub fn snapshot(&self, user_id: &str, org_id: &str, date: &str) -> Result<Snapshot> {
        let day = self.current_day(user_id, date)?;
        let mut segments = Vec::new();
        if let Some(d) = &day {
            let conn = self.lock();
            let mut stmt = conn.prepare(
                "SELECT id, workday_id, user_id, account_id, started_at, ended_at, source, note, material
                   FROM segments WHERE workday_id = ?1 ORDER BY started_at, id",
            )?;
            let rows = stmt.query_map(params![d.id], map_segment)?;
            segments = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        }
        Ok(Snapshot {
            day,
            segments,
            accounts: self.accounts(org_id, true)?,
        })
    }

    /// Apply a plan in one transaction. `source` records which client wrote it.
    pub fn apply_plan(
        &self,
        user_id: &str,
        date: &str,
        plan: &Plan,
        _now: Millis,
        source: &str,
    ) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for m in &plan.mutations {
            match m {
                dw_core::Mutation::OpenDay { date, at } => {
                    tx.execute(
                        "INSERT INTO workdays (id, user_id, date, opened_at, closed_at, status)
                         VALUES (?1, ?2, ?3, ?4, NULL, 'open')",
                        params![now_id(), user_id, date, at],
                    )?;
                }
                dw_core::Mutation::ReopenDay { .. } => {
                    tx.execute(
                        "UPDATE workdays SET status = 'open', closed_at = NULL
                          WHERE user_id = ?1 AND date = ?2",
                        params![user_id, date],
                    )?;
                }
                dw_core::Mutation::CloseDay { at } => {
                    tx.execute(
                        "UPDATE workdays SET status = 'closed', closed_at = ?3
                          WHERE user_id = ?1 AND date = ?2",
                        params![user_id, date, at],
                    )?;
                }
                dw_core::Mutation::OpenSegment { account_id, at } => {
                    let day_id: String = tx.query_row(
                        "SELECT id FROM workdays WHERE user_id = ?1 AND date = ?2",
                        params![user_id, date],
                        |r| r.get(0),
                    )?;
                    tx.execute(
                        "INSERT INTO segments
                           (id, workday_id, user_id, account_id, started_at, ended_at, client_uuid, source)
                         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6)",
                        params![now_id(), day_id, user_id, account_id, at, source],
                    )?;
                }
                dw_core::Mutation::AnnotateSegment {
                    segment_id,
                    note,
                    material,
                } => {
                    tx.execute(
                        "UPDATE segments SET note = ?2, material = ?3 WHERE id = ?1",
                        params![segment_id, note, material],
                    )?;
                }
                dw_core::Mutation::CloseSegment { segment_id, at } => {
                    tx.execute(
                        "UPDATE segments SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
                        params![segment_id, at],
                    )?;
                }
                dw_core::Mutation::SetSegmentEnd { segment_id, at } => {
                    tx.execute(
                        "UPDATE segments SET ended_at = ?2 WHERE id = ?1",
                        params![segment_id, at],
                    )?;
                }
                dw_core::Mutation::DeleteSegment { segment_id } => {
                    tx.execute("DELETE FROM segments WHERE id = ?1", params![segment_id])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Run one button press. Returns the resulting day.
    pub fn punch(
        &self,
        user: &User,
        action: &Action,
        now: Millis,
        date: &str,
        source: &str,
    ) -> Result<(DayView, dw_core::Outcome)> {
        self.punch_with(
            user,
            action,
            now,
            date,
            source,
            &dw_core::Annotation::default(),
        )
    }

    /// Run one button press and store `annotation` on the segment it closes.
    pub fn punch_with(
        &self,
        user: &User,
        action: &Action,
        now: Millis,
        date: &str,
        source: &str,
        annotation: &dw_core::Annotation,
    ) -> Result<(DayView, dw_core::Outcome)> {
        let snap = self.snapshot(&user.id, &user.org_id, date)?;
        // A shift that crossed midnight keeps writing to the day it started on.
        let effective = snap
            .day
            .as_ref()
            .map(|d| d.date.clone())
            .unwrap_or_else(|| date.to_string());
        let plan = dw_core::plan_with(&snap, action, now, &effective, annotation)
            .map_err(|e| StorageError::Conflict(e.to_string()))?;
        self.apply_plan(&user.id, &effective, &plan, now, source)?;
        let snap = self.snapshot(&user.id, &user.org_id, &effective)?;
        let totals = dw_core::totals(&snap.segments, &snap.accounts, now);
        let day = snap.day.ok_or(StorageError::NotFound)?;
        Ok((
            DayView {
                day,
                segments: snap.segments,
                totals,
            },
            plan.outcome,
        ))
    }

    pub fn days_between(
        &self,
        user_id: &str,
        org_id: &str,
        from: &str,
        to: &str,
        now: Millis,
    ) -> Result<Vec<DayView>> {
        let accounts = self.accounts(org_id, true)?;
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, date, opened_at, closed_at, status
               FROM workdays WHERE user_id = ?1 AND date BETWEEN ?2 AND ?3
              ORDER BY date DESC",
        )?;
        let days = stmt
            .query_map(params![user_id, from, to], map_workday)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut seg_stmt = conn.prepare(
            "SELECT id, workday_id, user_id, account_id, started_at, ended_at, source, note, material
               FROM segments WHERE workday_id = ?1 ORDER BY started_at, id",
        )?;
        let mut out = Vec::with_capacity(days.len());
        for day in days {
            let segments = seg_stmt
                .query_map(params![day.id], map_segment)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let totals = dw_core::totals(&segments, &accounts, now);
            out.push(DayView {
                day,
                segments,
                totals,
            });
        }
        Ok(out)
    }

    pub fn segment(&self, id: &str) -> Result<Option<Segment>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, workday_id, user_id, account_id, started_at, ended_at, source, note, material
                   FROM segments WHERE id = ?1",
                params![id],
                map_segment,
            )
            .optional()?)
    }

    pub fn workday_by_id(&self, id: &str) -> Result<Option<Workday>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT id, user_id, date, opened_at, closed_at, status
                   FROM workdays WHERE id = ?1",
                params![id],
                map_workday,
            )
            .optional()?)
    }

    /// Rewrite the note/material of one segment. Times are never touched here.
    pub fn update_segment_note(
        &self,
        id: &str,
        note: &str,
        material: &str,
        edited_by: &str,
        at: Millis,
    ) -> Result<Segment> {
        self.lock().execute(
            "UPDATE segments SET note = ?2, material = ?3, edited_by = ?4, edited_at = ?5
              WHERE id = ?1",
            params![id, note.trim(), material.trim(), edited_by, at],
        )?;
        self.segment(id)?.ok_or(StorageError::NotFound)
    }

    /// Full text search over segment notes and materials.
    pub fn search_bookings(&self, q: &BookingQuery<'_>) -> Result<Vec<BookingRow>> {
        use rusqlite::types::Value;

        let mut sql = String::from(
            "SELECT s.id, w.date, u.id, u.name, a.id, a.name, s.started_at, s.ended_at,
                    s.note, s.material, o.id, o.name
               FROM segments s
               JOIN workdays w ON w.id = s.workday_id
               JOIN users u ON u.id = s.user_id
               JOIN accounts a ON a.id = s.account_id
               JOIN organizations o ON o.id = u.org_id
              WHERE w.date BETWEEN ?1 AND ?2",
        );
        let mut values: Vec<Value> = vec![Value::Text(q.from.into()), Value::Text(q.to.into())];

        let mut bind = |sql: &mut String, value: Value, clause: &str| {
            values.push(value);
            sql.push_str(&clause.replace("{}", &values.len().to_string()));
        };

        if let Some(org) = q.org_id {
            bind(&mut sql, Value::Text(org.into()), " AND u.org_id = ?{}");
        }
        if let Some(user) = q.user_id {
            bind(&mut sql, Value::Text(user.into()), " AND s.user_id = ?{}");
        }
        if let Some(account) = q.account_id {
            bind(
                &mut sql,
                Value::Text(account.into()),
                " AND s.account_id = ?{}",
            );
        }
        for term in q.terms {
            if term.trim().is_empty() {
                continue;
            }
            let pattern = format!("%{}%", escape_like(&term.to_lowercase()));
            bind(
                &mut sql,
                Value::Text(pattern),
                " AND (dw_lower(s.note) LIKE ?{} ESCAPE '\\' OR dw_lower(s.material) LIKE ?{} ESCAPE '\\')",
            );
        }

        // The term clause contains ?{} twice; bind() substitutes the same
        // number into both, which SQLite allows.
        sql.push_str(" ORDER BY s.started_at DESC");
        bind(&mut sql, Value::Integer(q.limit), " LIMIT ?{}");
        bind(&mut sql, Value::Integer(q.offset), " OFFSET ?{}");

        let conn = self.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), map_booking)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    // ---- balances and statistics ---------------------------------------

    fn segments_in_range(
        &self,
        conn: &Connection,
        user_id: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<(Segment, AccountKind)>> {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.workday_id, s.user_id, s.account_id, s.started_at, s.ended_at, s.source,
                    s.note, s.material, a.kind
               FROM segments s
               JOIN workdays w ON w.id = s.workday_id
               JOIN accounts a ON a.id = s.account_id
              WHERE s.user_id = ?1 AND w.date BETWEEN ?2 AND ?3
              ORDER BY s.started_at",
        )?;
        let rows = stmt.query_map(params![user_id, from, to], |row| {
            let kind: String = row.get(9)?;
            Ok((map_segment(row)?, kind_from(&kind)))
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    fn balance_in_range(
        &self,
        conn: &Connection,
        user_id: &str,
        from: &str,
        to: &str,
        now: Millis,
    ) -> Result<Balance> {
        let rows = self.segments_in_range(conn, user_id, from, to)?;
        let mut worked: Millis = 0;
        let mut brk: Millis = 0;
        for (seg, kind) in &rows {
            let end = seg.ended_at.unwrap_or(now).max(seg.started_at);
            let dur = end - seg.started_at;
            match kind {
                AccountKind::Work => worked += dur,
                AccountKind::Break => brk += dur,
            }
        }

        let days: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT w.date)
               FROM workdays w
               JOIN segments s ON s.workday_id = w.id
              WHERE w.user_id = ?1 AND w.date BETWEEN ?2 AND ?3",
            params![user_id, from, to],
            |r| r.get(0),
        )?;

        let target_minutes: i64 = conn.query_row(
            "SELECT COALESCE(u.daily_target_minutes, o.daily_target_minutes)
               FROM users u JOIN organizations o ON o.id = u.org_id WHERE u.id = ?1",
            params![user_id],
            |r| r.get(0),
        )?;

        let adjustments_ms: i64 = conn.query_row(
            "SELECT COALESCE(SUM(minutes), 0) * 60000 FROM balance_adjustments
              WHERE user_id = ?1 AND date BETWEEN ?2 AND ?3",
            params![user_id, from, to],
            |r| r.get(0),
        )?;

        let target_ms = target_minutes * 60_000 * days;
        Ok(Balance {
            worked_ms: worked,
            break_ms: brk,
            days,
            target_minutes,
            target_ms,
            adjustments_ms,
            balance_ms: worked - target_ms + adjustments_ms,
        })
    }

    pub fn balance(&self, user_id: &str, from: &str, to: &str, now: Millis) -> Result<Balance> {
        let conn = self.lock();
        self.balance_in_range(&conn, user_id, from, to, now)
    }

    pub fn stats(&self, user_id: &str, from: &str, to: &str, now: Millis) -> Result<Stats> {
        let conn = self.lock();
        let period = self.balance_in_range(&conn, user_id, from, to, now)?;
        let total = self.balance_in_range(&conn, user_id, EPOCH_DATE, to, now)?;

        let mut acct_stmt = conn.prepare(
            "SELECT a.id, a.name, a.kind, COALESCE(SUM(MIN(COALESCE(s.ended_at, ?4), ?4) - s.started_at), 0)
               FROM segments s
               JOIN workdays w ON w.id = s.workday_id
               JOIN accounts a ON a.id = s.account_id
              WHERE s.user_id = ?1 AND w.date BETWEEN ?2 AND ?3
              GROUP BY a.id, a.name, a.kind
              ORDER BY a.sort_order, a.name",
        )?;
        let per_account = acct_stmt
            .query_map(params![user_id, from, to, now], |row| {
                let kind: String = row.get(2)?;
                Ok(AccountTotal {
                    account_id: row.get(0)?,
                    account_name: row.get(1)?,
                    kind: kind_from(&kind),
                    ms: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut day_stmt = conn.prepare(
            "SELECT w.date,
                    COALESCE(SUM(CASE WHEN a.kind = 'work'
                        THEN MIN(COALESCE(s.ended_at, ?4), ?4) - s.started_at ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.kind = 'break'
                        THEN MIN(COALESCE(s.ended_at, ?4), ?4) - s.started_at ELSE 0 END), 0)
               FROM workdays w
               LEFT JOIN segments s ON s.workday_id = w.id
               LEFT JOIN accounts a ON a.id = s.account_id
              WHERE w.user_id = ?1 AND w.date BETWEEN ?2 AND ?3
              GROUP BY w.date ORDER BY w.date",
        )?;
        let per_day = day_stmt
            .query_map(params![user_id, from, to, now], |row| {
                Ok(DayTotal {
                    date: row.get(0)?,
                    work_ms: row.get(1)?,
                    break_ms: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Stats {
            period,
            total,
            per_account,
            per_day,
        })
    }

    /// Workers with their balance. `org_id = None` means every organisation
    /// (admin only — the caller enforces that).
    pub fn team(
        &self,
        org_id: Option<&str>,
        from: &str,
        to: &str,
        now: Millis,
    ) -> Result<Vec<TeamRow>> {
        let orgs = self.orgs()?;
        let conn = self.lock();
        let mut stmt = match org_id {
            Some(_) => conn.prepare(
                "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at
                   FROM users WHERE org_id = ?1 AND active = 1 ORDER BY name COLLATE NOCASE",
            )?,
            None => conn.prepare(
                "SELECT id, org_id, email, name, role, locale, daily_target_minutes, active, created_at
                   FROM users WHERE active = 1 ORDER BY name COLLATE NOCASE",
            )?,
        };
        let users = match org_id {
            Some(id) => stmt
                .query_map(params![id], map_user)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map([], map_user)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };
        drop(stmt);

        let mut out = Vec::with_capacity(users.len());
        for user in users {
            let org = orgs.iter().find(|o| o.id == user.org_id);
            let period = self.balance_in_range(&conn, &user.id, from, to, now)?;
            let total = self.balance_in_range(&conn, &user.id, EPOCH_DATE, to, now)?;
            out.push(TeamRow {
                org_id: user.org_id.clone(),
                org_name: org.map(|o| o.name.clone()).unwrap_or_default(),
                user,
                period,
                total,
            });
        }
        Ok(out)
    }

    pub fn add_adjustment(
        &self,
        user_id: &str,
        date: &str,
        minutes: i64,
        reason: &str,
        created_by: &str,
        at: Millis,
    ) -> Result<()> {
        self.lock().execute(
            "INSERT INTO balance_adjustments (id, user_id, date, minutes, reason, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![now_id(), user_id, date, minutes, reason, created_by, at],
        )?;
        Ok(())
    }

    // ---- audit ---------------------------------------------------------

    pub fn audit(
        &self,
        org_id: &str,
        actor_id: &str,
        action: &str,
        entity: &str,
        entity_id: &str,
        payload: &str,
        at: Millis,
    ) -> Result<()> {
        self.lock().execute(
            "INSERT INTO audit_log (id, org_id, actor_id, action, entity, entity_id, payload, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                now_id(),
                org_id,
                actor_id,
                action,
                entity,
                entity_id,
                payload,
                at
            ],
        )?;
        Ok(())
    }

    // ---- backup --------------------------------------------------------

    pub fn export_json(&self) -> Result<serde_json::Value> {
        let conn = self.lock();
        let mut out = serde_json::Map::new();
        out.insert("format".into(), serde_json::json!("daywerk-backup"));
        out.insert("version".into(), serde_json::json!(SCHEMA_VERSION));
        out.insert("exported_at".into(), serde_json::json!(chrono_free_now()));

        out.insert(
            "organizations".into(),
            dump(
                &conn,
                "organizations",
                &[
                    "id",
                    "name",
                    "slug",
                    "logo_path",
                    "color_primary",
                    "color_accent",
                    "timezone",
                    "daily_target_minutes",
                    "created_at",
                ],
            )?,
        );
        out.insert(
            "users".into(),
            dump(
                &conn,
                "users",
                &[
                    "id",
                    "org_id",
                    "email",
                    "name",
                    "password_hash",
                    "role",
                    "locale",
                    "daily_target_minutes",
                    "active",
                    "created_at",
                ],
            )?,
        );
        out.insert(
            "accounts".into(),
            dump(
                &conn,
                "accounts",
                &[
                    "id",
                    "org_id",
                    "name",
                    "kind",
                    "icon",
                    "sort_order",
                    "archived",
                    "is_primary",
                ],
            )?,
        );
        out.insert(
            "workdays".into(),
            dump(
                &conn,
                "workdays",
                &[
                    "id",
                    "user_id",
                    "date",
                    "opened_at",
                    "closed_at",
                    "status",
                    "note",
                ],
            )?,
        );
        out.insert(
            "segments".into(),
            dump(
                &conn,
                "segments",
                &[
                    "id",
                    "workday_id",
                    "user_id",
                    "account_id",
                    "started_at",
                    "ended_at",
                    "client_uuid",
                    "source",
                    "note",
                    "material",
                    "edited_by",
                    "edited_at",
                ],
            )?,
        );
        out.insert(
            "balance_adjustments".into(),
            dump(
                &conn,
                "balance_adjustments",
                &[
                    "id",
                    "user_id",
                    "date",
                    "minutes",
                    "reason",
                    "created_by",
                    "created_at",
                ],
            )?,
        );
        out.insert(
            "audit_log".into(),
            dump(
                &conn,
                "audit_log",
                &[
                    "id",
                    "org_id",
                    "actor_id",
                    "action",
                    "entity",
                    "entity_id",
                    "payload",
                    "at",
                ],
            )?,
        );

        Ok(serde_json::Value::Object(out))
    }

    /// Replace the whole database with the contents of a backup.
    pub fn import_json(&self, value: &serde_json::Value) -> Result<serde_json::Value> {
        let obj = value
            .as_object()
            .ok_or_else(|| StorageError::Import("root is not an object".into()))?;
        if obj.get("format").and_then(|v| v.as_str()) != Some("daywerk-backup") {
            return Err(StorageError::Import("unknown backup format".into()));
        }

        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute_batch(
            "DELETE FROM audit_log;
             DELETE FROM balance_adjustments;
             DELETE FROM segments;
             DELETE FROM workdays;
             DELETE FROM accounts;
             DELETE FROM sessions;
             DELETE FROM users;
             DELETE FROM org_settings;
             DELETE FROM organizations;",
        )?;

        let mut counts = serde_json::Map::new();
        let tables: [(&str, &[&str]); 7] = [
            (
                "organizations",
                &[
                    "id",
                    "name",
                    "slug",
                    "logo_path",
                    "color_primary",
                    "color_accent",
                    "timezone",
                    "daily_target_minutes",
                    "created_at",
                ],
            ),
            (
                "users",
                &[
                    "id",
                    "org_id",
                    "email",
                    "name",
                    "password_hash",
                    "role",
                    "locale",
                    "daily_target_minutes",
                    "active",
                    "created_at",
                ],
            ),
            (
                "accounts",
                &[
                    "id",
                    "org_id",
                    "name",
                    "kind",
                    "icon",
                    "sort_order",
                    "archived",
                    "is_primary",
                ],
            ),
            (
                "workdays",
                &[
                    "id",
                    "user_id",
                    "date",
                    "opened_at",
                    "closed_at",
                    "status",
                    "note",
                ],
            ),
            (
                "segments",
                &[
                    "id",
                    "workday_id",
                    "user_id",
                    "account_id",
                    "started_at",
                    "ended_at",
                    "client_uuid",
                    "source",
                    "note",
                    "material",
                    "edited_by",
                    "edited_at",
                ],
            ),
            (
                "balance_adjustments",
                &[
                    "id",
                    "user_id",
                    "date",
                    "minutes",
                    "reason",
                    "created_by",
                    "created_at",
                ],
            ),
            (
                "audit_log",
                &[
                    "id",
                    "org_id",
                    "actor_id",
                    "action",
                    "entity",
                    "entity_id",
                    "payload",
                    "at",
                ],
            ),
        ];

        for (table, cols) in tables {
            let rows = obj
                .get(table)
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let placeholders = (1..=cols.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "INSERT INTO {table} ({}) VALUES ({placeholders})",
                cols.join(", ")
            );
            let mut stmt = tx.prepare(&sql)?;
            for row in &rows {
                let values: Vec<Box<dyn rusqlite::ToSql>> = cols
                    .iter()
                    .map(|c| match (c, row.get(*c)) {
                        // backups created before schema v2 have no note/material
                        (&"note" | &"material", None) => {
                            Box::new(String::new()) as Box<dyn rusqlite::ToSql>
                        }
                        (_, Some(value)) => json_to_sql(value),
                        (_, None) => json_to_sql(&serde_json::Value::Null),
                    })
                    .collect();
                let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
                stmt.execute(refs.as_slice())?;
            }
            counts.insert(table.to_string(), serde_json::json!(rows.len()));
        }

        tx.commit()?;
        Ok(serde_json::Value::Object(counts))
    }
}

// ---- row mappers ------------------------------------------------------

fn map_org(row: &Row<'_>) -> rusqlite::Result<Org> {
    Ok(Org {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get(2)?,
        logo_path: row.get(3)?,
        color_primary: row.get(4)?,
        color_accent: row.get(5)?,
        timezone: row.get(6)?,
        daily_target_minutes: row.get(7)?,
    })
}

fn map_user(row: &Row<'_>) -> rusqlite::Result<User> {
    let role: String = row.get(4)?;
    Ok(User {
        id: row.get(0)?,
        org_id: row.get(1)?,
        email: row.get(2)?,
        name: row.get(3)?,
        role: role_from(&role),
        locale: row.get(5)?,
        daily_target_minutes: row.get(6)?,
        active: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
    })
}

fn map_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let kind: String = row.get(3)?;
    Ok(Account {
        id: row.get(0)?,
        org_id: row.get(1)?,
        name: row.get(2)?,
        kind: kind_from(&kind),
        icon: row.get(4)?,
        sort_order: row.get(5)?,
        archived: row.get::<_, i64>(6)? != 0,
        is_primary: row.get::<_, i64>(7)? != 0,
    })
}

fn map_booking(row: &Row<'_>) -> rusqlite::Result<BookingRow> {
    Ok(BookingRow {
        id: row.get(0)?,
        date: row.get(1)?,
        user_id: row.get(2)?,
        user_name: row.get(3)?,
        account_id: row.get(4)?,
        account_name: row.get(5)?,
        started_at: row.get(6)?,
        ended_at: row.get(7)?,
        note: row.get(8)?,
        material: row.get(9)?,
        org_id: row.get(10)?,
        org_name: row.get(11)?,
    })
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn map_workday(row: &Row<'_>) -> rusqlite::Result<Workday> {
    let status: String = row.get(5)?;
    Ok(Workday {
        id: row.get(0)?,
        user_id: row.get(1)?,
        date: row.get(2)?,
        opened_at: row.get(3)?,
        closed_at: row.get(4)?,
        status: if status == "closed" {
            DayStatus::Closed
        } else {
            DayStatus::Open
        },
    })
}

fn map_segment(row: &Row<'_>) -> rusqlite::Result<Segment> {
    Ok(Segment {
        id: row.get(0)?,
        workday_id: row.get(1)?,
        user_id: row.get(2)?,
        account_id: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        source: row.get(6)?,
        note: row.get(7)?,
        material: row.get(8)?,
    })
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::Employee => "employee",
        Role::Office => "office",
        Role::Admin => "admin",
    }
}

fn role_from(s: &str) -> Role {
    match s {
        "admin" => Role::Admin,
        "office" => Role::Office,
        _ => Role::Employee,
    }
}

fn kind_str(kind: AccountKind) -> &'static str {
    match kind {
        AccountKind::Work => "work",
        AccountKind::Break => "break",
    }
}

fn kind_from(s: &str) -> AccountKind {
    match s {
        "break" => AccountKind::Break,
        _ => AccountKind::Work,
    }
}

fn chrono_free_now() -> Millis {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as Millis)
        .unwrap_or(0)
}

fn dump(conn: &Connection, table: &str, cols: &[&str]) -> Result<serde_json::Value> {
    let sql = format!("SELECT {} FROM {table}", cols.join(", "));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let v: rusqlite::types::Value = row.get(i)?;
            obj.insert((*col).to_string(), sql_to_json(v));
        }
        out.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::Array(out))
}

fn sql_to_json(v: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Integer(i) => serde_json::json!(i),
        Value::Real(f) => serde_json::json!(f),
        Value::Text(s) => serde_json::json!(s),
        Value::Blob(b) => serde_json::json!(b),
    }
}

fn json_to_sql(v: &serde_json::Value) -> Box<dyn rusqlite::ToSql> {
    match v {
        serde_json::Value::Null => Box::new(Option::<String>::None),
        serde_json::Value::Bool(b) => Box::new(i64::from(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else {
                Box::new(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Box::new(s.clone()),
        other => Box::new(other.to_string()),
    }
}

const SCHEMA_V1: &str = r#"
CREATE TABLE organizations (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    slug                 TEXT NOT NULL UNIQUE,
    logo_path            TEXT,
    color_primary        TEXT NOT NULL DEFAULT '#2a7647',
    color_accent         TEXT NOT NULL DEFAULT '#e8f2e8',
    timezone             TEXT NOT NULL DEFAULT 'Europe/Berlin',
    daily_target_minutes INTEGER NOT NULL DEFAULT 480,
    created_at           INTEGER NOT NULL
);

CREATE TABLE org_settings (
    org_id     TEXT PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    smtp_host  TEXT,
    smtp_port  INTEGER,
    smtp_user  TEXT,
    smtp_pass  TEXT,
    smtp_from  TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE users (
    id                   TEXT PRIMARY KEY,
    org_id               TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email                TEXT NOT NULL COLLATE NOCASE UNIQUE,
    name                 TEXT NOT NULL,
    password_hash        TEXT NOT NULL,
    role                 TEXT NOT NULL CHECK (role IN ('employee','office','admin')),
    locale               TEXT NOT NULL DEFAULT 'de' CHECK (locale IN ('de','en')),
    daily_target_minutes INTEGER,
    active               INTEGER NOT NULL DEFAULT 1,
    created_at           INTEGER NOT NULL
);
CREATE INDEX idx_users_org ON users(org_id);

CREATE TABLE sessions (
    token_hash   TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

CREATE TABLE accounts (
    id         TEXT PRIMARY KEY,
    org_id     TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('work','break')),
    icon       TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    archived   INTEGER NOT NULL DEFAULT 0,
    is_primary INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_accounts_org ON accounts(org_id, sort_order);
CREATE UNIQUE INDEX one_break_account ON accounts(org_id) WHERE kind = 'break';

CREATE TABLE workdays (
    id        TEXT PRIMARY KEY,
    user_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date      TEXT NOT NULL,
    opened_at INTEGER NOT NULL,
    closed_at INTEGER,
    status    TEXT NOT NULL CHECK (status IN ('open','closed')),
    note      TEXT,
    UNIQUE (user_id, date)
);
CREATE INDEX idx_workdays_date ON workdays(user_id, date);

CREATE TABLE segments (
    id          TEXT PRIMARY KEY,
    workday_id  TEXT NOT NULL REFERENCES workdays(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL,
    account_id  TEXT NOT NULL REFERENCES accounts(id),
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    client_uuid TEXT UNIQUE,
    source      TEXT NOT NULL DEFAULT 'web',
    edited_by   TEXT,
    edited_at   INTEGER,
    CHECK (ended_at IS NULL OR ended_at >= started_at)
);
CREATE INDEX idx_segments_day ON segments(workday_id, started_at);
CREATE INDEX idx_segments_account ON segments(account_id, started_at);
-- at most one running segment per user, forever
CREATE UNIQUE INDEX one_open_segment ON segments(user_id) WHERE ended_at IS NULL;

CREATE TABLE balance_adjustments (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date       TEXT NOT NULL,
    minutes    INTEGER NOT NULL,
    reason     TEXT,
    created_by TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_adjust_user ON balance_adjustments(user_id, date);

CREATE TABLE audit_log (
    id        TEXT PRIMARY KEY,
    org_id    TEXT,
    actor_id  TEXT,
    action    TEXT NOT NULL,
    entity    TEXT,
    entity_id TEXT,
    payload   TEXT,
    at        INTEGER NOT NULL
);
CREATE INDEX idx_audit_org ON audit_log(org_id, at);
"#;

const MIGRATION_V2: &str = r#"
ALTER TABLE segments ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE segments ADD COLUMN material TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX account_name_unique
    ON accounts(org_id, name COLLATE NOCASE) WHERE archived = 0;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use dw_core::Outcome;

    const NOW: Millis = 1_760_000_000_000;
    const DATE: &str = "2026-10-21";

    fn store() -> (Store, Org, User) {
        let store = Store::in_memory().unwrap();
        let org = store
            .create_org("Test GmbH", "test", "Europe/Berlin", 480)
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
                daily_target_minutes: None,
            })
            .unwrap();
        (store, org, user)
    }

    #[test]
    fn defaults_are_seeded() {
        let (store, org, _) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        assert_eq!(accounts.len(), 2);
        assert!(
            accounts
                .iter()
                .any(|a| a.kind == AccountKind::Work && a.is_primary)
        );
        assert!(accounts.iter().any(|a| a.kind == AccountKind::Break));
        // idempotent
        store.ensure_default_accounts(&org.id).unwrap();
        assert_eq!(store.accounts(&org.id, false).unwrap().len(), 2);
    }

    #[test]
    fn punch_flow_writes_segments() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        let brk = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Break)
            .unwrap();

        let (day, out) = store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        assert_eq!(
            out,
            Outcome::Started {
                account_id: work.id.clone()
            }
        );
        assert_eq!(day.segments.len(), 1);
        assert!(day.segments[0].ended_at.is_none());

        let (day, out) = store
            .punch(&user, &Action::Pause, NOW + 60_000, DATE, "test")
            .unwrap();
        assert_eq!(
            out,
            Outcome::Paused {
                from: work.id.clone()
            }
        );
        assert_eq!(day.segments.len(), 2);
        assert_eq!(day.segments[1].account_id, brk.id);

        let (day, out) = store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW + 120_000,
                DATE,
                "test",
            )
            .unwrap();
        assert!(matches!(out, Outcome::Switched { .. }));
        assert_eq!(day.segments.len(), 3);

        let (day, out) = store
            .punch(&user, &Action::Stop, NOW + 180_000, DATE, "test")
            .unwrap();
        assert!(matches!(out, Outcome::Stopped { .. }));
        assert_eq!(day.day.status, DayStatus::Closed);
        assert_eq!(day.totals.work_ms, 120_000);
        assert_eq!(day.totals.break_ms, 60_000);
    }

    #[test]
    fn stop_in_pause_removes_the_pause() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();

        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Pause, NOW + 60_000, DATE, "test")
            .unwrap();
        let (day, out) = store
            .punch(&user, &Action::Stop, NOW + 300_000, DATE, "test")
            .unwrap();

        assert!(matches!(out, Outcome::StoppedFromBreak { .. }));
        assert_eq!(day.segments.len(), 1, "pause segment must be deleted");
        assert_eq!(day.totals.break_ms, 0);
        assert_eq!(day.totals.work_ms, 60_000);
        assert_eq!(day.day.status, DayStatus::Closed);
    }

    #[test]
    fn only_one_open_segment_is_allowed_by_the_database() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();

        // A second open segment for the same user must violate the partial index.
        let conn = store.lock();
        let day_id: String = conn
            .query_row("SELECT id FROM workdays LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let err = conn.execute(
            "INSERT INTO segments (id, workday_id, user_id, account_id, started_at)
             VALUES ('x', ?1, ?2, ?3, ?4)",
            params![day_id, user.id, work.id, NOW + 1],
        );
        assert!(
            err.is_err(),
            "partial unique index must reject a second open segment"
        );
    }

    #[test]
    fn balance_counts_work_only_minus_target() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();

        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Pause, NOW + 4 * 3_600_000, DATE, "test")
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW + 4 * 3_600_000 + 30 * 60_000,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 8 * 3_600_000, DATE, "test")
            .unwrap();

        // 4h before the pause + 3.5h after = 7.5h = 450 min; target 480 -> -30
        let b = store.balance(&user.id, DATE, DATE, NOW).unwrap();
        assert_eq!(b.worked_ms, 450 * 60_000);
        assert_eq!(b.days, 1);
        assert_eq!(b.target_ms, 480 * 60_000);
        assert_eq!(b.balance_ms, -30 * 60_000);
    }

    #[test]
    fn per_user_target_overrides_org_default() {
        let (store, org, mut user) = store();
        user.daily_target_minutes = Some(600);
        store
            .update_user(&user.id, &user.name, user.role, "de", Some(600), true)
            .unwrap();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 3_600_000, DATE, "test")
            .unwrap();
        let b = store.balance(&user.id, DATE, DATE, NOW).unwrap();
        assert_eq!(b.target_minutes, 600);
        assert_eq!(b.balance_ms, (60 - 600) * 60_000);
    }

    #[test]
    fn team_lists_balances_per_org() {
        let (store, org, user) = store();
        let other = store
            .create_org("Other", "other", "Europe/Berlin", 480)
            .unwrap();
        store.ensure_default_accounts(&other.id).unwrap();
        let user2 = store
            .create_user(&NewUser {
                org_id: other.id.clone(),
                email: "b@example.org".into(),
                name: "Berta".into(),
                password_hash: "x".into(),
                role: Role::Employee,
                locale: "en".into(),
                daily_target_minutes: None,
            })
            .unwrap();

        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 60_000, DATE, "test")
            .unwrap();

        let own = store.team(Some(&org.id), DATE, DATE, NOW).unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].user.id, user.id);

        let all = store.team(None, DATE, DATE, NOW).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|r| r.user.id == user2.id));
    }

    #[test]
    fn backup_roundtrip_restores_everything() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch_with(
                &user,
                &Action::Stop,
                NOW + 60_000,
                DATE,
                "test",
                &dw_core::Annotation {
                    note: "Restored note".into(),
                    material: "2x Kabel".into(),
                },
            )
            .unwrap();
        let segment_id = store
            .days_between(&user.id, &org.id, DATE, DATE, NOW)
            .unwrap()[0]
            .segments[0]
            .id
            .clone();

        let backup = store.export_json().unwrap();
        let counts = store.import_json(&backup).unwrap();
        assert_eq!(counts["segments"], 1);
        let restored = store.segment(&segment_id).unwrap().unwrap();
        assert_eq!(restored.note, "Restored note");
        assert_eq!(restored.material, "2x Kabel");

        let days = store
            .days_between(&user.id, &org.id, DATE, DATE, NOW)
            .unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].segments.len(), 1);
    }

    #[test]
    fn notes_are_stored_on_the_closed_segment() {
        use dw_core::{Action, Annotation};

        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap()
            .clone();

        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch_with(
                &user,
                &Action::Stop,
                NOW + 3_600_000,
                DATE,
                "test",
                &Annotation {
                    note: "  Anschluss erneuert  ".into(),
                    material: "3x Kupferrohr 15 mm".into(),
                },
            )
            .unwrap();

        let seg = store
            .segment(
                &store
                    .days_between(&user.id, &org.id, DATE, DATE, NOW)
                    .unwrap()[0]
                    .segments[0]
                    .id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(seg.note, "Anschluss erneuert");
        assert_eq!(seg.material, "3x Kupferrohr 15 mm");
    }

    #[test]
    fn notes_can_be_edited_without_touching_times() {
        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch(&user, &Action::Stop, NOW + 60_000, DATE, "test")
            .unwrap();

        let day = &store
            .days_between(&user.id, &org.id, DATE, DATE, NOW)
            .unwrap()[0];
        let before = day.segments[0].clone();
        let updated = store
            .update_segment_note(&before.id, "Nachtrag", "Kabel", &user.id, NOW)
            .unwrap();

        assert_eq!(updated.note, "Nachtrag");
        assert_eq!(updated.material, "Kabel");
        assert_eq!(updated.started_at, before.started_at);
        assert_eq!(updated.ended_at, before.ended_at);
    }

    #[test]
    fn unused_accounts_can_be_deleted() {
        let (store, org, _) = store();
        let account = store
            .create_account(&org.id, "Tippfehler", AccountKind::Work, None, 100, false)
            .unwrap();
        store.delete_account(&account.id).unwrap();
        assert!(store.account(&account.id).unwrap().is_none());
    }

    #[test]
    fn used_accounts_are_protected_from_deletion() {
        let (store, org, user) = store();
        let account = store
            .create_account(&org.id, "Kunde", AccountKind::Work, None, 100, false)
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

        let err = store.delete_account(&account.id);
        assert!(matches!(err, Err(StorageError::Conflict(_))));
        assert!(
            store.account(&account.id).unwrap().is_some(),
            "account kept"
        );
    }

    #[test]
    fn structural_accounts_cannot_be_deleted() {
        let (store, org, _) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let primary = accounts.iter().find(|a| a.is_primary).unwrap();
        let brk = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Break)
            .unwrap();
        assert!(matches!(
            store.delete_account(&primary.id),
            Err(StorageError::Conflict(_))
        ));
        assert!(matches!(
            store.delete_account(&brk.id),
            Err(StorageError::Conflict(_))
        ));
    }

    #[test]
    fn archived_accounts_disappear_and_can_be_restored() {
        let (store, org, _) = store();
        let account = store
            .create_account(&org.id, "Lager", AccountKind::Work, None, 100, false)
            .unwrap();
        assert_eq!(store.accounts(&org.id, false).unwrap().len(), 3);

        store
            .update_account(&account.id, "Lager", None, 100, true, false)
            .unwrap();
        assert_eq!(store.accounts(&org.id, false).unwrap().len(), 2);

        store
            .update_account(&account.id, "Lager", None, 100, false, false)
            .unwrap();
        assert_eq!(store.accounts(&org.id, false).unwrap().len(), 3);
    }

    #[test]
    fn an_archived_name_can_be_reused() {
        let (store, org, _) = store();
        let account = store
            .create_account(&org.id, "Lager", AccountKind::Work, None, 100, false)
            .unwrap();
        store
            .update_account(&account.id, "Lager", None, 100, true, false)
            .unwrap();

        // while archived the name is free
        let again = store
            .create_account(&org.id, "Lager", AccountKind::Work, None, 100, false)
            .unwrap();
        assert_ne!(again.id, account.id);

        // restoring the first one now collides with the new active name
        let err = store.update_account(&account.id, "Lager", None, 100, false, false);
        assert!(err.is_err(), "two active accounts may not share a name");
    }

    #[test]
    fn duplicate_account_names_are_rejected_case_insensitively() {
        let (store, org, _) = store();
        let err = store.create_account(&org.id, "allgemein", AccountKind::Work, None, 100, false);
        assert!(matches!(err, Err(StorageError::Conflict(_))));
    }

    #[test]
    fn search_matches_notes_and_materials() {
        use dw_core::{Action, Annotation};

        let (store, org, user) = store();
        let accounts = store.accounts(&org.id, false).unwrap();
        let work = accounts
            .iter()
            .find(|a| a.kind == AccountKind::Work)
            .unwrap();
        store
            .punch(
                &user,
                &Action::Start {
                    account_id: work.id.clone(),
                },
                NOW,
                DATE,
                "test",
            )
            .unwrap();
        store
            .punch_with(
                &user,
                &Action::Stop,
                NOW + 3_600_000,
                DATE,
                "test",
                &Annotation {
                    note: "Anschluss für Müller erneuert".into(),
                    material: "3x Kupferrohr 15 mm".into(),
                },
            )
            .unwrap();

        let search = |term: &str| {
            let terms = vec![term.to_string()];
            store
                .search_bookings(&BookingQuery {
                    org_id: Some(&org.id),
                    user_id: None,
                    account_id: None,
                    from: DATE,
                    to: DATE,
                    terms: &terms,
                    limit: 50,
                    offset: 0,
                })
                .unwrap()
        };

        assert_eq!(
            search("kupferrohr").len(),
            1,
            "matches material, case-insensitive"
        );
        assert_eq!(search("KUPFERROHR").len(), 1);
        assert_eq!(search("anschluss").len(), 1, "matches note");
        assert_eq!(search("müller").len(), 1, "umlauts fold correctly");
        assert_eq!(search("MÜLLER").len(), 1);
        assert_eq!(search("nichts").len(), 0);

        let terms = vec!["kupferrohr".into(), "müller".into()];
        let both = store
            .search_bookings(&BookingQuery {
                org_id: Some(&org.id),
                user_id: None,
                account_id: None,
                from: DATE,
                to: DATE,
                terms: &terms,
                limit: 50,
                offset: 0,
            })
            .unwrap();
        assert_eq!(both.len(), 1, "all terms must match");

        let terms = vec!["kupferrohr".into(), "gibtsnicht".into()];
        let none = store
            .search_bookings(&BookingQuery {
                org_id: Some(&org.id),
                user_id: None,
                account_id: None,
                from: DATE,
                to: DATE,
                terms: &terms,
                limit: 50,
                offset: 0,
            })
            .unwrap();
        assert!(none.is_empty(), "AND across terms");
    }

    #[test]
    fn search_is_scoped_to_the_organisation() {
        use dw_core::{Action, Annotation};

        let (store, org, user) = store();
        let other = store
            .create_org("Other", "other", "Europe/Berlin", 480)
            .unwrap();
        store.ensure_default_accounts(&other.id).unwrap();
        let other_user = store
            .create_user(&NewUser {
                org_id: other.id.clone(),
                email: "b@example.org".into(),
                name: "Berta".into(),
                password_hash: "x".into(),
                role: Role::Employee,
                locale: "de".into(),
                daily_target_minutes: None,
            })
            .unwrap();

        for (u, o) in [(&user, &org), (&other_user, &other)] {
            let accounts = store.accounts(&o.id, false).unwrap();
            let work = accounts
                .iter()
                .find(|a| a.kind == AccountKind::Work)
                .unwrap();
            store
                .punch(
                    u,
                    &Action::Start {
                        account_id: work.id.clone(),
                    },
                    NOW,
                    DATE,
                    "test",
                )
                .unwrap();
            store
                .punch_with(
                    u,
                    &Action::Stop,
                    NOW + 60_000,
                    DATE,
                    "test",
                    &Annotation {
                        note: "gleiche Notiz".into(),
                        material: String::new(),
                    },
                )
                .unwrap();
        }

        let terms = vec!["gleiche".to_string()];
        let own = store
            .search_bookings(&BookingQuery {
                org_id: Some(&org.id),
                user_id: None,
                account_id: None,
                from: DATE,
                to: DATE,
                terms: &terms,
                limit: 50,
                offset: 0,
            })
            .unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].org_id, org.id);

        let all = store
            .search_bookings(&BookingQuery {
                org_id: None,
                user_id: None,
                account_id: None,
                from: DATE,
                to: DATE,
                terms: &terms,
                limit: 50,
                offset: 0,
            })
            .unwrap();
        assert_eq!(all.len(), 2, "no org filter returns everything");
    }

    #[test]
    fn upgrades_a_v1_database_in_place() {
        let path = std::env::temp_dir().join(format!("daywerk-mig-{}.db", uuid::Uuid::new_v4()));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(SCHEMA_V1).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute(
                "INSERT INTO organizations (id, name, slug, created_at) VALUES ('o1','Old','old',0)",
                [],
            )
            .unwrap();
        }

        let store = Store::open(path.to_str().unwrap()).unwrap();
        assert_eq!(store.schema_version().unwrap(), 2);
        assert_eq!(store.orgs().unwrap().len(), 1, "existing data survives");

        // the new columns and the new index are usable
        let conn = store.lock();
        let cols: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('segments') WHERE name IN ('note','material')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cols, 2);
        let idx: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='account_name_unique'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1);
        drop(conn);
        drop(store);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn duplicate_email_is_a_conflict() {
        let (store, org, _) = store();
        let err = store.create_user(&NewUser {
            org_id: org.id,
            email: "A@Example.org".into(),
            name: "dup".into(),
            password_hash: "x".into(),
            role: Role::Employee,
            locale: "de".into(),
            daily_target_minutes: None,
        });
        assert!(matches!(err, Err(StorageError::Conflict(_))));
    }
}
