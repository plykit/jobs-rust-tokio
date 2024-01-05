use cron::Schedule;
use std::str::FromStr;

pub fn secondly() -> Schedule {
    Schedule::from_str("* * * * * *").expect("secondly cron expression should parse")
}
pub fn minutely() -> Schedule {
    Schedule::from_str("0 * * * * *").expect("minutely cron expression should parse")
}
pub fn every_five_minutes() -> Schedule {
    Schedule::from_str("0 */5 * * * *").expect("every_five_minutes cron expression should parse")
}
