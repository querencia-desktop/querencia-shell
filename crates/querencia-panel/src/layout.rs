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

    /// The region a click at `x` on a panel bar `width` pixels wide adds
    /// to: the left third the start, the middle third the center, the right
    /// third the end. Points left of the bar (its margin) count as start,
    /// points right of it as end; a bar without width gives the start.
    pub fn at(x: f64, width: f64) -> Region {
        if width.is_nan() || width <= 0.0 || x.is_nan() || x < width / 3.0 {
            Region::Start
        } else if x < width * 2.0 / 3.0 {
            Region::Center
        } else {
            Region::End
        }
    }

    /// Where in the region a new applet goes: next to the free space the
    /// click was on. The start and center regions grow to the right, so a
    /// new applet comes last; the end region grows to the left, so it comes
    /// first. `len` is the number of applets the region holds.
    pub fn insertion_index(self, len: usize) -> usize {
        match self {
            Region::Start | Region::Center => len,
            Region::End => 0,
        }
    }

    /// Name of the region in messages.
    pub fn label(self) -> &'static str {
        match self {
            Region::Start => "links",
            Region::Center => "Mitte",
            Region::End => "rechts",
        }
    }

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

    /// Inserts `instance` at `index` of `region`; an index past the end
    /// appends.
    pub fn insert(&mut self, region: Region, index: usize, instance: Instance) {
        let items = &mut self.regions[region.index()];
        let index = index.min(items.len());
        items.insert(index, instance);
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

/// Why [`Layout::add_applet`] refused to add an applet. The layout is
/// unchanged in every case.
#[derive(Debug, PartialEq, Eq)]
pub enum AddError {
    /// No panel with this ID.
    UnknownPanel(String),
    /// No applet type with this ID.
    UnknownApplet(String),
    /// The type allows one instance only, and the layout has it already.
    SingleInstance(String),
    /// Every candidate instance ID is taken (needs thousands of instances).
    NoFreeId(String),
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::UnknownPanel(id) => write!(f, "Panel „{id}“ gibt es nicht"),
            AddError::UnknownApplet(id) => write!(f, "Applet-Typ „{id}“ ist unbekannt"),
            AddError::SingleInstance(id) => write!(
                f,
                "Applet-Typ „{id}“ ist nur einmal erlaubt und schon vorhanden"
            ),
            AddError::NoFreeId(id) => write!(f, "keine freie Instanz-ID für „{id}“"),
        }
    }
}

/// An applet added by [`Layout::add_applet`]: the new instance and where it
/// went.
#[derive(Debug, PartialEq, Eq)]
pub struct Added {
    pub instance: Instance,
    pub region: Region,
    /// Position within the region, counted from its first applet.
    pub index: usize,
}

/// Upper bound for the number in generated instance IDs.
const MAX_INSTANCE_NUMBER: u32 = 9999;

impl Layout {
    /// Whether [`Layout::add_applet`] would accept one more instance of the
    /// applet type `applet_id`. `applet` is the lookup of
    /// [`Layout::validated`].
    pub fn can_add(&self, applet_id: &str, applet: impl Fn(&str) -> Option<bool>) -> bool {
        match applet(applet_id) {
            None => false,
            Some(true) => true,
            Some(false) => !self.has_applet(applet_id),
        }
    }

    /// Adds a new instance of the applet type `applet_id` to `region` of the
    /// panel `panel_id`, next to the free space (see
    /// [`Region::insertion_index`]). The instance gets the ID
    /// `<type>-<n>` with the smallest `n` from 1 that no instance of the
    /// layout uses. Follows the rules of [`Layout::validated`], so the result
    /// stays a valid layout; on error nothing changes.
    pub fn add_applet(
        &mut self,
        panel_id: &str,
        applet_id: &str,
        region: Region,
        applet: impl Fn(&str) -> Option<bool>,
    ) -> Result<Added, AddError> {
        let multiple = match applet(applet_id) {
            Some(multiple) if valid_id(applet_id) => multiple,
            _ => return Err(AddError::UnknownApplet(applet_id.to_string())),
        };
        if !self.panels.iter().any(|p| p.id == panel_id) {
            return Err(AddError::UnknownPanel(panel_id.to_string()));
        }
        if !multiple && self.has_applet(applet_id) {
            return Err(AddError::SingleInstance(applet_id.to_string()));
        }
        let id = self
            .free_instance_id(applet_id)
            .ok_or_else(|| AddError::NoFreeId(applet_id.to_string()))?;
        let panel = self
            .panels
            .iter_mut()
            .find(|p| p.id == panel_id)
            .expect("checked above");
        let index = region.insertion_index(panel.region(region).len());
        let instance = Instance::new(&id, applet_id);
        panel.insert(region, index, instance.clone());
        Ok(Added {
            instance,
            region,
            index,
        })
    }

    fn has_applet(&self, applet_id: &str) -> bool {
        self.panels
            .iter()
            .flat_map(|p| p.instances())
            .any(|i| i.applet == applet_id)
    }

    /// `<applet_id>-<n>` with the smallest free `n`, if it is a valid ID.
    fn free_instance_id(&self, applet_id: &str) -> Option<String> {
        let taken: Vec<&str> = self
            .panels
            .iter()
            .flat_map(|p| p.instances())
            .map(|i| i.id.as_str())
            .collect();
        (1..=MAX_INSTANCE_NUMBER)
            .map(|n| format!("{applet_id}-{n}"))
            .find(|id| !taken.contains(&id.as_str()))
            .filter(|id| valid_id(id))
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

    fn panel<'a>(layout: &'a Layout, id: &str) -> &'a PanelSpec {
        layout.panels.iter().find(|p| p.id == id).unwrap()
    }

    #[test]
    fn adding_appends_to_the_start_region_of_the_chosen_panel() {
        let mut layout = Layout::standard();
        let added = layout
            .add_applet("top", "clock", Region::Start, known)
            .expect("clock may be added");
        assert_eq!(added.instance, Instance::new("clock-2", "clock"));
        assert_eq!((added.region, added.index), (Region::Start, 3));
        assert_eq!(
            ids(panel(&layout, "top"), Region::Start),
            ["applications-1", "places-1", "system-1", "clock-2"]
        );
        // the other panel and the other regions are untouched
        assert_eq!(panel(&layout, "bottom"), &Layout::standard().panels[1]);
        assert_eq!(ids(panel(&layout, "top"), Region::End), ["clock-1"]);
    }

    #[test]
    fn adding_to_the_bottom_panel_changes_only_the_bottom_panel() {
        let mut layout = Layout::standard();
        let added = layout
            .add_applet("bottom", "places", Region::Center, known)
            .unwrap();
        assert_eq!(added.instance, Instance::new("places-2", "places"));
        assert_eq!((added.region, added.index), (Region::Center, 0));
        assert_eq!(ids(panel(&layout, "bottom"), Region::Center), ["places-2"]);
        assert_eq!(panel(&layout, "top"), &Layout::standard().panels[0]);
    }

    #[test]
    fn adding_to_the_end_region_goes_first_next_to_the_free_space() {
        let mut layout = Layout::standard();
        let added = layout
            .add_applet("top", "system", Region::End, known)
            .unwrap();
        assert_eq!((added.region, added.index), (Region::End, 0));
        assert_eq!(
            ids(panel(&layout, "top"), Region::End),
            ["system-2", "clock-1"]
        );
    }

    #[test]
    fn repeated_adding_gives_new_unique_ids_and_stays_valid() {
        let mut layout = Layout::standard();
        let mut new_ids = Vec::new();
        for _ in 0..3 {
            new_ids.push(
                layout
                    .add_applet("bottom", "clock", Region::Start, known)
                    .unwrap()
                    .instance
                    .id,
            );
        }
        assert_eq!(new_ids, ["clock-2", "clock-3", "clock-4"]);
        assert_eq!(
            ids(panel(&layout, "bottom"), Region::Start),
            ["window-list-1", "clock-2", "clock-3", "clock-4"]
        );
        let validated = layout.clone().validated(known);
        assert!(validated.problems.is_empty(), "{:?}", validated.problems);
        assert_eq!(validated.layout, layout);
    }

    #[test]
    fn new_ids_fill_the_first_gap_and_ignore_other_types() {
        let mut top = PanelSpec::new("top", Edge::Top, 32);
        top.push(Region::Start, Instance::new("clock-1", "clock"));
        top.push(Region::Start, Instance::new("clock-3", "clock"));
        // an instance of another type may carry a name that looks like ours
        top.push(Region::End, Instance::new("clock-2", "places"));
        let mut layout = Layout { panels: vec![top] };
        let added = layout
            .add_applet("top", "clock", Region::Start, known)
            .unwrap();
        assert_eq!(added.instance.id, "clock-4");
    }

    #[test]
    fn standard_elements_are_kept_in_order_after_adding() {
        let mut layout = Layout::standard();
        for (panel_id, applet, region) in [
            ("top", "applications", Region::Start),
            ("top", "clock", Region::Center),
            ("bottom", "system", Region::End),
        ] {
            layout.add_applet(panel_id, applet, region, known).unwrap();
        }
        let kept: Vec<&str> = layout
            .panels
            .iter()
            .flat_map(|p| p.instances())
            .map(|i| i.id.as_str())
            .filter(|id| id.ends_with("-1"))
            .collect();
        assert_eq!(
            kept,
            [
                "applications-1",
                "places-1",
                "system-1",
                "clock-1",
                "window-list-1",
                "workspace-switcher-1"
            ]
        );
    }

    #[test]
    fn unknown_applet_types_are_refused_and_nothing_changes() {
        let mut layout = Layout::standard();
        for bad in ["wetter", "", "Clock", "clock;places"] {
            assert_eq!(
                layout.add_applet("top", bad, Region::Start, known),
                Err(AddError::UnknownApplet(bad.to_string())),
                "{bad}"
            );
        }
        assert_eq!(layout, Layout::standard());
    }

    #[test]
    fn unknown_panels_are_refused_and_nothing_changes() {
        let mut layout = Layout::standard();
        assert_eq!(
            layout.add_applet("links", "clock", Region::Start, known),
            Err(AddError::UnknownPanel("links".to_string()))
        );
        assert_eq!(layout, Layout::standard());
    }

    #[test]
    fn single_instance_types_are_added_once_only() {
        // the standard layout already shows both single types
        let mut layout = Layout::standard();
        for applet in ["window-list", "workspace-switcher"] {
            assert!(!layout.can_add(applet, known), "{applet}");
            assert_eq!(
                layout.add_applet("top", applet, Region::Center, known),
                Err(AddError::SingleInstance(applet.to_string()))
            );
        }
        assert_eq!(layout, Layout::standard());

        // a layout without a window list may get exactly one, on any panel
        let mut layout = Layout {
            panels: vec![PanelSpec::new("top", Edge::Top, 32)],
        };
        assert!(layout.can_add("window-list", known));
        let added = layout
            .add_applet("top", "window-list", Region::Center, known)
            .unwrap();
        assert_eq!(added.instance.id, "window-list-1");
        assert!(!layout.can_add("window-list", known));
        assert_eq!(
            layout.add_applet("top", "window-list", Region::Start, known),
            Err(AddError::SingleInstance("window-list".to_string()))
        );
    }

    #[test]
    fn can_add_follows_the_registry() {
        let layout = Layout::standard();
        for applet in ["applications", "places", "system", "clock"] {
            assert!(layout.can_add(applet, known), "{applet}");
        }
        assert!(!layout.can_add("wetter", known));
    }

    #[test]
    fn add_errors_read_as_german_messages() {
        assert_eq!(
            AddError::UnknownPanel("x".into()).to_string(),
            "Panel „x“ gibt es nicht"
        );
        assert_eq!(
            AddError::UnknownApplet("y".into()).to_string(),
            "Applet-Typ „y“ ist unbekannt"
        );
        assert_eq!(
            AddError::SingleInstance("window-list".into()).to_string(),
            "Applet-Typ „window-list“ ist nur einmal erlaubt und schon vorhanden"
        );
        assert_eq!(
            AddError::NoFreeId("clock".into()).to_string(),
            "keine freie Instanz-ID für „clock“"
        );
    }

    #[test]
    fn insert_clamps_the_index() {
        let mut panel = PanelSpec::new("top", Edge::Top, 32);
        panel.insert(Region::End, 5, Instance::new("b", "clock"));
        panel.insert(Region::End, 0, Instance::new("a", "clock"));
        panel.insert(Region::End, 1, Instance::new("m", "clock"));
        assert_eq!(ids(&panel, Region::End), ["a", "m", "b"]);
    }

    #[test]
    fn click_position_picks_the_region_by_thirds() {
        assert_eq!(Region::at(0.0, 900.0), Region::Start);
        assert_eq!(Region::at(299.0, 900.0), Region::Start);
        assert_eq!(Region::at(300.0, 900.0), Region::Center);
        assert_eq!(Region::at(599.0, 900.0), Region::Center);
        assert_eq!(Region::at(600.0, 900.0), Region::End);
        assert_eq!(Region::at(899.0, 900.0), Region::End);
        // outside the bar (its side margins) and degenerate widths
        assert_eq!(Region::at(-5.0, 900.0), Region::Start);
        assert_eq!(Region::at(950.0, 900.0), Region::End);
        assert_eq!(Region::at(10.0, 0.0), Region::Start);
        assert_eq!(Region::at(f64::NAN, 900.0), Region::Start);
    }

    #[test]
    fn region_names_for_messages() {
        assert_eq!(Region::ALL.map(Region::label), ["links", "Mitte", "rechts"]);
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
