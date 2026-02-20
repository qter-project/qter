use std::{fmt::Display, sync::LazyLock};

use chrono::{DateTime, Duration, Month, TimeZone};
use chrono_tz::{Tz, US::Pacific};

/// Times that are before `start` or after `end` are allowed, and times between breaks are allowed
#[derive(Debug)]
struct Day {
    talks_start: DateTime<Tz>,
    talks_end: DateTime<Tz>,
    tables_close: DateTime<Tz>,
    breaks: Vec<(DateTime<Tz>, DateTime<Tz>)>,
}

// Inputs a date and return a function that take in (hours, minutes) and returns a DateTime
fn day(day: u32) -> impl Fn(u32, u32) -> DateTime<Tz> {
    move |hour, min| {
        Pacific
            .with_ymd_and_hms(2026, Month::February.number_from_month(), day, hour, min, 0)
            .unwrap()
    }
}

static SCHEDULE: LazyLock<Vec<Day>> = LazyLock::new(|| {
    // https://www.gathering4gardner.org/g4g16-program.pdf
    let thursday = day(19);
    let friday = day(20);
    let saturday = day(21);
    let sunday = day(22);
    vec![
        Day {
            talks_start: thursday(8, 30),
            talks_end: thursday(17, 30),
            tables_close: thursday(20, 0),
            breaks: vec![
                (thursday(10, 0), thursday(10, 30)),
                (thursday(12, 0), thursday(13, 30)),
                (thursday(15, 15), thursday(15, 45)),
            ],
        },
        Day {
            talks_start: friday(8, 30),
            talks_end: friday(17, 30),
            tables_close: friday(20, 0),
            breaks: vec![
                (friday(10, 0), friday(10, 30)),
                (friday(12, 0), friday(13, 30)),
                (friday(15, 15), friday(15, 45)),
            ],
        },
        Day {
            talks_start: saturday(8, 30),
            talks_end: saturday(12, 0),
            tables_close: saturday(20, 0),
            breaks: vec![(saturday(10, 0), saturday(10, 30))],
        },
        Day {
            talks_start: sunday(8, 30),
            talks_end: sunday(12, 0),
            tables_close: sunday(13, 0),
            breaks: vec![(sunday(10, 0), sunday(10, 30))],
        },
    ]
});

fn format_duration(f: &mut std::fmt::Formatter<'_>, duration: Duration) -> std::fmt::Result {
    let mins = duration.num_minutes();
    write!(f, "{}:{}", mins / 60, mins % 60)
}

/// The current status that the robot should be in given the conference schedule. The `Display` implementation outputs a message and a timer
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Status {
    /// The robot is disabled for the given duration
    TimeUntilEnabled(Duration),
    /// The robot is enabled for the given duration
    TimeUntilDisabled(Duration),
    /// We're after the talks but before the official end of the conference for the day. We'll say that the robot can still run after the official end of the day but we can still have a timer to it.
    TimeUntilOfficialClose(Duration),
    /// The conference is over; the robot can run
    ConferenceOver,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::TimeUntilEnabled(time_delta) => {
                write!(f, "Time until talks end: ")?;
                format_duration(f, *time_delta)
            },
            Status::TimeUntilDisabled(time_delta) => {
                write!(f, "Time until talks start: ")?;
                format_duration(f, *time_delta)
            },
            Status::TimeUntilOfficialClose(time_delta) => {
                write!(f, "Time until exhibit tables officially close: ")?;
                format_duration(f, *time_delta)
            },
            Status::ConferenceOver => write!(f, "Conference is over!"),
        }
    }
}

impl Status {
    pub fn robot_enabled(&self) -> bool {
        // !matches!(self, Status::TimeUntilEnabled(_))
        true
    }
}

fn status_known_tz(now: DateTime<Tz>) -> Status {
    let Some(day) = SCHEDULE.iter().find(|day| now < day.tables_close) else {
        return Status::ConferenceOver;
    };

    if now < day.talks_start {
        return Status::TimeUntilDisabled(day.talks_start - now);
    }

    if now <= day.talks_end {
        for (start, end) in &day.breaks {
            if *start <= now && now <= *end {
                return Status::TimeUntilDisabled(*end - now);
            }
        }

        return Status::TimeUntilEnabled(match day.breaks.iter().find(|v| now < v.0) {
            Some((start, _)) => *start - now,
            None => day.talks_end - now,
        });
    }

    Status::TimeUntilOfficialClose(day.tables_close - now)
}

pub fn status<Tz: TimeZone>(now: DateTime<Tz>) -> Status {
    status_known_tz(now.with_timezone(&Pacific))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use chrono_tz::US::{Pacific, Eastern};

    use crate::{Status, status};

    #[test]
    fn test_status() {
        // Thursday
        
        // Before talks
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 19, 7, 30, 0).unwrap()),
            Status::TimeUntilDisabled(Duration::hours(1)),
        );

        // During talks
        assert_eq!(
            status(Eastern.with_ymd_and_hms(2026, 2, 19, 12, 30, 0).unwrap()),
            Status::TimeUntilEnabled(Duration::minutes(30)),
        );

        // During talks + UTC conv
        assert_eq!(
            status(Eastern.with_ymd_and_hms(2026, 2, 19, 12, 30, 0).unwrap().with_timezone(&Utc)),
            Status::TimeUntilEnabled(Duration::minutes(30)),
        );

        // During break
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 19, 12, 30, 0).unwrap()),
            Status::TimeUntilDisabled(Duration::hours(1)),
        );

        // After talks
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 19, 18, 00, 0).unwrap()),
            Status::TimeUntilOfficialClose(Duration::hours(2)),
        );

        // After day
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 19, 21, 30, 0).unwrap()),
            Status::TimeUntilDisabled(Duration::hours(11)),
        );

        // Saturday

        // After talks
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 21, 12, 30, 0).unwrap()),
            Status::TimeUntilOfficialClose(Duration::minutes(30 + 60*7)),
        );

        // During talks
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 21, 11, 30, 0).unwrap()),
            Status::TimeUntilEnabled(Duration::minutes(30)),
        );

        // During break
        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 21, 10, 15, 0).unwrap()),
            Status::TimeUntilDisabled(Duration::minutes(15)),
        );

        // Sunday

        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 22, 12, 30, 0).unwrap()),
            Status::TimeUntilOfficialClose(Duration::minutes(30)),
        );

        // After conference

        assert_eq!(
            status(Pacific.with_ymd_and_hms(2026, 2, 22, 13, 30, 0).unwrap()),
            Status::ConferenceOver,
        );

        assert_eq!(
            status(Eastern.with_ymd_and_hms(2026, 2, 25, 21, 30, 0).unwrap()),
            Status::ConferenceOver,
        );
    }
}
