// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Pure domain logic for Daywerk.
//!
//! This crate contains **no I/O**. It answers one question: given the current
//! day, the recorded segments and the available time accounts, what has to be
//! written when the user presses a button?
//!
//! The answer is a [`Plan`] — an ordered list of [`Mutation`]s that the storage
//! layer applies inside a single transaction. Keeping the plan separate from the
//! write makes every transition exhaustively testable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Id = String;
pub type Millis = i64;

/// A time account. `Break` is the single pause account; everything else is work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Work,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Employee,
    Office,
    Admin,
}

impl Role {
    /// Everyone who may look at more than their own data.
    pub fn sees_all(&self) -> bool {
        matches!(self, Role::Office | Role::Admin)
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: Id,
    pub org_id: Id,
    pub name: String,
    pub kind: AccountKind,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: Id,
    pub workday_id: Id,
    pub user_id: Id,
    pub account_id: Id,
    pub started_at: Millis,
    pub ended_at: Option<Millis>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workday {
    pub id: Id,
    pub user_id: Id,
    /// Local date (`YYYY-MM-DD`) the day belongs to. A shift that crosses
    /// midnight stays on the day it started.
    pub date: String,
    pub opened_at: Millis,
    pub closed_at: Option<Millis>,
    pub status: DayStatus,
}

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Start { account_id: Id },
    Pause,
    Stop,
}

/// Everything the planner needs to know. `segments` must be ordered by
/// `started_at` ascending.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub day: Option<Workday>,
    pub segments: Vec<Segment>,
    pub accounts: Vec<Account>,
}

/// One write. `OpenSegment` carries no id; storage assigns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mutation", rename_all = "snake_case")]
pub enum Mutation {
    OpenDay { date: String, at: Millis },
    ReopenDay { at: Millis },
    CloseDay { at: Millis },
    OpenSegment { account_id: Id, at: Millis },
    CloseSegment { segment_id: Id, at: Millis },
    SetSegmentEnd { segment_id: Id, at: Millis },
    DeleteSegment { segment_id: Id },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Started {
        account_id: Id,
    },
    Switched {
        from: Option<Id>,
        to: Id,
    },
    Paused {
        from: Id,
    },
    Stopped {
        last: Option<Id>,
    },
    /// Stop was pressed while the pause account was running: the pause is
    /// deleted and the previous work segment ends when the pause began.
    StoppedFromBreak {
        deleted_break: Id,
        closed_work: Option<Id>,
    },
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub mutations: Vec<Mutation>,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("unknown time account: {0}")]
    UnknownAccount(Id),
    #[error("time account is archived: {0}")]
    AccountArchived(String),
    #[error("no pause (break) account configured")]
    MissingBreakAccount,
    #[error("no account is running")]
    NoActiveAccount,
    #[error("the day is already closed")]
    DayAlreadyClosed,
    #[error("nothing is running on this day")]
    NotRunning,
}

fn kind_of(snapshot: &Snapshot, id: &Id) -> Result<AccountKind, CoreError> {
    snapshot
        .accounts
        .iter()
        .find(|a| &a.id == id)
        .map(|a| a.kind)
        .ok_or_else(|| CoreError::UnknownAccount(id.clone()))
}

/// The segment that is currently running, if any.
pub fn active_segment(snapshot: &Snapshot) -> Option<&Segment> {
    snapshot.segments.iter().find(|s| s.ended_at.is_none())
}

/// Compute the mutations for one button press.
pub fn plan(
    snapshot: &Snapshot,
    action: &Action,
    now: Millis,
    date: &str,
) -> Result<Plan, CoreError> {
    let active = active_segment(snapshot);

    match action {
        Action::Start { account_id } => {
            let account = snapshot
                .accounts
                .iter()
                .find(|a| &a.id == account_id)
                .ok_or_else(|| CoreError::UnknownAccount(account_id.clone()))?;
            if account.archived {
                return Err(CoreError::AccountArchived(account.name.clone()));
            }

            // Switching to the account that is already running is a no-op and
            // must not reopen or restamp anything.
            if let Some(seg) = active {
                if &seg.account_id == account_id {
                    return Ok(Plan {
                        mutations: Vec::new(),
                        outcome: Outcome::Noop,
                    });
                }
            }

            let mut mutations = Vec::new();
            match snapshot.day.as_ref() {
                None => mutations.push(Mutation::OpenDay {
                    date: date.to_string(),
                    at: now,
                }),
                Some(day) if day.status == DayStatus::Closed => {
                    mutations.push(Mutation::ReopenDay { at: now })
                }
                _ => {}
            }

            let outcome = match active {
                Some(seg) => {
                    mutations.push(Mutation::CloseSegment {
                        segment_id: seg.id.clone(),
                        at: now,
                    });
                    mutations.push(Mutation::OpenSegment {
                        account_id: account_id.clone(),
                        at: now,
                    });
                    Outcome::Switched {
                        from: Some(seg.account_id.clone()),
                        to: account_id.clone(),
                    }
                }
                None => {
                    mutations.push(Mutation::OpenSegment {
                        account_id: account_id.clone(),
                        at: now,
                    });
                    Outcome::Started {
                        account_id: account_id.clone(),
                    }
                }
            };

            Ok(Plan { mutations, outcome })
        }

        Action::Pause => {
            let seg = active.ok_or(CoreError::NoActiveAccount)?;
            if kind_of(snapshot, &seg.account_id)? == AccountKind::Break {
                return Ok(Plan {
                    mutations: Vec::new(),
                    outcome: Outcome::Noop,
                });
            }
            let brk = snapshot
                .accounts
                .iter()
                .find(|a| a.kind == AccountKind::Break && !a.archived)
                .ok_or(CoreError::MissingBreakAccount)?;

            Ok(Plan {
                mutations: vec![
                    Mutation::CloseSegment {
                        segment_id: seg.id.clone(),
                        at: now,
                    },
                    Mutation::OpenSegment {
                        account_id: brk.id.clone(),
                        at: now,
                    },
                ],
                outcome: Outcome::Paused {
                    from: seg.account_id.clone(),
                },
            })
        }

        Action::Stop => {
            let day = snapshot.day.as_ref().ok_or(CoreError::NotRunning)?;
            if day.status == DayStatus::Closed {
                return Err(CoreError::DayAlreadyClosed);
            }

            let mut mutations = Vec::new();
            let outcome = match active {
                Some(seg) => {
                    if kind_of(snapshot, &seg.account_id)? == AccountKind::Break {
                        // Stop while paused: the pause never happened. Delete it
                        // and make sure the previous work segment ends when the
                        // pause began.
                        let paused_at = seg.started_at;
                        mutations.push(Mutation::DeleteSegment {
                            segment_id: seg.id.clone(),
                        });

                        let last_work = snapshot.segments.iter().rev().find(|s| {
                            s.id != seg.id
                                && snapshot
                                    .accounts
                                    .iter()
                                    .any(|a| a.id == s.account_id && a.kind == AccountKind::Work)
                        });

                        if let Some(work) = last_work {
                            if work.ended_at != Some(paused_at) {
                                mutations.push(Mutation::SetSegmentEnd {
                                    segment_id: work.id.clone(),
                                    at: paused_at,
                                });
                            }
                        }

                        Outcome::StoppedFromBreak {
                            deleted_break: seg.account_id.clone(),
                            closed_work: last_work.map(|w| w.account_id.clone()),
                        }
                    } else {
                        mutations.push(Mutation::CloseSegment {
                            segment_id: seg.id.clone(),
                            at: now,
                        });
                        Outcome::Stopped {
                            last: Some(seg.account_id.clone()),
                        }
                    }
                }
                None => Outcome::Stopped { last: None },
            };

            mutations.push(Mutation::CloseDay { at: now });
            Ok(Plan { mutations, outcome })
        }
    }
}

/// Aggregated durations of one day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub work_ms: Millis,
    pub break_ms: Millis,
    pub span_ms: Millis,
}

/// Sum the segments, counting an open segment up to `until`.
pub fn totals(segments: &[Segment], accounts: &[Account], until: Millis) -> Totals {
    let mut t = Totals::default();
    for seg in segments {
        let end = seg.ended_at.unwrap_or(until).max(seg.started_at);
        let dur = end - seg.started_at;
        match accounts
            .iter()
            .find(|a| a.id == seg.account_id)
            .map(|a| a.kind)
        {
            Some(AccountKind::Break) => t.break_ms += dur,
            Some(AccountKind::Work) => t.work_ms += dur,
            None => {}
        }
    }
    t.span_ms = t.work_ms + t.break_ms;
    t
}

/// Hours balance for a period: worked time minus the daily target for every
/// day that actually has a workday, plus manual adjustments.
pub fn balance_ms(work_ms: Millis, target_minutes: i64, days: i64) -> Millis {
    work_ms - target_minutes * 60_000 * days
}

/// Format a duration as `H:MM` (negative values keep their sign).
pub fn format_duration(ms: Millis) -> String {
    let neg = ms < 0;
    let abs = ms.abs();
    let minutes = abs / 60_000;
    let s = format!("{}:{:02}", minutes / 60, minutes % 60);
    if neg { format!("-{s}") } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Millis = 1_000_000;
    const DATE: &str = "2026-10-21";

    fn acct(id: &str, name: &str, kind: AccountKind) -> Account {
        Account {
            id: id.into(),
            org_id: "org".into(),
            name: name.into(),
            kind,
            icon: None,
            sort_order: 0,
            archived: false,
            is_primary: false,
        }
    }

    fn accounts() -> Vec<Account> {
        vec![
            // The seeded default account names come from the requirements.
            acct("general", "ALLGEMEIN", AccountKind::Work),
            acct("pause", "PAUSE", AccountKind::Break),
            acct("customer", "Customer TWx", AccountKind::Work),
        ]
    }

    fn seg(id: &str, account: &str, start: Millis, end: Option<Millis>) -> Segment {
        Segment {
            id: id.into(),
            workday_id: "day".into(),
            user_id: "u1".into(),
            account_id: account.into(),
            started_at: start,
            ended_at: end,
            source: "test".into(),
        }
    }

    fn day(status: DayStatus) -> Workday {
        Workday {
            id: "day".into(),
            user_id: "u1".into(),
            date: DATE.into(),
            opened_at: 0,
            closed_at: None,
            status,
        }
    }

    fn snap(segments: Vec<Segment>, status: Option<DayStatus>) -> Snapshot {
        Snapshot {
            day: status.map(day),
            segments,
            accounts: accounts(),
        }
    }

    fn start(id: &str) -> Action {
        Action::Start {
            account_id: id.into(),
        }
    }

    #[test]
    fn first_press_opens_day_and_segment() {
        let s = snap(vec![], None);
        let p = plan(&s, &start("general"), NOW, DATE).unwrap();
        assert_eq!(
            p.mutations,
            vec![
                Mutation::OpenDay {
                    date: DATE.into(),
                    at: NOW
                },
                Mutation::OpenSegment {
                    account_id: "general".into(),
                    at: NOW
                },
            ]
        );
        assert_eq!(
            p.outcome,
            Outcome::Started {
                account_id: "general".into()
            }
        );
    }

    #[test]
    fn project_can_be_the_first_account() {
        let s = snap(vec![], None);
        let p = plan(&s, &start("customer"), NOW, DATE).unwrap();
        assert!(matches!(p.outcome, Outcome::Started { .. }));
        assert!(p.mutations.contains(&Mutation::OpenSegment {
            account_id: "customer".into(),
            at: NOW
        }));
    }

    #[test]
    fn switching_closes_previous_at_same_instant() {
        let s = snap(vec![seg("s1", "general", 100, None)], Some(DayStatus::Open));
        let p = plan(&s, &start("customer"), NOW, DATE).unwrap();
        assert_eq!(
            p.mutations,
            vec![
                Mutation::CloseSegment {
                    segment_id: "s1".into(),
                    at: NOW
                },
                Mutation::OpenSegment {
                    account_id: "customer".into(),
                    at: NOW
                },
            ]
        );
        assert_eq!(
            p.outcome,
            Outcome::Switched {
                from: Some("general".into()),
                to: "customer".into()
            }
        );
    }

    #[test]
    fn starting_the_running_account_is_a_noop() {
        let s = snap(vec![seg("s1", "general", 100, None)], Some(DayStatus::Open));
        let p = plan(&s, &start("general"), NOW, DATE).unwrap();
        assert!(p.mutations.is_empty());
        assert_eq!(p.outcome, Outcome::Noop);
    }

    #[test]
    fn pause_closes_work_and_opens_break() {
        let s = snap(vec![seg("s1", "general", 100, None)], Some(DayStatus::Open));
        let p = plan(&s, &Action::Pause, NOW, DATE).unwrap();
        assert_eq!(
            p.mutations,
            vec![
                Mutation::CloseSegment {
                    segment_id: "s1".into(),
                    at: NOW
                },
                Mutation::OpenSegment {
                    account_id: "pause".into(),
                    at: NOW
                },
            ]
        );
        assert_eq!(
            p.outcome,
            Outcome::Paused {
                from: "general".into()
            }
        );
    }

    #[test]
    fn pausing_twice_is_a_noop() {
        let s = snap(vec![seg("s1", "pause", 100, None)], Some(DayStatus::Open));
        let p = plan(&s, &Action::Pause, NOW, DATE).unwrap();
        assert!(p.mutations.is_empty());
        assert_eq!(p.outcome, Outcome::Noop);
    }

    #[test]
    fn pause_without_running_account_errors() {
        let s = snap(vec![], Some(DayStatus::Open));
        assert_eq!(
            plan(&s, &Action::Pause, NOW, DATE),
            Err(CoreError::NoActiveAccount)
        );
    }

    #[test]
    fn resume_from_pause_closes_break() {
        let s = snap(vec![seg("s2", "pause", 200, None)], Some(DayStatus::Open));
        let p = plan(&s, &start("general"), NOW, DATE).unwrap();
        assert_eq!(
            p.mutations,
            vec![
                Mutation::CloseSegment {
                    segment_id: "s2".into(),
                    at: NOW
                },
                Mutation::OpenSegment {
                    account_id: "general".into(),
                    at: NOW
                },
            ]
        );
    }

    #[test]
    fn stop_while_working_closes_segment_and_day() {
        let s = snap(vec![seg("s1", "general", 100, None)], Some(DayStatus::Open));
        let p = plan(&s, &Action::Stop, NOW, DATE).unwrap();
        assert_eq!(
            p.mutations,
            vec![
                Mutation::CloseSegment {
                    segment_id: "s1".into(),
                    at: NOW
                },
                Mutation::CloseDay { at: NOW },
            ]
        );
        assert_eq!(
            p.outcome,
            Outcome::Stopped {
                last: Some("general".into())
            }
        );
    }

    #[test]
    fn stop_while_paused_deletes_pause_and_closes_day() {
        // Work ran 100..200, pause started at 200 and is still open.
        let s = snap(
            vec![
                seg("s1", "general", 100, Some(200)),
                seg("s2", "pause", 200, None),
            ],
            Some(DayStatus::Open),
        );
        let p = plan(&s, &Action::Stop, NOW, DATE).unwrap();

        // The work segment already ends at the pause start, so only the pause
        // is deleted and the day is closed.
        assert_eq!(
            p.mutations,
            vec![
                Mutation::DeleteSegment {
                    segment_id: "s2".into()
                },
                Mutation::CloseDay { at: NOW },
            ]
        );
        assert_eq!(
            p.outcome,
            Outcome::StoppedFromBreak {
                deleted_break: "pause".into(),
                closed_work: Some("general".into())
            }
        );
    }

    #[test]
    fn stop_while_paused_corrects_a_wrong_work_end() {
        // The work segment was (incorrectly) recorded as ending at 250 while the
        // pause began at 200. Stop must pull the end back to the pause start.
        let s = snap(
            vec![
                seg("s1", "general", 100, Some(250)),
                seg("s2", "pause", 200, None),
            ],
            Some(DayStatus::Open),
        );
        let p = plan(&s, &Action::Stop, NOW, DATE).unwrap();
        assert!(p.mutations.contains(&Mutation::SetSegmentEnd {
            segment_id: "s1".into(),
            at: 200
        }));
        assert!(p.mutations.contains(&Mutation::DeleteSegment {
            segment_id: "s2".into()
        }));
        assert!(p.mutations.contains(&Mutation::CloseDay { at: NOW }));
    }

    #[test]
    fn stop_without_running_account_closes_day() {
        let s = snap(vec![], Some(DayStatus::Open));
        let p = plan(&s, &Action::Stop, NOW, DATE).unwrap();
        assert_eq!(p.mutations, vec![Mutation::CloseDay { at: NOW }]);
        assert_eq!(p.outcome, Outcome::Stopped { last: None });
    }

    #[test]
    fn stop_without_day_errors() {
        let s = snap(vec![], None);
        assert_eq!(
            plan(&s, &Action::Stop, NOW, DATE),
            Err(CoreError::NotRunning)
        );
    }

    #[test]
    fn stop_on_closed_day_errors() {
        let s = snap(vec![], Some(DayStatus::Closed));
        assert_eq!(
            plan(&s, &Action::Stop, NOW, DATE),
            Err(CoreError::DayAlreadyClosed)
        );
    }

    #[test]
    fn start_on_closed_day_reopens_it() {
        let s = snap(
            vec![seg("s1", "general", 100, Some(200))],
            Some(DayStatus::Closed),
        );
        let p = plan(&s, &start("general"), NOW, DATE).unwrap();
        assert!(p.mutations.contains(&Mutation::ReopenDay { at: NOW }));
        assert!(p.mutations.contains(&Mutation::OpenSegment {
            account_id: "general".into(),
            at: NOW
        }));
    }

    #[test]
    fn unknown_account_is_rejected() {
        let s = snap(vec![], None);
        assert_eq!(
            plan(&s, &start("nope"), NOW, DATE),
            Err(CoreError::UnknownAccount("nope".into()))
        );
    }

    #[test]
    fn archived_account_is_rejected() {
        let mut s = snap(vec![], None);
        s.accounts[2].archived = true;
        assert!(matches!(
            plan(&s, &start("customer"), NOW, DATE),
            Err(CoreError::AccountArchived(_))
        ));
    }

    #[test]
    fn missing_break_account_is_reported() {
        let mut s = snap(vec![seg("s1", "general", 100, None)], Some(DayStatus::Open));
        s.accounts.retain(|a| a.kind != AccountKind::Break);
        assert_eq!(
            plan(&s, &Action::Pause, NOW, DATE),
            Err(CoreError::MissingBreakAccount)
        );
    }

    #[test]
    fn full_requirement_flow() {
        // Requirement example: general → customer → break → general, then stop.
        let mut s = snap(vec![], None);

        let p1 = plan(&s, &start("general"), 0, DATE).unwrap();
        s.day = Some(day(DayStatus::Open));
        s.segments.push(seg("s1", "general", 0, None));

        let p2 = plan(&s, &start("customer"), 100, DATE).unwrap();
        assert!(p2.mutations.contains(&Mutation::CloseSegment {
            segment_id: "s1".into(),
            at: 100
        }));
        s.segments[0].ended_at = Some(100);
        s.segments.push(seg("s2", "customer", 100, None));

        let p3 = plan(&s, &Action::Pause, 250, DATE).unwrap();
        assert!(p3.mutations.contains(&Mutation::CloseSegment {
            segment_id: "s2".into(),
            at: 250
        }));
        s.segments[1].ended_at = Some(250);
        s.segments.push(seg("s3", "pause", 250, None));

        let p4 = plan(&s, &start("general"), 400, DATE).unwrap();
        assert!(p4.mutations.contains(&Mutation::CloseSegment {
            segment_id: "s3".into(),
            at: 400
        }));
        s.segments[2].ended_at = Some(400);
        s.segments.push(seg("s4", "general", 400, None));

        let p5 = plan(&s, &Action::Stop, 500, DATE).unwrap();
        assert_eq!(
            p5.mutations,
            vec![
                Mutation::CloseSegment {
                    segment_id: "s4".into(),
                    at: 500
                },
                Mutation::CloseDay { at: 500 },
            ]
        );

        // Totals: work 0..100 + 100..250 + 400..500 = 350ms, break 250..400 = 150ms
        s.segments[3].ended_at = Some(500);
        let t = totals(&s.segments, &s.accounts, 500);
        assert_eq!(t.work_ms, 350);
        assert_eq!(t.break_ms, 150);
        assert_eq!(t.span_ms, 500);
        assert_eq!(
            p1.outcome,
            Outcome::Started {
                account_id: "general".into()
            }
        );
    }

    #[test]
    fn totals_count_open_segment_up_to_now() {
        let s = snap(
            vec![seg("s1", "general", 1_000, None)],
            Some(DayStatus::Open),
        );
        let t = totals(&s.segments, &s.accounts, 61_000);
        assert_eq!(t.work_ms, 60_000);
        assert_eq!(t.break_ms, 0);
    }

    #[test]
    fn balance_uses_target_and_days() {
        // 8h target, 1 day, 7.5h worked -> -0.5h
        assert_eq!(
            balance_ms(7 * 3_600_000 + 30 * 60_000, 480, 1),
            -30 * 60_000
        );
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(90 * 60_000), "1:30");
        assert_eq!(format_duration(-30 * 60_000), "-0:30");
        assert_eq!(format_duration(8 * 3_600_000 + 5 * 60_000), "8:05");
    }

    #[test]
    fn role_ordering() {
        assert!(Role::Admin > Role::Office);
        assert!(Role::Office > Role::Employee);
        assert!(Role::Admin.is_admin());
        assert!(Role::Office.sees_all());
        assert!(!Role::Employee.sees_all());
    }
}
