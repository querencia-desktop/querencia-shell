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

/// Applet types placed right of a neighbour type when added to the end
/// region (see [`Region::insertion_index_for`]): (type, neighbour).
pub const RIGHT_OF: &[(&str, &str)] = &[("network", "volume")];

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

    /// Like [`Region::insertion_index`] for a new applet of type
    /// `applet_id` among `items`: in the end region a type listed in
    /// [`RIGHT_OF`] goes right of the last applet of its neighbour type if
    /// the region has one (G2-M: the network applet right of the volume
    /// applet, the order of the GNOME 2 notification area).
    pub fn insertion_index_for(self, items: &[Instance], applet_id: &str) -> usize {
        if self == Region::End
            && let Some((_, left)) = RIGHT_OF.iter().find(|(kind, _)| *kind == applet_id)
            && let Some(position) = items.iter().rposition(|i| i.applet == *left)
        {
            return position + 1;
        }
        self.insertion_index(items.len())
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

    /// The region left of this one.
    pub fn before(self) -> Option<Region> {
        match self {
            Region::Start => None,
            Region::Center => Some(Region::Start),
            Region::End => Some(Region::Center),
        }
    }

    /// The region right of this one.
    pub fn after(self) -> Option<Region> {
        match self {
            Region::Start => Some(Region::Center),
            Region::Center => Some(Region::End),
            Region::End => None,
        }
    }
}

/// How an applet moves within its panel (`Layout::move_applet`), as with
/// „Nach links“, „Nach rechts“, „An den linken Rand“, „An den rechten
/// Rand“, or to a place it was dragged to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    StartEdge,
    EndEdge,
    /// To `index` of the region, counted without the moving applet.
    To(Region, usize),
}

/// Applet type of an application launcher (HMP-11). It is the only type
/// whose instances carry a setting of their own: the desktop ID of the
/// application they start ([`Instance::app`]).
pub const LAUNCHER: &str = "launcher";

/// One applet on a panel: a stable instance ID and the applet type it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub id: String,
    pub applet: String,
    /// Desktop ID (e.g. `firefox_firefox.desktop`) of the application a
    /// [`LAUNCHER`] starts; `None` for every other applet type.
    pub app: Option<String>,
}

impl Instance {
    pub fn new(id: &str, applet: &str) -> Instance {
        Instance {
            id: id.to_string(),
            applet: applet.to_string(),
            app: None,
        }
    }

    /// A [`LAUNCHER`] instance for the application with desktop ID `app`.
    #[cfg(test)]
    pub fn launcher(id: &str, app: &str) -> Instance {
        Instance {
            app: Some(app.to_string()),
            ..Instance::new(id, LAUNCHER)
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

    /// Region and index of the instance `instance_id`, if the panel has it.
    pub fn position(&self, instance_id: &str) -> Option<(Region, usize)> {
        Region::ALL.into_iter().find_map(|region| {
            self.region(region)
                .iter()
                .position(|i| i.id == instance_id)
                .map(|index| (region, index))
        })
    }

    /// Where `how` moves the instance at `from` (region, index after it was
    /// taken out of its region), or `None` if it stays where it is.
    ///
    /// The applets of a panel stand in one row: start region, center
    /// region, end region. One step left or right swaps with the
    /// neighbour; at the border of a region the applet changes into the
    /// neighbouring region (last of the one before, first of the one
    /// after), also into an empty center. The edges are the first place of
    /// the start region and the last of the end region.
    pub fn move_target(&self, from: (Region, usize), how: Move) -> Option<(Region, usize)> {
        let (region, index) = from;
        let len = |r: Region| self.region(r).len();
        if index >= len(region) {
            return None;
        }
        // lengths with the moving applet taken out
        let without = |r: Region| len(r) - usize::from(r == region);
        let target = match how {
            Move::Left if index > 0 => (region, index - 1),
            Move::Left => (region.before()?, without(region.before()?)),
            Move::Right if index + 1 < len(region) => (region, index + 1),
            Move::Right => (region.after()?, 0),
            Move::StartEdge => (Region::Start, 0),
            Move::EndEdge => (Region::End, without(Region::End)),
            Move::To(region, index) => (region, index.min(without(region))),
        };
        (target != from).then_some(target)
    }

    /// Removes the instance `instance_id` and returns it; `None` (nothing
    /// changed) if the panel has no such instance.
    fn remove(&mut self, instance_id: &str) -> Option<Instance> {
        self.regions.iter_mut().find_map(|items| {
            let index = items.iter().position(|i| i.id == instance_id)?;
            Some(items.remove(index))
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    pub panels: Vec<PanelSpec>,
    /// Settings of the window list (G2-AK), the same for every window list.
    pub window_list: WindowListSettings,
}

/// `[window-list]` in `panel.ini` (G2-AK, B11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowListSettings {
    /// `alle_arbeitsflaechen`: the windows of all workspaces (`ja`) or, as
    /// in GNOME 2, only those of the current one (`nein`). `None`: not in
    /// the file, the standard [`WindowListSettings::DEFAULT_ALL_WORKSPACES`];
    /// saving then leaves it out, so the file keeps following the standard.
    pub all_workspaces: Option<bool>,
}

impl WindowListSettings {
    /// GNOME 2's window list showed the current workspace only.
    pub const DEFAULT_ALL_WORKSPACES: bool = false;

    pub fn all_workspaces(self) -> bool {
        self.all_workspaces.unwrap_or(Self::DEFAULT_ALL_WORKSPACES)
    }
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
            ..Default::default()
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
                    } else if instance.applet == LAUNCHER
                        && !instance.app.as_deref().is_some_and(valid_desktop_id)
                    {
                        Some("Anwendung fehlt oder ungültig")
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
                    let mut instance = instance.clone();
                    if instance.applet != LAUNCHER && instance.app.take().is_some() {
                        problems.push(format!(
                            "Applet „{}“ ({}) in Panel „{}“: DesktopId gilt nur für \
                             Anwendungsstarter, ignoriert",
                            instance.id, instance.applet, panel.id
                        ));
                    }
                    kept.push(region, instance);
                }
            }
            panels.push(kept);
        }

        if panels.is_empty() {
            problems.push("kein gültiges Panel, Standardlayout verwendet".to_string());
            return Validated {
                layout: Layout {
                    window_list: self.window_list,
                    ..Layout::standard()
                },
                problems,
                fallback: true,
            };
        }
        Validated {
            layout: Layout {
                panels,
                window_list: self.window_list,
            },
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
    /// A launcher needs the valid desktop ID of an application, every other
    /// type none (the text given).
    Application(String),
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
            AddError::Application(app) if app.is_empty() => {
                write!(f, "ein Anwendungsstarter braucht eine Anwendung")
            }
            AddError::Application(app) => {
                write!(f, "„{app}“ ist keine gültige Desktop-ID für einen Starter")
            }
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

/// An applet moved by [`Layout::move_applet`]: from where to where.
#[derive(Debug, PartialEq, Eq)]
pub struct Moved {
    pub instance: Instance,
    pub panel: String,
    pub from: (Region, usize),
    pub to: (Region, usize),
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
        self.add_instance(panel_id, applet_id, None, region, applet)
    }

    /// Adds a new [`LAUNCHER`] for the application with the desktop ID
    /// `app`, like [`Layout::add_applet`]. Several launchers, even for the
    /// same application, are allowed; each gets its own instance ID
    /// `launcher-<n>`.
    pub fn add_launcher(
        &mut self,
        panel_id: &str,
        app: &str,
        region: Region,
        applet: impl Fn(&str) -> Option<bool>,
    ) -> Result<Added, AddError> {
        self.add_instance(panel_id, LAUNCHER, Some(app), region, applet)
    }

    fn add_instance(
        &mut self,
        panel_id: &str,
        applet_id: &str,
        app: Option<&str>,
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
        match (applet_id == LAUNCHER, app) {
            (true, Some(app)) if valid_desktop_id(app) => {}
            (true, app) | (false, app @ Some(_)) => {
                return Err(AddError::Application(app.unwrap_or_default().to_string()));
            }
            (false, None) => {}
        }
        let id = self
            .free_instance_id(applet_id)
            .ok_or_else(|| AddError::NoFreeId(applet_id.to_string()))?;
        let panel = self
            .panels
            .iter_mut()
            .find(|p| p.id == panel_id)
            .expect("checked above");
        let index = region.insertion_index_for(panel.region(region), applet_id);
        let instance = Instance {
            app: app.map(str::to_string),
            ..Instance::new(&id, applet_id)
        };
        panel.insert(region, index, instance.clone());
        Ok(Added {
            instance,
            region,
            index,
        })
    }

    /// Removes the applet instance `instance_id` from whichever panel holds
    /// it and returns the removed instance; `None` if there is no such
    /// instance, and the layout is unchanged. Its ID becomes free again.
    pub fn remove_applet(&mut self, instance_id: &str) -> Option<Instance> {
        self.panels
            .iter_mut()
            .find_map(|panel| panel.remove(instance_id))
    }

    /// Moves the applet instance `instance_id` within its panel as `how`
    /// says ([`PanelSpec::move_target`]). `None`: no such instance, or it
    /// is already there (nothing changed).
    pub fn move_applet(&mut self, instance_id: &str, how: Move) -> Option<Moved> {
        let panel = self
            .panels
            .iter_mut()
            .find(|p| p.position(instance_id).is_some())?;
        let from = panel.position(instance_id)?;
        let to = panel.move_target(from, how)?;
        let instance = panel.remove(instance_id)?;
        panel.insert(to.0, to.1, instance.clone());
        Some(Moved {
            instance,
            panel: panel.id.clone(),
            from,
            to,
        })
    }

    /// Whether any panel has an instance of the applet type `applet_id`.
    pub fn has_applet(&self, applet_id: &str) -> bool {
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

/// Desktop IDs a launcher may store: what GIO accepts as the ID of an
/// installed desktop entry, ending in `.desktop`, without `/` (a path is no
/// ID), without `;` (the list separator of the key file) and without
/// control characters, at most 255 bytes.
pub fn valid_desktop_id(id: &str) -> bool {
    id.len() > ".desktop".len()
        && id.len() <= 255
        && id.ends_with(".desktop")
        && !id.starts_with('.')
        && !id.chars().any(|c| c == '/' || c == ';' || c.is_control())
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

    fn top_with(start: &[&str], center: &[&str], end: &[&str]) -> Layout {
        let mut top = PanelSpec::new("top", Edge::Top, DEFAULT_HEIGHT);
        for (region, list) in [
            (Region::Start, start),
            (Region::Center, center),
            (Region::End, end),
        ] {
            for id in list {
                top.push(
                    region,
                    Instance::new(id, id.trim_end_matches(char::is_numeric)),
                );
            }
        }
        Layout {
            panels: vec![top],
            ..Default::default()
        }
    }

    fn rows(layout: &Layout) -> [Vec<&str>; 3] {
        Region::ALL.map(|r| ids(&layout.panels[0], r))
    }

    #[test]
    fn applets_step_left_and_right_through_the_regions() {
        let mut layout = top_with(&["a1", "b1"], &[], &["c1", "d1"]);
        // within a region: swap
        let moved = layout.move_applet("b1", Move::Left).unwrap();
        assert_eq!(
            (moved.from, moved.to),
            ((Region::Start, 1), (Region::Start, 0))
        );
        assert_eq!(rows(&layout), [vec!["b1", "a1"], vec![], vec!["c1", "d1"]]);
        // already at the left edge: nothing
        assert!(layout.move_applet("b1", Move::Left).is_none());
        assert!(layout.move_applet("b1", Move::StartEdge).is_none());
        // first of the end region, left: into the empty center
        layout.move_applet("c1", Move::Left).unwrap();
        assert_eq!(rows(&layout), [vec!["b1", "a1"], vec!["c1"], vec!["d1"]]);
        // and on into the start region, last
        layout.move_applet("c1", Move::Left).unwrap();
        assert_eq!(rows(&layout), [vec!["b1", "a1", "c1"], vec![], vec!["d1"]]);
        // right again: center, then first of the end region
        layout.move_applet("c1", Move::Right).unwrap();
        assert_eq!(rows(&layout), [vec!["b1", "a1"], vec!["c1"], vec!["d1"]]);
        layout.move_applet("c1", Move::Right).unwrap();
        assert_eq!(rows(&layout), [vec!["b1", "a1"], vec![], vec!["c1", "d1"]]);
        layout.move_applet("c1", Move::Right).unwrap();
        assert_eq!(rows(&layout), [vec!["b1", "a1"], vec![], vec!["d1", "c1"]]);
        assert!(
            layout.move_applet("c1", Move::Right).is_none(),
            "rechter Rand"
        );
        assert!(layout.move_applet("c1", Move::EndEdge).is_none());
        // edges
        layout.move_applet("c1", Move::StartEdge).unwrap();
        assert_eq!(rows(&layout), [vec!["c1", "b1", "a1"], vec![], vec!["d1"]]);
        layout.move_applet("b1", Move::EndEdge).unwrap();
        assert_eq!(rows(&layout), [vec!["c1", "a1"], vec![], vec!["d1", "b1"]]);
        // to a dragged place, index counted without the applet
        layout.move_applet("a1", Move::To(Region::End, 1)).unwrap();
        assert_eq!(rows(&layout), [vec!["c1"], vec![], vec!["d1", "a1", "b1"]]);
        layout
            .move_applet("a1", Move::To(Region::Center, 7))
            .unwrap();
        assert_eq!(rows(&layout), [vec!["c1"], vec!["a1"], vec!["d1", "b1"]]);
        assert!(
            layout
                .move_applet("a1", Move::To(Region::Center, 0))
                .is_none()
        );
        assert!(layout.move_applet("fehlt-1", Move::Left).is_none());
        // the moved layout stays valid
        let checked = layout.clone().validated(|_| Some(true));
        assert!(checked.problems.is_empty() && !checked.fallback);
    }

    #[test]
    fn region_neighbours() {
        assert_eq!(Region::Start.before(), None);
        assert_eq!(Region::Center.before(), Some(Region::Start));
        assert_eq!(Region::End.after(), None);
        assert_eq!(Region::Center.after(), Some(Region::End));
    }

    #[test]
    fn network_goes_right_of_volume_in_the_end_region_only() {
        let mut layout = Layout::standard();
        layout
            .add_applet("top", "volume", Region::End, known)
            .unwrap();
        assert_eq!(types(&layout.panels[0], Region::End), ["volume", "clock"]);
        let added = layout
            .add_applet("top", "network", Region::End, known)
            .unwrap();
        assert_eq!(added.index, 1);
        assert_eq!(
            types(&layout.panels[0], Region::End),
            ["volume", "network", "clock"]
        );
        // without a volume applet: first, as every other applet
        let mut layout = Layout::standard();
        layout
            .add_applet("top", "network", Region::End, known)
            .unwrap();
        assert_eq!(types(&layout.panels[0], Region::End), ["network", "clock"]);
        // the volume applet added later still goes first (left of it)
        layout
            .add_applet("top", "volume", Region::End, known)
            .unwrap();
        assert_eq!(
            types(&layout.panels[0], Region::End),
            ["volume", "network", "clock"]
        );
        // other regions keep their rule
        let mut layout = Layout::standard();
        layout
            .add_applet("top", "volume", Region::Start, known)
            .unwrap();
        layout
            .add_applet("top", "network", Region::Start, known)
            .unwrap();
        assert_eq!(
            types(&layout.panels[0], Region::Start),
            ["applications", "places", "system", "volume", "network"]
        );
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
            ..Default::default()
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
            ..Default::default()
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
        let empty = Layout {
            panels: vec![],
            ..Default::default()
        }
        .validated(known);
        assert!(empty.fallback);
        assert_eq!(empty.layout, Layout::standard());
        assert_eq!(empty.problems.len(), 1);

        let broken = Layout {
            panels: vec![PanelSpec::new("-x", Edge::Top, 32)],
            ..Default::default()
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
            ..Default::default()
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
        let mut layout = Layout {
            panels: vec![top],
            ..Default::default()
        };
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
            ..Default::default()
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

    // ---- HMP-11: application launchers ----

    #[test]
    fn launchers_get_their_own_ids_and_keep_their_application() {
        let mut layout = Layout::standard();
        for (app, id) in [
            ("firefox_firefox.desktop", "launcher-1"),
            ("org.example.Beispiel.desktop", "launcher-2"),
            ("firefox_firefox.desktop", "launcher-3"),
        ] {
            let added = layout
                .add_launcher("top", app, Region::Start, known)
                .expect("launcher");
            assert_eq!(added.instance, Instance::launcher(id, app));
        }
        let top = &layout.panels[0];
        assert_eq!(
            ids(top, Region::Start),
            [
                "applications-1",
                "places-1",
                "system-1",
                "launcher-1",
                "launcher-2",
                "launcher-3"
            ]
        );
        let apps: Vec<_> = top
            .region(Region::Start)
            .iter()
            .filter_map(|i| i.app.as_deref())
            .collect();
        assert_eq!(
            apps,
            [
                "firefox_firefox.desktop",
                "org.example.Beispiel.desktop",
                "firefox_firefox.desktop"
            ]
        );
        // The result is a valid layout as it stands.
        let checked = layout.clone().validated(known);
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert_eq!(checked.layout, layout);
    }

    #[test]
    fn a_launcher_needs_a_valid_desktop_id_and_others_take_none() {
        let mut layout = Layout::standard();
        for bad in [
            "",
            "firefox",
            ".desktop",
            "/usr/share/applications/firefox.desktop",
            "a;b.desktop",
            "neu\nzeile.desktop",
        ] {
            assert_eq!(
                layout.add_launcher("top", bad, Region::Start, known),
                Err(AddError::Application(bad.to_string())),
                "{bad:?}"
            );
        }
        assert_eq!(
            layout.add_applet("top", LAUNCHER, Region::Start, known),
            Err(AddError::Application(String::new()))
        );
        assert_eq!(layout, Layout::standard());
        assert!(valid_desktop_id("org.example.Querencia.Terminal.desktop"));
        assert!(valid_desktop_id("firefox_firefox.desktop"));
        assert!(valid_desktop_id("kde-org.kde.kate.desktop"));
    }

    #[test]
    fn validation_drops_launchers_without_application_and_strips_stray_ids() {
        let mut panel = PanelSpec::new("top", Edge::Top, DEFAULT_HEIGHT);
        panel.push(
            Region::Start,
            Instance::launcher("launcher-1", "foot.desktop"),
        );
        panel.push(Region::Start, Instance::new("launcher-2", LAUNCHER));
        panel.push(
            Region::Start,
            Instance::launcher("launcher-3", "kein/pfad.desktop"),
        );
        panel.push(
            Region::End,
            Instance {
                app: Some("foot.desktop".into()),
                ..Instance::new("clock-1", "clock")
            },
        );
        let checked = Layout {
            panels: vec![panel],
            ..Default::default()
        }
        .validated(known);
        let top = &checked.layout.panels[0];
        assert_eq!(ids(top, Region::Start), ["launcher-1"]);
        assert_eq!(top.region(Region::End), [Instance::new("clock-1", "clock")]);
        assert_eq!(checked.problems.len(), 3, "{:#?}", checked.problems);
    }

    #[test]
    fn removing_frees_the_id_and_leaves_the_rest() {
        let mut layout = Layout::standard();
        layout
            .add_launcher("bottom", "a.desktop", Region::Start, known)
            .unwrap();
        layout
            .add_launcher("bottom", "b.desktop", Region::Start, known)
            .unwrap();
        let removed = layout.remove_applet("launcher-1").expect("removed");
        assert_eq!(removed, Instance::launcher("launcher-1", "a.desktop"));
        assert_eq!(
            ids(&layout.panels[1], Region::Start),
            ["window-list-1", "launcher-2"]
        );
        assert_eq!(layout.remove_applet("launcher-1"), None);
        assert_eq!(layout.remove_applet("unbekannt"), None);
        let again = layout
            .add_launcher("bottom", "c.desktop", Region::Start, known)
            .unwrap();
        assert_eq!(again.instance.id, "launcher-1");
    }
}
