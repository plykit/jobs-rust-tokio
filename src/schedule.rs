use chrono::{DateTime, Utc};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct Schedule(cron::Schedule);

impl Schedule {
    pub fn due(&self, last: &DateTime<Utc>, now: DateTime<Utc>) -> bool {
        self.0
            .after(last)
            .next()
            .unwrap_or_else(|| DateTime::default())
            .lt(&now)
    }
}

impl FromStr for Schedule {
    type Err = InvalidCronExpression;

    fn from_str(s: &str) -> std::result::Result<Schedule, InvalidCronExpression> {
        cron::Schedule::from_str(s)
            .map_err(|e| InvalidCronExpression {
                expression: s.to_owned(),
                msg: e.to_string(),
            })
            .map(Schedule)
    }
}

impl From<Schedule> for String {
    fn from(value: Schedule) -> Self {
        value.0.to_string()
    }
}

pub fn secondly() -> Schedule {
    Schedule(
        cron::Schedule::from_str("* * * * * *").expect("secondly cron expression should parse"),
    )
}
pub fn minutely() -> Schedule {
    Schedule(
        cron::Schedule::from_str("0 * * * * *").expect("minutely cron expression should parse"),
    )
}
pub fn every_five_minutes() -> Schedule {
    Schedule(
        cron::Schedule::from_str("0 */5 * * * *")
            .expect("every_five_minutes cron expression should parse"),
    )
}

pub struct InvalidCronExpression {
    expression: String,
    msg: String,
}

impl Display for InvalidCronExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "invalid cron expression '{}': {}",
            self.expression, self.msg
        ))
    }
}

impl Debug for InvalidCronExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "invalid cron expression '{}': {}",
            self.expression, self.msg
        ))
    }
}

impl std::error::Error for InvalidCronExpression {}
