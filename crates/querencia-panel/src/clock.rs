use std::time::Duration;

use gtk4::{Label, glib, prelude::*};

const FORMAT: &str = "%d.%m.%Y %H:%M";

/// Shows the local time in `label` and refreshes it once per minute,
/// aligned to the minute boundary.
pub fn attach(label: &Label) {
    refresh(label);
}

fn refresh(label: &Label) {
    let delay = match glib::DateTime::now_local() {
        Ok(now) => {
            label.set_text(&format(&now));
            until_next_minute(now.second(), now.microsecond())
        }
        Err(_) => {
            label.set_text("--.--.---- --:--");
            Duration::from_secs(60)
        }
    };

    let label = label.downgrade();
    glib::timeout_add_local_once(delay, move || {
        if let Some(label) = label.upgrade() {
            refresh(&label);
        }
    });
}

fn format(time: &glib::DateTime) -> String {
    time.format(FORMAT).map(Into::into).unwrap_or_default()
}

/// Time left until the next full minute. Never zero, so a timer that
/// fires a little early simply re-arms instead of spinning.
fn until_next_minute(second: i32, microsecond: i32) -> Duration {
    let elapsed = Duration::from_secs(second.max(0) as u64)
        + Duration::from_micros(microsecond.max(0) as u64);
    Duration::from_secs(60)
        .saturating_sub(elapsed)
        .max(Duration::from_millis(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_minute_waits_sixty_seconds() {
        assert_eq!(until_next_minute(0, 0), Duration::from_secs(60));
    }

    #[test]
    fn mid_minute_waits_the_rest() {
        assert_eq!(
            until_next_minute(30, 500_000),
            Duration::from_millis(29_500)
        );
    }

    #[test]
    fn end_of_minute_and_leap_second_never_return_zero() {
        assert_eq!(until_next_minute(59, 999_999), Duration::from_millis(1));
        assert_eq!(until_next_minute(60, 0), Duration::from_millis(1));
    }

    #[test]
    fn formats_day_month_year_and_minutes() {
        let time = glib::DateTime::new(&glib::TimeZone::utc(), 2026, 10, 3, 9, 5, 42.7).unwrap();
        assert_eq!(format(&time), "03.10.2026 09:05");
    }
}
