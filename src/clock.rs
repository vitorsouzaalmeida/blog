use std::time::{SystemTime, UNIX_EPOCH};

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn year_from_unix_secs(secs: u64) -> i32 {
    fn from(year: i32, days_left: i32) -> i32 {
        let len = if is_leap(year) { 366 } else { 365 };
        if days_left < len {
            year
        } else {
            from(year + 1, days_left - len)
        }
    }
    from(1970, (secs / 86_400) as i32)
}

pub fn current_year() -> i32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    year_from_unix_secs(secs)
}
