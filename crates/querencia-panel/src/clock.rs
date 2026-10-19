//! The clock applet: the local date and time, refreshed every minute. A
//! left click opens the month calendar below it, as in GNOME 2 (G2-AD,
//! QD-052); a second click or Escape closes it. In the calendar
//! (`gtk4::Calendar`) the arrow keys move between the days, Space selects
//! one and Ctrl+Left/Right turn the month; labwc gives the grabbing popover
//! the keyboard although the panel has none of its own
//! (`KeyboardMode::None`), as for the volume popup.
//!
//! GTK takes month and day names and the first day of the week from the
//! time locale (`LC_TIME`). The Querencia session sets `LANG=en_US.UTF-8`
//! with `LANGUAGE=de`, which would give an English calendar starting on
//! Sunday; the calendar is therefore created under the time locale of the
//! interface language ([`time_locale`]), for this thread only.

use std::ffi::CString;
use std::time::Duration;

use gtk4::{Calendar, GestureClick, Label, Popover, gdk, glib, prelude::*};

const FORMAT: &str = "%d.%m.%Y %H:%M";
/// CSS class of the calendar popover (style in `main.rs`).
pub const CALENDAR_CLASS: &str = "querencia-calendar";

/// The clock applet: a label with the time and the calendar popover.
pub fn widget() -> Label {
    let label = Label::new(None);
    attach(&label);
    attach_calendar(&label);
    label
}

/// Shows the local time in `label` and refreshes it once per minute,
/// aligned to the minute boundary.
fn attach(label: &Label) {
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

/// The calendar popover of `label`, opened by a left click. Every opening
/// shows today's month with today selected; GTK marks today (`.today`).
fn attach_calendar(label: &Label) {
    let calendar = with_time_locale(Calendar::new);
    let popover = Popover::builder().has_arrow(true).child(&calendar).build();
    popover.add_css_class(CALENDAR_CLASS);
    popover.set_parent(label);

    popover.connect_show(glib::clone!(
        #[weak]
        calendar,
        move |_| {
            if let Ok(today) = glib::DateTime::now_local() {
                calendar.select_day(&today);
            }
            calendar.grab_focus();
            eprintln!(
                "querencia-panel: Uhr: Kalender geöffnet ({})",
                shown_month(&calendar)
            );
        }
    ));
    popover.connect_closed(|_| eprintln!("querencia-panel: Uhr: Kalender geschlossen"));
    // Browsing is logged for the isolated GUI test.
    let browsed = |calendar: &Calendar| {
        eprintln!(
            "querencia-panel: Uhr: Kalender zeigt {}",
            shown_month(calendar)
        );
    };
    calendar.connect_month_notify(browsed);
    calendar.connect_year_notify(browsed);

    // The press that closes an open popover never gets here: GTK hides an
    // autohide popover on a press outside it and drops that press. So a
    // second click on the clock only closes the calendar.
    let click = GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.connect_pressed(glib::clone!(
        #[weak]
        popover,
        move |_, _, _, _| {
            if popover.is_visible() {
                popover.popdown();
            } else {
                popover.popup();
            }
        }
    ));
    label.add_controller(click);
    label.connect_destroy(move |_| {
        if popover.parent().is_some() {
            popover.unparent();
        }
    });
}

/// "MM/YYYY" of the month `calendar` shows.
fn shown_month(calendar: &Calendar) -> String {
    format!("{:02}/{}", calendar.month() + 1, calendar.year())
}

/// The time locale (`LC_TIME`) the calendar needs so that its names and its
/// first weekday follow the interface language: `None` if the time locale
/// already has that language (or there is none to follow), else a UTF-8
/// locale of the language, e.g. `de_DE.UTF-8` for `LANGUAGE=de`. Like
/// gettext, the first entry of `LANGUAGE` counts, unless the messages
/// locale is C, which disables `LANGUAGE`; `LC_ALL` overrides both.
pub fn time_locale(
    language: Option<&str>,
    lc_all: Option<&str>,
    lc_time: Option<&str>,
    lc_messages: Option<&str>,
    lang: Option<&str>,
) -> Option<String> {
    fn set(value: Option<&str>) -> Option<&str> {
        value.filter(|v| !v.is_empty())
    }
    let time = set(lc_all).or(set(lc_time)).or(set(lang)).unwrap_or("C");
    let messages = set(lc_all)
        .or(set(lc_messages))
        .or(set(lang))
        .unwrap_or("C");
    if is_c(messages) {
        return None;
    }
    let interface = set(language)
        .and_then(|list| list.split(':').find(|entry| !entry.is_empty()))
        .unwrap_or(messages);
    let wanted = language_of(interface);
    if is_c(interface) || wanted.is_empty() || language_of(time) == wanted {
        return None;
    }
    let territory = interface
        .split(['.', '@'])
        .next()
        .and_then(|name| name.split_once('_'))
        .map(|(_, territory)| territory)
        .filter(|territory| !territory.is_empty())
        .map_or_else(|| wanted.to_uppercase(), str::to_string);
    Some(format!("{wanted}_{territory}.UTF-8"))
}

fn is_c(locale: &str) -> bool {
    matches!(locale.split('.').next(), Some("C" | "POSIX" | ""))
}

/// "de" of "de", "de_AT", "de_DE.UTF-8" or "de@euro".
fn language_of(locale: &str) -> &str {
    locale.split(['_', '.', '@']).next().unwrap_or_default()
}

/// What [`time_locale`] says for the environment the session gave the panel.
fn wanted_time_locale() -> Option<String> {
    let var = |name: &str| std::env::var(name).ok();
    time_locale(
        var("LANGUAGE").as_deref(),
        var("LC_ALL").as_deref(),
        var("LC_TIME").as_deref(),
        var("LC_MESSAGES").as_deref(),
        var("LANG").as_deref(),
    )
}

/// glibc's per-thread locales (`newlocale(3)`, `uselocale(3)`); the libc
/// is linked anyway, so no crate is needed for four functions.
mod thread_locale {
    use std::ffi::{c_char, c_int, c_void};

    pub type Locale = *mut c_void;
    /// glibc numbers the categories; time is number 2, so its mask bit is 4.
    pub const LC_TIME_MASK: c_int = 1 << 2;
    pub const LC_GLOBAL_LOCALE: Locale = -1isize as Locale;

    unsafe extern "C" {
        pub fn newlocale(mask: c_int, locale: *const c_char, base: Locale) -> Locale;
        pub fn duplocale(locale: Locale) -> Locale;
        pub fn uselocale(locale: Locale) -> Locale;
        pub fn freelocale(locale: Locale);
    }
}

/// Runs `create` with [`wanted_time_locale`] as the time locale of this
/// thread only; the process locale and every other thread stay as they
/// are. Without such a locale (not needed, or not installed) `create` runs
/// unchanged.
fn with_time_locale<T>(create: impl FnOnce() -> T) -> T {
    use thread_locale::*;
    let Some(name) = wanted_time_locale() else {
        return create();
    };
    let Ok(c_name) = CString::new(name.as_str()) else {
        return create();
    };
    // SAFETY: plain calls into glibc with valid arguments. `base` is a
    // fresh copy of the global locale; newlocale takes it over on success
    // and leaves it to the caller on failure. The new locale is in use only
    // on this thread, between the two uselocale calls, and freed after.
    unsafe {
        let base = duplocale(LC_GLOBAL_LOCALE);
        if base.is_null() {
            return create();
        }
        let locale = newlocale(LC_TIME_MASK, c_name.as_ptr(), base);
        if locale.is_null() {
            freelocale(base);
            eprintln!(
                "querencia-panel: Uhr: Zeitformat {name} nicht installiert, Kalender nach LC_TIME"
            );
            return create();
        }
        let previous = uselocale(locale);
        let created = create();
        uselocale(previous);
        freelocale(locale);
        eprintln!("querencia-panel: Uhr: Kalender mit Zeitformat {name}");
        created
    }
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

    fn en_us_with(language: &str) -> Option<String> {
        time_locale(Some(language), None, None, None, Some("en_US.UTF-8"))
    }

    #[test]
    fn the_querencia_session_gets_a_german_calendar() {
        // session/querencia/labwc/environment: LANG=en_US.UTF-8, LANGUAGE=de
        assert_eq!(en_us_with("de").as_deref(), Some("de_DE.UTF-8"));
        assert_eq!(en_us_with("de_AT:de").as_deref(), Some("de_AT.UTF-8"));
        assert_eq!(en_us_with(":fr_CH.UTF-8").as_deref(), Some("fr_CH.UTF-8"));
        assert_eq!(en_us_with("de@euro").as_deref(), Some("de_DE.UTF-8"));
    }

    #[test]
    fn a_matching_time_locale_stays() {
        assert_eq!(
            time_locale(Some("de"), None, None, None, Some("de_DE.UTF-8")),
            None
        );
        assert_eq!(
            time_locale(
                Some("de"),
                None,
                Some("de_AT.UTF-8"),
                None,
                Some("en_US.UTF-8")
            ),
            None
        );
        assert_eq!(en_us_with("en_GB"), None);
        assert_eq!(
            time_locale(None, None, None, None, Some("en_US.UTF-8")),
            None
        );
        assert_eq!(
            time_locale(Some(""), None, None, None, Some("de_DE.UTF-8")),
            None
        );
    }

    #[test]
    fn lc_all_and_c_follow_gettext() {
        // LC_ALL sets time and messages; LANGUAGE still picks the language.
        assert_eq!(
            time_locale(
                Some("de"),
                Some("en_US.UTF-8"),
                Some("de_DE.UTF-8"),
                None,
                None
            )
            .as_deref(),
            Some("de_DE.UTF-8")
        );
        // A C messages locale disables LANGUAGE: nothing to follow.
        assert_eq!(time_locale(Some("de"), Some("C"), None, None, None), None);
        assert_eq!(
            time_locale(Some("de"), None, None, Some("C.UTF-8"), None),
            None
        );
        assert_eq!(time_locale(Some("de"), None, None, None, None), None);
        // Without LANGUAGE the messages locale is the interface language.
        assert_eq!(
            time_locale(None, None, Some("en_US.UTF-8"), None, Some("de_DE.UTF-8")).as_deref(),
            Some("de_DE.UTF-8")
        );
        assert_eq!(
            time_locale(None, None, None, Some("de_CH.UTF-8"), Some("en_US.UTF-8")).as_deref(),
            Some("de_CH.UTF-8")
        );
    }
}
