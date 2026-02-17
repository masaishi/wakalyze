use chrono::{Datelike, Local, NaiveDate, TimeZone};

use crate::core::DaySessions;

pub fn format_duration(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    format!("{hours}h{minutes:02}m")
}

pub fn format_time(timestamp: i64) -> String {
    let dt = Local
        .timestamp_opt(timestamp, 0)
        .single()
        .expect("valid timestamp");
    let formatted = dt.format("%I:%M%p").to_string();
    formatted.trim_start_matches('0').to_lowercase()
}

pub fn format_date_short(date: NaiveDate) -> String {
    format!("{}/{}", date.month(), date.day())
}

pub fn build_lines(days: &[DaySessions], label: &str) -> Vec<String> {
    let mut lines = vec![label.to_string()];
    for (index, day) in days.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.push(format!("- {}", format_date_short(day.date)));
        for session in &day.sessions {
            let project = session.project.as_deref().unwrap_or("unknown");
            lines.push(format!(
                "  - {} ~ {} ({}) {}",
                format_time(session.start),
                format_time(session.end),
                format_duration(session.seconds),
                project,
            ));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Session;

    fn local_timestamp(year: i32, month: u32, day: u32, hour: u32, min: u32) -> i64 {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, min, 0)
            .unwrap();
        Local
            .from_local_datetime(&naive)
            .single()
            .expect("unambiguous local time")
            .timestamp()
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0), "0h00m");
    }

    #[test]
    fn format_duration_minutes_only() {
        assert_eq!(format_duration(300), "0h05m");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(3661), "1h01m");
    }

    #[test]
    fn format_duration_many_hours() {
        assert_eq!(format_duration(36000), "10h00m");
    }

    #[test]
    fn format_time_morning() {
        let ts = local_timestamp(2026, 2, 1, 9, 30);
        assert_eq!(format_time(ts), "9:30am");
    }

    #[test]
    fn format_time_afternoon() {
        let ts = local_timestamp(2026, 2, 1, 14, 5);
        assert_eq!(format_time(ts), "2:05pm");
    }

    #[test]
    fn format_time_noon() {
        let ts = local_timestamp(2026, 2, 1, 12, 0);
        assert_eq!(format_time(ts), "12:00pm");
    }

    #[test]
    fn format_time_midnight() {
        let ts = local_timestamp(2026, 2, 1, 0, 0);
        assert_eq!(format_time(ts), "12:00am");
    }

    #[test]
    fn format_date_short_basic() {
        assert_eq!(
            format_date_short(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
            "2/1"
        );
    }

    #[test]
    fn format_date_short_double_digit() {
        assert_eq!(
            format_date_short(NaiveDate::from_ymd_opt(2026, 12, 25).unwrap()),
            "12/25"
        );
    }

    #[test]
    fn build_lines_empty_days() {
        let result = build_lines(&[], "2026/02");
        assert_eq!(result, vec!["2026/02"]);
    }

    #[test]
    fn build_lines_with_sessions() {
        let ts1 = local_timestamp(2026, 2, 1, 9, 0);
        let ts2 = local_timestamp(2026, 2, 1, 10, 0);
        let days = vec![DaySessions {
            date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            sessions: vec![Session {
                start: ts1,
                end: ts2,
                seconds: 3600,
                project: Some("myproj".into()),
            }],
        }];
        let result = build_lines(&days, "2026/02");
        assert_eq!(result[0], "2026/02");
        assert_eq!(result[1], "- 2/1");
        assert!(result[2].contains("myproj"));
        assert!(result[2].contains("1h00m"));
    }
}
