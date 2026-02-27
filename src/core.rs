use std::collections::BTreeSet;

use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::error::{Result, WakalyzeError};

pub const DEFAULT_MAX_GAP_SECONDS: i64 = 15 * 60;

#[derive(Debug, Clone, Deserialize)]
pub struct RawHeartbeat {
    pub time: Option<f64>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatEntry {
    pub time: i64,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub start: i64,
    pub end: i64,
    pub seconds: i64,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaySessions {
    pub date: NaiveDate,
    pub sessions: Vec<Session>,
}

pub fn parse_month(value: &str) -> Result<NaiveDate> {
    let (year_str, month_str) = value.split_once('/').ok_or(WakalyzeError::InvalidMonth)?;

    if year_str.len() != 4 || month_str.len() != 2 {
        return Err(WakalyzeError::InvalidMonth);
    }

    let year: i32 = year_str.parse().map_err(|_| WakalyzeError::InvalidMonth)?;
    let month: u32 = month_str.parse().map_err(|_| WakalyzeError::InvalidMonth)?;

    NaiveDate::from_ymd_opt(year, month, 1).ok_or(WakalyzeError::InvalidMonth)
}

pub fn month_last_day(first_day: NaiveDate) -> NaiveDate {
    let (year, month) = if first_day.month() == 12 {
        (first_day.year() + 1, 1)
    } else {
        (first_day.year(), first_day.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
}

pub fn iter_dates(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    std::iter::successors(Some(start).filter(|&d| d <= end), |d| {
        d.succ_opt().filter(|&d| d <= end)
    })
    .collect()
}

pub fn week_range(first_day: NaiveDate, week: u32) -> Result<(NaiveDate, NaiveDate)> {
    if !(1..=6).contains(&week) {
        return Err(WakalyzeError::InvalidWeek);
    }
    let dow = first_day.weekday().num_days_from_sunday() as i64;
    let week1_start = first_day - chrono::Duration::days(dow);
    let start = week1_start + chrono::Duration::days(((week - 1) * 7) as i64);
    let end = start + chrono::Duration::days(6);
    let last = month_last_day(first_day);
    if start > last {
        return Err(WakalyzeError::WeekOutOfRange(week));
    }
    Ok((start, end))
}

pub fn estimate_seconds(times: &[i64], max_gap: i64) -> i64 {
    let unique: BTreeSet<i64> = times.iter().copied().collect();
    let sorted: Vec<i64> = unique.into_iter().collect();
    let mut total: i64 = 0;
    for pair in sorted.windows(2) {
        let gap = pair[1] - pair[0];
        if gap > 0 && gap <= max_gap {
            total += gap;
        }
    }
    total
}

pub fn extract_entries(heartbeats: &[RawHeartbeat]) -> Vec<HeartbeatEntry> {
    let mut entries: Vec<HeartbeatEntry> = heartbeats
        .iter()
        .filter_map(|hb| {
            let time = hb.time? as i64;
            let project = hb
                .project
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .map(String::from);
            Some(HeartbeatEntry { time, project })
        })
        .collect();
    entries.sort_by_key(|e| e.time);
    entries
}

pub fn build_sessions(heartbeats: &[RawHeartbeat], max_gap: i64) -> Vec<Session> {
    let entries = extract_entries(heartbeats);
    if entries.is_empty() {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    let mut current_times = vec![entries[0].time];
    let mut current_project = entries[0].project.clone();
    let mut prev_time = entries[0].time;

    for entry in &entries[1..] {
        if entry.time == prev_time {
            continue;
        }
        let gap = entry.time - prev_time;
        if gap <= max_gap && entry.project == current_project {
            current_times.push(entry.time);
        } else {
            sessions.push(make_session(
                &current_times,
                current_project.as_deref(),
                max_gap,
            ));
            current_times = vec![entry.time];
            current_project = entry.project.clone();
        }
        prev_time = entry.time;
    }

    sessions.push(make_session(
        &current_times,
        current_project.as_deref(),
        max_gap,
    ));
    sessions
}

fn make_session(times: &[i64], project: Option<&str>, max_gap: i64) -> Session {
    Session {
        start: times[0],
        end: *times.last().unwrap(),
        seconds: estimate_seconds(times, max_gap),
        project: project.map(str::to_owned),
    }
}

pub fn filter_sessions(days: &[DaySessions], filter: Option<&str>) -> Vec<DaySessions> {
    let term = match filter {
        Some(t) if !t.is_empty() => t,
        _ => return days.to_vec(),
    };

    let needles: Vec<String> = term
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    if needles.is_empty() {
        return days.to_vec();
    }

    days.iter()
        .filter_map(|day| {
            let sessions: Vec<Session> = day
                .sessions
                .iter()
                .filter(|s| {
                    let proj = s.project.as_deref().unwrap_or("").to_lowercase();
                    needles.iter().any(|needle| proj.contains(needle.as_str()))
                })
                .cloned()
                .collect();
            if sessions.is_empty() {
                None
            } else {
                Some(DaySessions {
                    date: day.date,
                    sessions,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: i64 = DEFAULT_MAX_GAP_SECONDS;

    fn hb(time: f64, project: &str) -> RawHeartbeat {
        RawHeartbeat {
            time: Some(time),
            project: Some(project.to_string()),
        }
    }

    #[test]
    fn parse_month_valid() {
        assert_eq!(
            parse_month("2026/02").unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()
        );
    }

    #[test]
    fn parse_month_invalid_format() {
        assert!(parse_month("2026-02").is_err());
    }

    #[test]
    fn parse_month_no_leading_zero() {
        assert!(parse_month("2026/2").is_err());
    }

    #[test]
    fn parse_month_invalid_month_number() {
        assert!(parse_month("2026/00").is_err());
        assert!(parse_month("2026/13").is_err());
    }

    #[test]
    fn month_last_day_january() {
        let first = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(
            month_last_day(first),
            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
        );
    }

    #[test]
    fn month_last_day_february_non_leap() {
        let first = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        assert_eq!(
            month_last_day(first),
            NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()
        );
    }

    #[test]
    fn month_last_day_february_leap() {
        let first = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
        assert_eq!(
            month_last_day(first),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn month_last_day_december() {
        let first = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
        assert_eq!(
            month_last_day(first),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        );
    }

    #[test]
    fn month_last_day_april() {
        let first = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        assert_eq!(
            month_last_day(first),
            NaiveDate::from_ymd_opt(2025, 4, 30).unwrap()
        );
    }

    #[test]
    fn iter_dates_single_day() {
        let d = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert_eq!(iter_dates(d, d), vec![d]);
    }

    #[test]
    fn iter_dates_range() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 2, 3).unwrap();
        let result = iter_dates(start, end);
        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(),
            ]
        );
    }

    #[test]
    fn iter_dates_empty_when_start_after_end() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 5).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert_eq!(iter_dates(start, end), Vec::<NaiveDate>::new());
    }

    #[test]
    fn week_range_week_1_feb_2026() {
        // Feb 1 2026 is Sunday → week 1 starts on Mon Jan 26
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let (start, end) = week_range(first, 1).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 1, 26).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
    }

    #[test]
    fn week_range_week_5_feb_2026() {
        // Week 5 spans into March
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let (start, end) = week_range(first, 5).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 2, 23).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
    }

    #[test]
    fn week_range_first_is_monday() {
        // Jun 1 2026 is Monday → week 1 starts on Jun 1
        let first = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let (start, end) = week_range(first, 1).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 6, 7).unwrap());
    }

    #[test]
    fn week_range_invalid_0() {
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert!(week_range(first, 0).is_err());
    }

    #[test]
    fn week_range_invalid_7() {
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert!(week_range(first, 7).is_err());
    }

    #[test]
    fn week_range_out_of_month() {
        let first = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert!(week_range(first, 6).is_err());
    }

    #[test]
    fn estimate_seconds_empty() {
        assert_eq!(estimate_seconds(&[], GAP), 0);
    }

    #[test]
    fn estimate_seconds_single() {
        assert_eq!(estimate_seconds(&[100], GAP), 0);
    }

    #[test]
    fn estimate_seconds_within_gap() {
        assert_eq!(estimate_seconds(&[100, 200], GAP), 100);
    }

    #[test]
    fn estimate_seconds_exceeds_gap() {
        assert_eq!(estimate_seconds(&[100, 100 + 15 * 60 + 1], GAP), 0);
    }

    #[test]
    fn estimate_seconds_exact_gap_limit() {
        assert_eq!(estimate_seconds(&[100, 100 + 15 * 60], GAP), 15 * 60);
    }

    #[test]
    fn estimate_seconds_duplicates_ignored() {
        assert_eq!(estimate_seconds(&[100, 100, 200], GAP), 100);
    }

    #[test]
    fn estimate_seconds_multiple_segments() {
        assert_eq!(estimate_seconds(&[100, 200, 300], GAP), 200);
    }

    #[test]
    fn extract_entries_empty() {
        assert_eq!(extract_entries(&[]), Vec::<HeartbeatEntry>::new());
    }

    #[test]
    fn extract_entries_valid() {
        let heartbeats = vec![hb(200.0, "foo"), hb(100.0, "bar")];
        let result = extract_entries(&heartbeats);
        assert_eq!(
            result,
            vec![
                HeartbeatEntry {
                    time: 100,
                    project: Some("bar".into())
                },
                HeartbeatEntry {
                    time: 200,
                    project: Some("foo".into())
                },
            ]
        );
    }

    #[test]
    fn extract_entries_skips_missing_time() {
        let heartbeats = vec![RawHeartbeat {
            time: None,
            project: Some("foo".into()),
        }];
        assert_eq!(extract_entries(&heartbeats), Vec::<HeartbeatEntry>::new());
    }

    #[test]
    fn extract_entries_float_time() {
        let heartbeats = vec![hb(100.5, "foo")];
        let result = extract_entries(&heartbeats);
        assert_eq!(
            result,
            vec![HeartbeatEntry {
                time: 100,
                project: Some("foo".into())
            }]
        );
    }

    #[test]
    fn extract_entries_empty_project_becomes_none() {
        let heartbeats = vec![RawHeartbeat {
            time: Some(100.0),
            project: Some("  ".into()),
        }];
        let result = extract_entries(&heartbeats);
        assert_eq!(
            result,
            vec![HeartbeatEntry {
                time: 100,
                project: None
            }]
        );
    }

    #[test]
    fn extract_entries_missing_project_becomes_none() {
        let heartbeats = vec![RawHeartbeat {
            time: Some(100.0),
            project: None,
        }];
        let result = extract_entries(&heartbeats);
        assert_eq!(
            result,
            vec![HeartbeatEntry {
                time: 100,
                project: None
            }]
        );
    }

    #[test]
    fn build_sessions_empty() {
        assert_eq!(build_sessions(&[], GAP), Vec::<Session>::new());
    }

    #[test]
    fn build_sessions_single_heartbeat() {
        let heartbeats = vec![hb(1000.0, "foo")];
        let sessions = build_sessions(&heartbeats, GAP);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].start, 1000);
        assert_eq!(sessions[0].end, 1000);
        assert_eq!(sessions[0].seconds, 0);
        assert_eq!(sessions[0].project, Some("foo".into()));
    }

    #[test]
    fn build_sessions_two_heartbeats_same_session() {
        let heartbeats = vec![hb(1000.0, "foo"), hb(1300.0, "foo")];
        let sessions = build_sessions(&heartbeats, GAP);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].start, 1000);
        assert_eq!(sessions[0].end, 1300);
        assert_eq!(sessions[0].seconds, 300);
    }

    #[test]
    fn build_sessions_project_change_splits() {
        let heartbeats = vec![hb(1000.0, "foo"), hb(1300.0, "bar")];
        let sessions = build_sessions(&heartbeats, GAP);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].project, Some("foo".into()));
        assert_eq!(sessions[1].project, Some("bar".into()));
    }

    #[test]
    fn build_sessions_gap_splits() {
        let heartbeats = vec![hb(1000.0, "foo"), hb((1000 + 15 * 60 + 1) as f64, "foo")];
        let sessions = build_sessions(&heartbeats, GAP);
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn build_sessions_duplicate_timestamps_skipped() {
        let heartbeats = vec![hb(1000.0, "foo"), hb(1000.0, "foo"), hb(1300.0, "foo")];
        let sessions = build_sessions(&heartbeats, GAP);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].start, 1000);
        assert_eq!(sessions[0].end, 1300);
        assert_eq!(sessions[0].seconds, 300);
    }

    #[test]
    fn filter_sessions_none_returns_all() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: 1,
                end: 2,
                seconds: 1,
                project: Some("foo".into()),
            }],
        }];
        assert_eq!(filter_sessions(&days, None), days);
    }

    #[test]
    fn filter_sessions_empty_returns_all() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: 1,
                end: 2,
                seconds: 1,
                project: Some("foo".into()),
            }],
        }];
        assert_eq!(filter_sessions(&days, Some("")), days);
    }

    #[test]
    fn filter_sessions_matches_substring() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![
                Session {
                    start: 1,
                    end: 2,
                    seconds: 1,
                    project: Some("my-project".into()),
                },
                Session {
                    start: 3,
                    end: 4,
                    seconds: 1,
                    project: Some("other".into()),
                },
            ],
        }];
        let result = filter_sessions(&days, Some("proj"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sessions.len(), 1);
        assert_eq!(result[0].sessions[0].project, Some("my-project".into()));
    }

    #[test]
    fn filter_sessions_case_insensitive() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: 1,
                end: 2,
                seconds: 1,
                project: Some("MyProject".into()),
            }],
        }];
        let result = filter_sessions(&days, Some("myproject"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_sessions_comma_separated_match_any() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![
                Session {
                    start: 1,
                    end: 2,
                    seconds: 1,
                    project: Some("foo".into()),
                },
                Session {
                    start: 3,
                    end: 4,
                    seconds: 1,
                    project: Some("bar".into()),
                },
                Session {
                    start: 5,
                    end: 6,
                    seconds: 1,
                    project: Some("baz".into()),
                },
            ],
        }];
        let result = filter_sessions(&days, Some("foo,bar"));
        assert_eq!(result.len(), 1);
        let projects: Vec<_> = result[0]
            .sessions
            .iter()
            .map(|s| s.project.clone())
            .collect();
        assert_eq!(projects, vec![Some("foo".into()), Some("bar".into())]);
    }

    #[test]
    fn filter_sessions_comma_separated_trimmed() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: 1,
                end: 2,
                seconds: 1,
                project: Some("bar".into()),
            }],
        }];
        let result = filter_sessions(&days, Some(" , bar , "));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_sessions_removes_empty_days() {
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: 1,
                end: 2,
                seconds: 1,
                project: Some("foo".into()),
            }],
        }];
        let result = filter_sessions(&days, Some("bar"));
        assert!(result.is_empty());
    }
}
