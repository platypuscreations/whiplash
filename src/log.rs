//! `whiplash.log` in the settings folder: problems worth reading in full
//! later, each with the time. A release build on Windows has no console,
//! so this is the only place they'd be seen. Any thread can write to it.

use std::fs;
use std::io::Write;

use crate::config;

/// Adds a line to the log. Writing it can fail (no settings folder, say);
/// then the line is just lost, as nothing else can be done about it.
pub fn write(text: &str) {
    let Some(folder) = config::folder() else { return };
    let line = format!("{} {text}\n", now());
    let _ = fs::create_dir_all(&folder).and_then(|()| {
        fs::OpenOptions::new().create(true).append(true).open(folder.join("whiplash.log"))?.write_all(line.as_bytes())
    });
}

/// The time now, for the log: like "2026-10-18 21:45:03 UTC".
fn now() -> String {
    utc_time(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

/// `seconds` since 1970 as a date and time in UTC.
fn utc_time(seconds: u64) -> String {
    let (days, time) = ((seconds / 86_400) as i64, seconds % 86_400);
    // Days since 1970 to a calendar date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", time / 3600, time / 60 % 60, time % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_times() {
        assert_eq!(super::utc_time(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(super::utc_time(951_868_799), "2000-02-29 23:59:59 UTC");
        assert_eq!(super::utc_time(1_792_359_903), "2026-10-18 21:45:03 UTC");
    }
}
