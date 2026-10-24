//! What the audio system reports, as plain data, and the rules the applet
//! and the mixer follow: volume in percent, the icon level, the tooltip,
//! the mouse-wheel step. No GTK and no libpulse here, so all of it is
//! tested without an audio service.
//!
//! Volumes are kept as the raw PulseAudio values per channel
//! (`pa_volume_t`, [`VOLUME_NORM`] = 100 %), as the server reports them, so
//! changing the volume keeps the balance between the channels. The percent
//! of a device is that of its loudest channel, as in the GNOME 2 volume
//! control.

use crate::i18n::{gettext, gettext_f};

/// `PA_VOLUME_NORM`: 100 %, no amplification.
pub const VOLUME_NORM: u32 = 0x10000;
/// `PA_CHANNELS_MAX`.
pub const CHANNELS_MAX: usize = 32;
/// The mouse wheel over the applet changes the volume in steps of this
/// many percent.
pub const SCROLL_STEP: u32 = 5;
/// Highest volume the applet and the mixer set (no amplification, as the
/// GNOME 2 applet).
pub const MAX_PERCENT: u32 = 100;

/// Percent of the raw volume `raw`, rounded to the nearest whole percent.
pub fn percent(raw: u32) -> u32 {
    ((u64::from(raw) * 100 + u64::from(VOLUME_NORM) / 2) / u64::from(VOLUME_NORM)) as u32
}

/// The raw volume of `percent` percent, rounded to the nearest value.
pub fn raw_for_percent(percent: u32) -> u32 {
    ((u64::from(percent) * u64::from(VOLUME_NORM) + 50) / 100) as u32
}

/// The channel volumes `channels` changed so that the loudest channel has
/// `percent` percent; the others keep their ratio to it (balance). A
/// device whose channels are all at zero gets `percent` on every channel.
/// At least one channel, at most [`CHANNELS_MAX`].
pub fn scaled(channels: &[u32], percent: u32) -> Vec<u32> {
    let target = raw_for_percent(percent);
    let count = channels.len().clamp(1, CHANNELS_MAX);
    let max = channels.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return vec![target; count];
    }
    channels
        .iter()
        .take(count)
        .map(|&v| ((u64::from(v) * u64::from(target) + u64::from(max) / 2) / u64::from(max)) as u32)
        .collect()
}

/// Percent of the loudest of `channels` (0 for none).
pub fn channels_percent(channels: &[u32]) -> u32 {
    percent(channels.iter().copied().max().unwrap_or(0))
}

/// The volume after one notch of the mouse wheel, `up` or down, from
/// `current` percent: [`SCROLL_STEP`] more up to [`MAX_PERCENT`], or less
/// down to 0. A volume above 100 % (set elsewhere) is not raised further,
/// but goes down in steps.
pub fn scroll_target(current: u32, up: bool) -> u32 {
    if up {
        if current >= MAX_PERCENT {
            current
        } else {
            (current + SCROLL_STEP).min(MAX_PERCENT)
        }
    } else {
        current.saturating_sub(SCROLL_STEP)
    }
}

/// An output (sink) or input (source) device.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Device {
    pub index: u32,
    /// Stable name, e.g. `alsa_output.pci-0000_00_1f.3.analog-stereo`.
    pub name: String,
    /// Name for people, e.g. „Eingebautes Tonsystem Analog Stereo“.
    pub description: String,
    /// Raw volume per channel.
    pub volume: Vec<u32>,
    pub muted: bool,
    /// A source that only records what a sink plays (`*.monitor`). The
    /// mixer does not offer these as recording devices.
    pub monitor: bool,
}

impl Device {
    pub fn percent(&self) -> u32 {
        channels_percent(&self.volume)
    }

    /// The description, or the name if the description is empty.
    pub fn label(&self) -> &str {
        if self.description.trim().is_empty() {
            &self.name
        } else {
            &self.description
        }
    }
}

/// A stream of an application: playing (sink input) or recording (source
/// output).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stream {
    pub index: u32,
    /// `application.name`, else the stream name.
    pub application: String,
    /// `media.name`, may be empty.
    pub title: String,
    /// `application.icon_name`, if given.
    pub icon: Option<String>,
    /// Index of the sink or source it plays to or records from.
    pub device: u32,
    pub volume: Vec<u32>,
    pub muted: bool,
    /// The stream's volume can be changed (`has_volume` and
    /// `volume_writable`).
    pub adjustable: bool,
}

impl Stream {
    pub fn percent(&self) -> u32 {
        channels_percent(&self.volume)
    }

    /// „Anwendung: Titel“, or only the application if the title is empty or
    /// the same.
    pub fn label(&self) -> String {
        let title = self.title.trim();
        if title.is_empty() || title == self.application {
            self.application.clone()
        } else {
            format!("{}: {title}", self.application)
        }
    }
}

/// Whether the audio service can be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// Not yet tried.
    Connecting,
    /// Connected and the first complete state is in.
    Ready,
    /// No audio service (or no libpulse); the text says why, for the log.
    Unavailable(String),
}

/// Everything the applet and the mixer show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub connection: Connection,
    /// e.g. „PulseAudio (on PipeWire 1.6.2)“.
    pub server: String,
    pub default_sink: Option<String>,
    pub default_source: Option<String>,
    pub sinks: Vec<Device>,
    pub sources: Vec<Device>,
    pub playback: Vec<Stream>,
    pub recording: Vec<Stream>,
}

impl Default for State {
    fn default() -> State {
        State::unavailable("noch nicht verbunden")
    }
}

impl State {
    /// The state without an audio service.
    pub fn unavailable(reason: &str) -> State {
        State {
            connection: Connection::Unavailable(reason.to_string()),
            server: String::new(),
            default_sink: None,
            default_source: None,
            sinks: Vec::new(),
            sources: Vec::new(),
            playback: Vec::new(),
            recording: Vec::new(),
        }
    }

    pub fn ready(&self) -> bool {
        self.connection == Connection::Ready
    }

    /// The default output device: the one the server names, else the first
    /// one (a server may name a device that is gone).
    pub fn output(&self) -> Option<&Device> {
        pick(&self.sinks, self.default_sink.as_deref())
    }

    /// The input device the mixer shows: the server's default if it is a
    /// real input; if the server names a monitor (PipeWire without a
    /// microphone: `auto_null.monitor`) or nothing, the first real input;
    /// a monitor only if there is no real input at all.
    pub fn input(&self) -> Option<&Device> {
        let named = self
            .default_source
            .as_deref()
            .and_then(|name| self.sources.iter().find(|d| d.name == name));
        named
            .filter(|d| !d.monitor)
            .or_else(|| self.inputs().next())
            .or(named)
    }

    /// The recording devices the mixer offers: no monitors.
    pub fn inputs(&self) -> impl Iterator<Item = &Device> {
        self.sources.iter().filter(|d| !d.monitor)
    }
}

fn pick<'a>(devices: &'a [Device], name: Option<&str>) -> Option<&'a Device> {
    name.and_then(|name| devices.iter().find(|d| d.name == name))
        .or_else(|| devices.first())
}

/// Which of the four applet icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    Muted,
    Low,
    Medium,
    High,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Muted, Level::Low, Level::Medium, Level::High];

    /// The level for `percent` percent, muted or not, as the GNOME 2 applet:
    /// muted or 0 % the muted icon, then a third each.
    pub fn of(percent: u32, muted: bool) -> Level {
        match percent {
            _ if muted => Level::Muted,
            0 => Level::Muted,
            1..=33 => Level::Low,
            34..=66 => Level::Medium,
            _ => Level::High,
        }
    }

    /// Name for logs and file names.
    pub fn name(self) -> &'static str {
        match self {
            Level::Muted => "stumm",
            Level::Low => "leise",
            Level::Medium => "mittel",
            Level::High => "laut",
        }
    }
}

/// "Kein Audiodienst" in the mixer (`main.rs`), which is not translated yet;
/// the panel's tooltip comes from the catalog ([`look`]).
pub const NO_SERVICE: &str = "Kein Audiodienst";

/// What the panel applet shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Look {
    pub level: Level,
    pub tooltip: String,
    /// The controls work (service and output device present); otherwise the
    /// icon is greyed out.
    pub available: bool,
    /// Percent of the output device, 0 without one.
    pub percent: u32,
    pub muted: bool,
}

/// The applet's look for `state`.
pub fn look(state: &State) -> Look {
    let unavailable = |tooltip: &str| Look {
        level: Level::Muted,
        tooltip: tooltip.to_string(),
        available: false,
        percent: 0,
        muted: false,
    };
    match &state.connection {
        Connection::Unavailable(_) => unavailable(&gettext("No audio service")),
        Connection::Connecting => unavailable(&gettext("Connecting to the audio service…")),
        Connection::Ready => match state.output() {
            None => unavailable(&gettext("No output device")),
            Some(device) => {
                let percent = device.percent();
                let value = percent.to_string();
                let volume = if device.muted {
                    gettext_f("Volume: muted ({percent} %)", &[("percent", &value)])
                } else {
                    gettext_f("Volume: {percent} %", &[("percent", &value)])
                };
                let line = gettext_f("Device: {device}", &[("device", device.label())]);
                Look {
                    level: Level::of(percent, device.muted),
                    tooltip: format!("{volume}\n{line}"),
                    available: true,
                    percent,
                    muted: device.muted,
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, volume: &[u32], muted: bool) -> Device {
        Device {
            index: 0,
            name: name.into(),
            description: format!("Gerät {name}"),
            volume: volume.to_vec(),
            muted,
            monitor: name.ends_with(".monitor"),
        }
    }

    fn ready(sinks: Vec<Device>, default: Option<&str>) -> State {
        State {
            connection: Connection::Ready,
            sinks,
            default_sink: default.map(str::to_string),
            ..State::unavailable("")
        }
    }

    #[test]
    fn percent_and_raw_round_trip_for_every_whole_percent() {
        assert_eq!(percent(0), 0);
        assert_eq!(percent(VOLUME_NORM), 100);
        assert_eq!(percent(VOLUME_NORM / 2), 50);
        assert_eq!(raw_for_percent(100), VOLUME_NORM);
        assert_eq!(raw_for_percent(0), 0);
        for p in 0..=150 {
            assert_eq!(percent(raw_for_percent(p)), p, "{p}");
        }
    }

    #[test]
    fn scaling_keeps_the_balance_and_sets_the_loudest_channel() {
        let left_louder = [VOLUME_NORM, VOLUME_NORM / 2];
        let half = scaled(&left_louder, 50);
        assert_eq!(channels_percent(&half), 50);
        assert_eq!(half[0], raw_for_percent(50));
        assert_eq!(half[1], raw_for_percent(25));
        // all channels at zero: every channel gets the target
        assert_eq!(scaled(&[0, 0], 30), vec![raw_for_percent(30); 2]);
        // no channel reported: one channel
        assert_eq!(scaled(&[], 40), vec![raw_for_percent(40)]);
        // to zero and back up: the balance is lost (as in GNOME), both equal
        let zero = scaled(&left_louder, 0);
        assert_eq!(zero, vec![0, 0]);
        assert_eq!(scaled(&zero, 70), vec![raw_for_percent(70); 2]);
        // never more than PA_CHANNELS_MAX
        assert_eq!(scaled(&[1; 40], 10).len(), CHANNELS_MAX);
    }

    #[test]
    fn mouse_wheel_moves_in_five_percent_steps_within_0_and_100() {
        assert_eq!(scroll_target(50, true), 55);
        assert_eq!(scroll_target(50, false), 45);
        assert_eq!(scroll_target(97, true), 100);
        assert_eq!(scroll_target(100, true), 100);
        assert_eq!(scroll_target(3, false), 0);
        assert_eq!(scroll_target(0, false), 0);
        // above 100 % (set elsewhere): not further up, but down in steps
        assert_eq!(scroll_target(120, true), 120);
        assert_eq!(scroll_target(120, false), 115);
    }

    #[test]
    fn icon_levels_follow_the_gnome2_thirds() {
        assert_eq!(Level::of(0, false), Level::Muted);
        assert_eq!(Level::of(80, true), Level::Muted);
        assert_eq!(Level::of(1, false), Level::Low);
        assert_eq!(Level::of(33, false), Level::Low);
        assert_eq!(Level::of(34, false), Level::Medium);
        assert_eq!(Level::of(66, false), Level::Medium);
        assert_eq!(Level::of(67, false), Level::High);
        assert_eq!(Level::of(150, false), Level::High);
        let names: Vec<_> = Level::ALL.iter().map(|l| l.name()).collect();
        assert_eq!(names, ["stumm", "leise", "mittel", "laut"]);
    }

    #[test]
    fn without_a_service_the_applet_is_greyed_and_says_so() {
        let look = look(&State::unavailable("Verbindung abgelehnt"));
        assert!(!look.available);
        assert_eq!(look.tooltip, "No audio service");
        assert_eq!(look.level, Level::Muted);
        assert!(!super::look(&State::default()).available);
        let no_output = super::look(&ready(Vec::new(), None));
        assert!(!no_output.available);
        assert_eq!(no_output.tooltip, "No output device");
    }

    #[test]
    fn tooltip_has_percent_and_device() {
        let state = ready(
            vec![
                device("a", &[raw_for_percent(20)], false),
                device("b", &[raw_for_percent(45), raw_for_percent(40)], false),
            ],
            Some("b"),
        );
        let look = look(&state);
        assert!(look.available);
        assert_eq!(look.percent, 45);
        assert_eq!(look.level, Level::Medium);
        assert_eq!(look.tooltip, "Volume: 45 %\nDevice: Gerät b");
        let mut muted = state.clone();
        muted.sinks[1].muted = true;
        let look = super::look(&muted);
        assert_eq!(look.level, Level::Muted);
        assert!(look.muted);
        assert_eq!(look.tooltip, "Volume: muted (45 %)\nDevice: Gerät b");
    }

    #[test]
    fn default_device_falls_back_to_the_first_and_skips_monitors_for_input() {
        let state = ready(
            vec![device("a", &[1], false), device("b", &[2], false)],
            Some("weg"),
        );
        assert_eq!(state.output().map(|d| d.name.as_str()), Some("a"));
        let state = State {
            sources: vec![device("a.monitor", &[1], false), device("mic", &[1], false)],
            ..state
        };
        assert_eq!(state.input().map(|d| d.name.as_str()), Some("mic"));
        assert_eq!(state.inputs().count(), 1);
        // PipeWire without a microphone names the monitor: the real input
        // is shown, the monitor only if there is nothing else
        let named_monitor = State {
            default_source: Some("a.monitor".into()),
            ..state
        };
        assert_eq!(named_monitor.input().map(|d| d.name.as_str()), Some("mic"));
        let only_monitor = State {
            sources: vec![device("a.monitor", &[1], false)],
            ..named_monitor
        };
        assert_eq!(
            only_monitor.input().map(|d| d.name.as_str()),
            Some("a.monitor")
        );
    }

    #[test]
    fn labels_fall_back_sensibly() {
        let mut d = device("x", &[], false);
        assert_eq!(d.label(), "Gerät x");
        d.description = " ".into();
        assert_eq!(d.label(), "x");
        let mut s = Stream {
            application: "Firefox".into(),
            title: "Musik".into(),
            ..Stream::default()
        };
        assert_eq!(s.label(), "Firefox: Musik");
        s.title = "Firefox".into();
        assert_eq!(s.label(), "Firefox");
        s.title.clear();
        assert_eq!(s.label(), "Firefox");
    }
}
