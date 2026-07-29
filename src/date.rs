//! Civil dates in the proleptic Gregorian calendar.
//!
//! Time of day and time zones are left out on purpose: `rfc822` pins every
//! date to midnight GMT.

use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Date {
    y: i32,
    m: u32,
    d: u32,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

impl Date {
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Result<Date, String> {
        match days_in_month(y, m) {
            0 => Err(format!("month out of range: {m}")),
            last if !(1..=last).contains(&d) => {
                Err(format!("day out of range for {y:04}-{m:02}: {d}"))
            }
            _ => Ok(Date { y, m, d }),
        }
    }

    pub fn parse(s: &str) -> Result<Date, String> {
        let date_part = s.split(['T', ' ']).next().unwrap_or(s);
        let [y, m, d] = date_part.split('-').collect::<Vec<_>>()[..] else {
            return Err(format!("expected date as YYYY-MM-DD, got {s:?}"));
        };
        let bad = |name: &str| format!("invalid {name} in date {s:?}");
        Date::from_ymd(
            y.trim().parse().map_err(|_| bad("year"))?,
            m.trim().parse().map_err(|_| bad("month"))?,
            d.trim().parse().map_err(|_| bad("day"))?,
        )
        .map_err(|e| format!("{e} in date {s:?}"))
    }

    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }

    pub fn dotted(&self) -> String {
        format!("{:04} · {:02} · {:02}", self.y, self.m, self.d)
    }

    fn weekday(&self) -> usize {
        let (y, m) = (self.y, self.m as i32);
        let mm = if m < 3 { m + 12 } else { m };
        let y = if m < 3 { y - 1 } else { y };
        let (k, j) = (y % 100, y / 100);
        let h = (self.d as i32 + (13 * (mm + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
        ((h + 6) % 7) as usize
    }

    pub fn rfc822(&self) -> String {
        format!(
            "{}, {:02} {} {:04} 00:00:00 GMT",
            WEEKDAYS[self.weekday()],
            self.d,
            MONTHS[(self.m - 1) as usize],
            self.y
        )
    }
}

impl Ord for Date {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.y, self.m, self.d).cmp(&(o.y, o.m, o.d))
    }
}
impl PartialOrd for Date {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_dates_are_rejected_not_defaulted() {
        for bad in [
            "",
            "not-a-date",
            "2023",
            "2023-08",
            "2023-8x-13",
            "2023-13-01",
        ] {
            assert!(Date::parse(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn impossible_dates_are_rejected() {
        for bad in [
            "2023-02-29",
            "2023-02-31",
            "1900-02-29",
            "2023-04-31",
            "2023-06-31",
            "2023-00-01",
            "2023-01-00",
        ] {
            assert!(Date::parse(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn leap_days_that_exist_are_accepted() {
        for good in ["2024-02-29", "2000-02-29", "2023-02-28", "2023-12-31"] {
            assert!(Date::parse(good).is_ok(), "{good:?} should parse");
        }
    }

    #[test]
    fn weekdays_match_the_gregorian_calendar() {
        for (date, weekday) in [
            ("2026-07-27", "Mon"),
            ("2026-06-24", "Wed"),
            ("2025-11-04", "Tue"),
            ("2024-11-03", "Sun"),
            ("2023-08-12", "Sat"),
            ("2024-02-29", "Thu"),
            ("2000-02-29", "Tue"),
            ("1900-03-01", "Thu"),
        ] {
            let got = Date::parse(date).unwrap().rfc822();
            assert!(
                got.starts_with(weekday),
                "{date} should be a {weekday}, got {got}"
            );
        }
    }

    #[test]
    fn dates_order_by_year_then_month_then_day() {
        let d = |s| Date::parse(s).unwrap();
        assert!(d("2023-08-12") < d("2023-08-13"));
        assert!(d("2023-08-31") < d("2023-09-01"));
        assert!(d("2023-12-31") < d("2024-01-01"));
    }
}
