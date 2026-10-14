//! The panel layout as plain data, independent of GTK: which panels exist,
//! at which screen edge, and which applet instances they show, in order, in
//! their start, center and end regions. `panel.rs` turns a layout into
//! windows; `layout_store.rs` reads and writes it as `panel.ini`.

/// Version of the stored layout format (`[Layout] Version` in `panel.ini`).
pub const FORMAT_VERSION: i32 = 1;
/// Panel height of the standard layout, in pixels.
pub const DEFAULT_HEIGHT: i32 = 32;
/// Heights a stored layout may ask for.
pub const HEIGHTS: std::ops::RangeInclusive<i32> = 16..=128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
}

impl Edge {
    pub fn as_str(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        }
    }

    pub fn parse(text: &str) -> Option<Edge> {
        match text {
            "top" => Some(Edge::Top),
            "bottom" => Some(Edge::Bottom),
            _ => None,
        }
    }
}

/// The three areas of a panel, from left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Start,
    Center,
    End,
}

impl Region {
    pub const ALL: [Region; 3] = [Region::Start, Region::Center, Region::End];

    fn index(self) -> usize {
        match self {
            Region::Start => 0,
            Region::Center => 1,
            Region::End => 2,
        }
    }
}

/// One applet on a panel: a stable instance ID and the applet type it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub id: String,
    pub applet: String,
}

impl Instance {
    pub fn new(id: &str, applet: &str) -> Instance {
        Instance {
            id: id.to_string(),
            applet: applet.to_string(),
        }
    }
}

/// One panel: its ID, screen edge, height and the ordered applet instances
/// of each region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelSpec {
    pub id: String,
    pub edge: Edge,
    pub height: i32,
    regions: [Vec<Instance>; 3],
}

impl PanelSpec {
    pub fn new(id: &str, edge: Edge, height: i32) -> PanelSpec {
        PanelSpec {
            id: id.to_string(),
            edge,
            height,
            regions: Default::default(),
        }
    }

    pub fn region(&self, region: Region) -> &[Instance] {
        &self.regions[region.index()]
    }

    /// Appends `instance` at the end of `region`.
    pub fn push(&mut self, region: Region, instance: Instance) {
        self.regions[region.index()].push(instance);
    }

    /// All instances, region by region, each in its order.
    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.regions.iter().flatten()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub panels: Vec<PanelSpec>,
}

/// Result of [`Layout::validated`].
#[derive(Debug)]
pub struct Validated {
    pub layout: Layout,
    /// One message per dropped or corrected entry.
    pub problems: Vec<String>,
    /// No panel was left, `layout` is the standard layout.
    pub fallback: bool,
}

impl Layout {
    /// The layout the panel had before it became configurable: Anwendungen,
    /// Orte, System and the clock on top; window list and workspace switcher
    /// at the bottom.
    pub fn standard() -> Layout {
        let mut top = PanelSpec::new("top", Edge::Top, DEFAULT_HEIGHT);
        top.push(
            Region::Start,
            Instance::new("applications-1", "applications"),
        );
        top.push(Region::Start, Instance::new("places-1", "places"));
        top.push(Region::Start, Instance::new("system-1", "system"));
        top.push(Region::End, Instance::new("clock-1", "clock"));

        let mut bottom = PanelSpec::new("bottom", Edge::Bottom, DEFAULT_HEIGHT);
        bottom.push(Region::Start, Instance::new("window-list-1", "window-list"));
        bottom.push(
            Region::End,
            Instance::new("workspace-switcher-1", "workspace-switcher"),
        );

        Layout {
            panels: vec![top, bottom],
        }
    }

    /// Keeps what is valid and reports the rest, so one bad entry never
    /// costs the whole layout. `applet` looks up an applet type ID and tells
    /// whether that type allows several instances (`None`: unknown type).
    ///
    /// Dropped are: panels with an invalid or repeated ID or a second panel
    /// on the same edge; instances with an invalid or repeated ID, an unknown
    /// type, or a second instance of a single-instance type. A height outside
    /// [`HEIGHTS`] becomes [`DEFAULT_HEIGHT`]. If no panel is left, the
    /// result is the standard layout.
    pub fn validated(self, applet: impl Fn(&str) -> Option<bool>) -> Validated {
        let mut problems = Vec::new();
        let mut panel_ids = Vec::new();
        let mut edges = Vec::new();
        let mut instance_ids = Vec::new();
        let mut single_types = Vec::new();
        let mut panels = Vec::new();

        for panel in self.panels {
            if !valid_id(&panel.id) {
                problems.push(format!("Panel „{}“: ungültige ID, übersprungen", panel.id));
                continue;
            }
            if panel_ids.contains(&panel.id) {
                problems.push(format!("Panel „{}“: doppelte ID, übersprungen", panel.id));
                continue;
            }
            if edges.contains(&panel.edge) {
                problems.push(format!(
                    "Panel „{}“: Kante {} schon belegt, übersprungen",
                    panel.id,
                    panel.edge.as_str()
                ));
                continue;
            }
            panel_ids.push(panel.id.clone());
            edges.push(panel.edge);

            let mut kept = PanelSpec::new(&panel.id, panel.edge, panel.height);
            if !HEIGHTS.contains(&panel.height) {
                problems.push(format!(
                    "Panel „{}“: Höhe {} außerhalb {}–{}, {} verwendet",
                    panel.id,
                    panel.height,
                    HEIGHTS.start(),
                    HEIGHTS.end(),
                    DEFAULT_HEIGHT
                ));
                kept.height = DEFAULT_HEIGHT;
            }
            for region in Region::ALL {
                for instance in panel.region(region) {
                    let problem = if !valid_id(&instance.id) {
                        Some("ungültige ID")
                    } else if instance_ids.contains(&instance.id) {
                        Some("doppelte ID")
                    } else {
                        match applet(&instance.applet) {
                            None => Some("unbekannter Applet-Typ"),
                            Some(false) if single_types.contains(&instance.applet) => {
                                Some("Applet-Typ nur einmal erlaubt")
                            }
                            Some(_) => None,
                        }
                    };
                    if let Some(problem) = problem {
                        problems.push(format!(
                            "Applet „{}“ ({}) in Panel „{}“: {problem}, übersprungen",
                            instance.id, instance.applet, panel.id
                        ));
                        continue;
                    }
                    if applet(&instance.applet) == Some(false) {
                        single_types.push(instance.applet.clone());
                    }
                    instance_ids.push(instance.id.clone());
                    kept.push(region, instance.clone());
                }
            }
            panels.push(kept);
        }

        if panels.is_empty() {
            problems.push("kein gültiges Panel, Standardlayout verwendet".to_string());
            return Validated {
                layout: Layout::standard(),
                problems,
                fallback: true,
            };
        }
        Validated {
            layout: Layout { panels },
            problems,
            fallback: false,
        }
    }
}

/// IDs of panels, applet types and instances: 1 to 64 characters from
/// `a-z`, `0-9` and `-`, starting with a letter or digit. Safe as key file
/// group names and list items.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real applet types.
    fn known(id: &str) -> Option<bool> {
        crate::applets::find(id).map(|applet| applet.multiple)
    }

    fn ids(panel: &PanelSpec, region: Region) -> Vec<&str> {
        panel.region(region).iter().map(|i| i.id.as_str()).collect()
    }

    fn types(panel: &PanelSpec, region: Region) -> Vec<&str> {
        panel
            .region(region)
            .iter()
            .map(|i| i.applet.as_str())
            .collect()
    }

    #[test]
    fn standard_layout_matches_the_former_fixed_panels() {
        let layout = Layout::standard();
        assert_eq!(layout.panels.len(), 2);
        let [top, bottom] = &layout.panels[..] else {
            unreachable!()
        };
        assert_eq!(
            (top.id.as_str(), top.edge, top.height),
            ("top", Edge::Top, 32)
        );
        assert_eq!(
            types(top, Region::Start),
            ["applications", "places", "system"]
        );
        assert!(top.region(Region::Center).is_empty());
        assert_eq!(types(top, Region::End), ["clock"]);
        assert_eq!(
            (bottom.id.as_str(), bottom.edge, bottom.height),
            ("bottom", Edge::Bottom, 32)
        );
        assert_eq!(types(bottom, Region::Start), ["window-list"]);
        assert!(bottom.region(Region::Center).is_empty());
        assert_eq!(types(bottom, Region::End), ["workspace-switcher"]);
    }

    #[test]
    fn standard_layout_is_valid_with_unique_stable_ids() {
        let standard = Layout::standard();
        let Validated {
            layout,
            problems,
            fallback,
        } = standard.clone().validated(known);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!fallback);
        assert_eq!(layout, standard);

        let all: Vec<&str> = layout
            .panels
            .iter()
            .flat_map(|p| p.instances())
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(
            all,
            [
                "applications-1",
                "places-1",
                "system-1",
                "clock-1",
                "window-list-1",
                "workspace-switcher-1"
            ]
        );
        assert_eq!(Layout::standard(), standard, "standard layout is stable");
    }

    #[test]
    fn order_within_a_region_is_kept() {
        let mut panel = PanelSpec::new("top", Edge::Top, 32);
        for (id, applet) in [("c-1", "clock"), ("s-1", "system"), ("a-1", "applications")] {
            panel.push(Region::Start, Instance::new(id, applet));
        }
        let result = Layout {
            panels: vec![panel],
        }
        .validated(known);
        assert!(result.problems.is_empty());
        assert_eq!(
            ids(&result.layout.panels[0], Region::Start),
            ["c-1", "s-1", "a-1"]
        );
    }

    #[test]
    fn invalid_entries_are_dropped_and_the_rest_kept() {
        let mut top = PanelSpec::new("top", Edge::Top, 32);
        top.push(Region::Start, Instance::new("a-1", "applications"));
        top.push(Region::Start, Instance::new("x-1", "gibt-es-nicht"));
        top.push(Region::Start, Instance::new("a-1", "places"));
        top.push(Region::Start, Instance::new("Gross", "places"));
        top.push(Region::End, Instance::new("w-1", "window-list"));
        top.push(Region::End, Instance::new("w-2", "window-list"));
        top.push(Region::End, Instance::new("c-1", "clock"));
        top.push(Region::End, Instance::new("c-2", "clock"));
        let same_edge = PanelSpec::new("oben", Edge::Top, 32);
        let mut bottom = PanelSpec::new("bottom", Edge::Bottom, 500);
        bottom.push(Region::Center, Instance::new("p-1", "places"));
        let repeated = PanelSpec::new("bottom", Edge::Bottom, 32);
        let bad_id = PanelSpec::new("", Edge::Bottom, 32);

        let Validated {
            layout,
            problems,
            fallback,
        } = Layout {
            panels: vec![top, same_edge, bottom, repeated, bad_id],
        }
        .validated(known);
        assert!(!fallback);

        assert_eq!(layout.panels.len(), 2);
        let top = &layout.panels[0];
        assert_eq!(ids(top, Region::Start), ["a-1"]);
        assert_eq!(ids(top, Region::End), ["w-1", "c-1", "c-2"]);
        let bottom = &layout.panels[1];
        assert_eq!(bottom.height, DEFAULT_HEIGHT);
        assert_eq!(ids(bottom, Region::Center), ["p-1"]);
        // unknown type, repeated ID, invalid ID, second window list, panel on
        // a taken edge, height, repeated panel ID, invalid panel ID
        assert_eq!(problems.len(), 8, "{problems:#?}");
    }

    #[test]
    fn nothing_usable_falls_back_to_the_standard_layout() {
        let empty = Layout { panels: vec![] }.validated(known);
        assert!(empty.fallback);
        assert_eq!(empty.layout, Layout::standard());
        assert_eq!(empty.problems.len(), 1);

        let broken = Layout {
            panels: vec![PanelSpec::new("-x", Edge::Top, 32)],
        }
        .validated(known);
        assert!(broken.fallback);
        assert_eq!(broken.layout, Layout::standard());
        assert_eq!(broken.problems.len(), 2);
    }

    #[test]
    fn empty_panels_stay_valid() {
        let result = Layout {
            panels: vec![PanelSpec::new("leer", Edge::Bottom, 24)],
        }
        .validated(known);
        assert!(result.problems.is_empty() && !result.fallback);
        assert_eq!(result.layout.panels[0].instances().count(), 0);
        assert_eq!(result.layout.panels[0].height, 24);
    }

    #[test]
    fn id_rules() {
        for good in ["top", "clock-1", "a", "9", "workspace-switcher-12"] {
            assert!(valid_id(good), "{good}");
        }
        let long = "a".repeat(65);
        for bad in [
            "",
            "-a",
            "Top",
            "a b",
            "a_b",
            "ä",
            "a;b",
            "[x]",
            long.as_str(),
        ] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert!(valid_id(&"a".repeat(64)));
    }

    #[test]
    fn edges_round_trip() {
        for edge in [Edge::Top, Edge::Bottom] {
            assert_eq!(Edge::parse(edge.as_str()), Some(edge));
        }
        assert_eq!(Edge::parse("left"), None);
        assert_eq!(Edge::parse("Top"), None);
    }
}
