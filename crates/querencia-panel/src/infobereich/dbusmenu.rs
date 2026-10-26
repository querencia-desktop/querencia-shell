//! The menus of the items: client of `com.canonical.dbusmenu` (the menu a
//! StatusNotifierItem names in its `Menu` property). The layout is read
//! afresh each time the menu opens and turned into a `gio::Menu` for the
//! classic menus of `querencia-oberflaeche`: separators become sections,
//! submenus submenus, check and radio items get the toggle attributes of
//! [`classic_menu::TOGGLE_ATTRIBUTE`], disabled items have no action
//! (shown insensitive), hidden ones are left out. Choosing an entry calls
//! `Event(id, "clicked", …)`.
//!
//! Opening: `AboutToShow(0)`, `GetLayout(0, -1, [])`, then
//! `AboutToShowGroup` for every submenu; if that asks for an update, the
//! layout is read once more. Every call has a time limit, and errors of
//! `AboutToShow*` (old servers do not have them) do not stop the menu.
//! `LayoutUpdated` and `ItemsPropertiesUpdated` only mark the layout as
//! changed (log): the next opening reads it anyway, and a classic menu's
//! model is not changed while it is open.
//!
//! Interface: libdbusmenu `dbus-menu.xml` (version 3).

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_oberflaeche::classic_menu;

pub const INTERFACE: &str = "com.canonical.dbusmenu";
/// Action of a menu entry in the item's action group: `klick` with the
/// entry's ID (`i`).
pub const ACTION_GROUP: &str = "sni";
pub const KLICK: &str = "klick";
/// Time limit of a call to the program's menu.
const TIMEOUT_MS: i32 = 2000;

/// One entry of a layout with the properties the menu uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Knoten {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub separator: bool,
    pub icon_name: String,
    pub icon_data: Vec<u8>,
    pub toggle: Option<classic_menu::Toggle>,
    /// GTK accelerator of the first shortcut (`<Control>q`).
    pub accel: Option<String>,
    pub submenu: bool,
    pub children: Vec<Knoten>,
}

/// The node of a `(ia{sv}av)` layout value; `None` for another type.
pub fn knoten(value: &glib::Variant) -> Option<Knoten> {
    if value.type_().as_str() != "(ia{sv}av)" {
        return None;
    }
    let id = value.child_value(0).get::<i32>()?;
    let props = value.child_value(1).get::<glib::VariantDict>()?;
    let text = |name: &str| {
        props
            .lookup_value(name, None)
            .and_then(|v| v.str().map(str::to_owned))
    };
    let flag =
        |name: &str, default: bool| props.lookup::<bool>(name).ok().flatten().unwrap_or(default);
    let toggle_state = props
        .lookup::<i32>("toggle-state")
        .ok()
        .flatten()
        .unwrap_or(0);
    let toggle = match text("toggle-type").as_deref() {
        Some("checkmark") => Some(classic_menu::Toggle::Check(toggle_state == 1)),
        Some("radio") => Some(classic_menu::Toggle::Radio(toggle_state == 1)),
        _ => None,
    };
    let accel = props
        .lookup_value("shortcut", None)
        .and_then(|v| v.get::<Vec<Vec<String>>>())
        .and_then(|list| list.into_iter().next())
        .and_then(|keys| accel(&keys));
    let children_value = value.child_value(2);
    let children: Vec<Knoten> = (0..children_value.n_children())
        .filter_map(|i| children_value.child_value(i).as_variant())
        .filter_map(|child| knoten(&child))
        .collect();
    Some(Knoten {
        id,
        label: ohne_mnemonic(&text("label").unwrap_or_default()),
        enabled: flag("enabled", true),
        visible: flag("visible", true),
        separator: text("type").as_deref() == Some("separator"),
        icon_name: text("icon-name").unwrap_or_default(),
        icon_data: props
            .lookup_value("icon-data", None)
            .and_then(|v| v.get::<Vec<u8>>())
            .unwrap_or_default(),
        toggle,
        accel,
        submenu: text("children-display").as_deref() == Some("submenu"),
        children,
    })
}

/// `label` without the access key marks (`_Datei` → `Datei`, `__` → `_`):
/// the classic menus show labels as they are.
pub fn ohne_mnemonic(label: &str) -> String {
    let mut out = String::new();
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                chars.next();
                out.push('_');
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A dbusmenu shortcut (`["Control", "Shift", "q"]`) as a GTK accelerator
/// (`<Control><Shift>q`); `None` without a key or with an unknown modifier.
pub fn accel(keys: &[String]) -> Option<String> {
    let (key, modifiers) = keys.split_last()?;
    if key.is_empty() {
        return None;
    }
    let mut out = String::new();
    for modifier in modifiers {
        let name = match modifier.as_str() {
            "Control" => "Control",
            "Alt" => "Alt",
            "Shift" => "Shift",
            "Super" => "Super",
            _ => return None,
        };
        out.push('<');
        out.push_str(name);
        out.push('>');
    }
    out.push_str(key);
    Some(out)
}

/// The IDs of all submenus below `node`, for `AboutToShowGroup`.
pub fn submenu_ids(node: &Knoten) -> Vec<i32> {
    let mut ids = Vec::new();
    for child in &node.children {
        if child.visible && (child.submenu || !child.children.is_empty()) {
            ids.push(child.id);
            ids.extend(submenu_ids(child));
        }
    }
    ids
}

/// The children of `node` as a menu: sections between separators, entries
/// with the action [`ACTION_GROUP`].[`KLICK`] and their ID as target.
pub fn menu(node: &Knoten) -> gio::Menu {
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    let mut sections = 0;
    let flush = |menu: &gio::Menu, section: &mut gio::Menu, sections: &mut usize| {
        if section.n_items() > 0 {
            menu.append_section(None, section);
            *sections += 1;
        }
        *section = gio::Menu::new();
    };
    for child in node.children.iter().filter(|c| c.visible) {
        if child.separator {
            flush(&menu, &mut section, &mut sections);
            continue;
        }
        section.append_item(&item(child));
    }
    if sections == 0 {
        // no separator: the entries directly, without a section
        return section;
    }
    flush(&menu, &mut section, &mut sections);
    menu
}

fn item(node: &Knoten) -> gio::MenuItem {
    let has_submenu = node.submenu || !node.children.is_empty();
    let item = if has_submenu && node.enabled {
        gio::MenuItem::new_submenu(Some(&node.label), &menu(node))
    } else {
        let item = gio::MenuItem::new(Some(&node.label), None);
        if node.enabled && !has_submenu {
            item.set_action_and_target_value(
                Some(&format!("{ACTION_GROUP}.{KLICK}")),
                Some(&node.id.to_variant()),
            );
        }
        item
    };
    if let Some(icon) = icon(node) {
        item.set_icon(&icon);
    }
    if let Some(toggle) = node.toggle {
        let (kind, on) = match toggle {
            classic_menu::Toggle::Check(on) => ("check", on),
            classic_menu::Toggle::Radio(on) => ("radio", on),
        };
        item.set_attribute_value(classic_menu::TOGGLE_ATTRIBUTE, Some(&kind.to_variant()));
        item.set_attribute_value(classic_menu::TOGGLED_ATTRIBUTE, Some(&on.to_variant()));
    }
    if let Some(accel) = &node.accel {
        item.set_attribute_value("accel", Some(&accel.to_variant()));
    }
    item
}

/// The entry's icon: its PNG data, else its theme name.
fn icon(node: &Knoten) -> Option<gio::Icon> {
    if !node.icon_data.is_empty() {
        return Some(gio::BytesIcon::new(&glib::Bytes::from(&node.icon_data)).upcast());
    }
    (!node.icon_name.is_empty()).then(|| gio::ThemedIcon::new(&node.icon_name).upcast())
}

/// Where a program's menu lives.
#[derive(Clone, Debug)]
pub struct Ziel {
    pub connection: gio::DBusConnection,
    pub bus: String,
    pub path: String,
}

impl Ziel {
    fn call(
        &self,
        method: &'static str,
        args: glib::Variant,
        reply: &'static str,
        done: impl FnOnce(Result<glib::Variant, glib::Error>) + 'static,
    ) {
        self.connection.call(
            Some(&self.bus),
            &self.path,
            INTERFACE,
            method,
            Some(&args),
            Some(glib::VariantTy::new(reply).expect("Signatur")),
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            done,
        );
    }

    /// Reads the whole layout as the menu opens (see the module text) and
    /// hands the root node, or the error of `GetLayout`, to `done`.
    pub fn laden(&self, done: impl FnOnce(Result<Knoten, String>) + 'static) {
        let ziel = self.clone();
        self.call("AboutToShow", (0i32,).to_variant(), "(b)", move |result| {
            if let Err(err) = result {
                eprintln!("querencia-panel: Infobereich: AboutToShow ohne Antwort: {err}");
            }
            let weiter = ziel.clone();
            weiter.layout(move |result| match result {
                Ok(root) => {
                    let ids = submenu_ids(&root);
                    if ids.is_empty() {
                        done(Ok(root));
                        return;
                    }
                    let again = ziel.clone();
                    ziel.call(
                        "AboutToShowGroup",
                        (ids,).to_variant(),
                        "(aiai)",
                        move |result| {
                            let neu = result
                                .ok()
                                .is_some_and(|antwort| antwort.child_value(0).n_children() > 0);
                            if neu {
                                again.layout(done);
                            } else {
                                done(Ok(root));
                            }
                        },
                    );
                }
                Err(err) => done(Err(err)),
            });
        });
    }

    fn layout(&self, done: impl FnOnce(Result<Knoten, String>) + 'static) {
        self.call(
            "GetLayout",
            (0i32, -1i32, Vec::<String>::new()).to_variant(),
            "(u(ia{sv}av))",
            move |result| {
                done(match result {
                    Ok(antwort) => {
                        knoten(&antwort.child_value(1)).ok_or_else(|| "Layout unlesbar".to_owned())
                    }
                    Err(err) => Err(err.to_string()),
                })
            },
        );
    }

    /// Tells the program about `event` (`clicked`, `opened`, `closed`) on
    /// entry `id`; errors go to the log.
    pub fn ereignis(&self, id: i32, event: &'static str) {
        // milliseconds as the X server time libdbusmenu passes; only the
        // order counts
        let zeit = (glib::monotonic_time() / 1000) as u32;
        let args = (
            id,
            event,
            glib::Variant::from_variant(&0i32.to_variant()),
            zeit,
        )
            .to_variant();
        self.call("Event", args, "()", move |result| {
            if let Err(err) = result {
                eprintln!(
                    "querencia-panel: Infobereich: Event {event} für {id} fehlgeschlagen: {err}"
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(eintraege: &[(&str, glib::Variant)]) -> glib::Variant {
        let dict = glib::VariantDict::new(None);
        for (name, wert) in eintraege {
            dict.insert_value(name, wert);
        }
        dict.end()
    }

    fn node(
        id: i32,
        eintraege: &[(&str, glib::Variant)],
        children: Vec<glib::Variant>,
    ) -> glib::Variant {
        let children: Vec<glib::Variant> =
            children.iter().map(glib::Variant::from_variant).collect();
        glib::Variant::tuple_from_iter([
            id.to_variant(),
            props(eintraege),
            glib::Variant::array_from_iter_with_type(glib::VariantTy::VARIANT, children),
        ])
    }

    fn label(menu: &gio::MenuModel, i: i32) -> String {
        menu.item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
            .unwrap_or_default()
    }

    fn layout() -> glib::Variant {
        let sub = node(
            10,
            &[
                ("label", "_Status".to_variant()),
                ("children-display", "submenu".to_variant()),
            ],
            vec![
                node(
                    11,
                    &[
                        ("label", "Verfügbar".to_variant()),
                        ("toggle-type", "radio".to_variant()),
                        ("toggle-state", 1i32.to_variant()),
                    ],
                    vec![],
                ),
                node(
                    12,
                    &[
                        ("label", "Abwesend".to_variant()),
                        ("toggle-type", "radio".to_variant()),
                        ("toggle-state", 0i32.to_variant()),
                    ],
                    vec![],
                ),
            ],
        );
        node(
            0,
            &[("children-display", "submenu".to_variant())],
            vec![
                node(
                    1,
                    &[
                        ("label", "Ö_ffnen".to_variant()),
                        ("icon-name", "document-open".to_variant()),
                        (
                            "shortcut",
                            vec![vec!["Control".to_string(), "o".to_string()]].to_variant(),
                        ),
                    ],
                    vec![],
                ),
                node(
                    2,
                    &[
                        ("label", "Stumm".to_variant()),
                        ("toggle-type", "checkmark".to_variant()),
                        ("toggle-state", 1i32.to_variant()),
                    ],
                    vec![],
                ),
                node(3, &[("type", "separator".to_variant())], vec![]),
                sub,
                node(
                    4,
                    &[
                        ("label", "Gesperrt".to_variant()),
                        ("enabled", false.to_variant()),
                    ],
                    vec![],
                ),
                node(
                    5,
                    &[
                        ("label", "Versteckt".to_variant()),
                        ("visible", false.to_variant()),
                    ],
                    vec![],
                ),
                node(6, &[("type", "separator".to_variant())], vec![]),
                node(7, &[("label", "_Beenden".to_variant())], vec![]),
            ],
        )
    }

    #[test]
    fn a_layout_becomes_nodes() {
        let root = knoten(&layout()).unwrap();
        assert_eq!(root.id, 0);
        assert_eq!(root.children.len(), 8);
        let open = &root.children[0];
        assert_eq!(open.label, "Öffnen");
        assert_eq!(open.accel.as_deref(), Some("<Control>o"));
        assert_eq!(open.icon_name, "document-open");
        assert_eq!(
            root.children[1].toggle,
            Some(classic_menu::Toggle::Check(true))
        );
        assert!(root.children[2].separator);
        assert!(root.children[3].submenu && root.children[3].children.len() == 2);
        assert!(!root.children[4].enabled);
        assert!(!root.children[5].visible);
        assert_eq!(submenu_ids(&root), [10]);
        assert!(knoten(&"falsch".to_variant()).is_none());
    }

    #[test]
    fn nodes_become_a_classic_menu() {
        let root = knoten(&layout()).unwrap();
        let menu = menu(&root).upcast::<gio::MenuModel>();
        let rows = classic_menu::rows(&menu);
        let text: Vec<String> = rows
            .iter()
            .map(|row| match row {
                classic_menu::Row::Separator => "---".into(),
                classic_menu::Row::Entry {
                    label,
                    action,
                    target,
                    toggle,
                    accel,
                    ..
                } => format!(
                    "{label} {} {:?} {toggle:?} {accel:?}",
                    action.as_deref().unwrap_or("-"),
                    target.as_ref().and_then(|t| t.get::<i32>())
                ),
                classic_menu::Row::Submenu { label, model, .. } => {
                    format!("{label} > {}", model.n_items())
                }
            })
            .collect();
        assert_eq!(
            text,
            [
                "Öffnen sni.klick Some(1) None Some(\"Ctrl+O\")",
                "Stumm sni.klick Some(2) Some(Check(true)) None",
                "---",
                "Status > 2",
                "Gesperrt - None None None",
                "---",
                "Beenden sni.klick Some(7) None None",
            ]
        );
        let classic_menu::Row::Submenu { model, .. } = &rows[3] else {
            panic!("Untermenü");
        };
        let sub = classic_menu::rows(model);
        assert!(matches!(
            sub[0],
            classic_menu::Row::Entry {
                toggle: Some(classic_menu::Toggle::Radio(true)),
                ..
            }
        ));
        assert_eq!(label(model, 1), "Abwesend");
    }

    #[test]
    fn a_disabled_submenu_is_an_insensitive_entry() {
        let mut root = knoten(&layout()).unwrap();
        root.children[3].enabled = false;
        let menu = menu(&root).upcast::<gio::MenuModel>();
        let rows = classic_menu::rows(&menu);
        assert!(matches!(
            &rows[3],
            classic_menu::Row::Entry { action: None, label, .. } if label == "Status"
        ));
    }

    #[test]
    fn mnemonics_and_shortcuts() {
        assert_eq!(ohne_mnemonic("_Datei"), "Datei");
        assert_eq!(ohne_mnemonic("a__b_c"), "a_bc");
        assert_eq!(ohne_mnemonic("_"), "");
        let s = |keys: &[&str]| accel(&keys.iter().map(|k| k.to_string()).collect::<Vec<_>>());
        assert_eq!(
            s(&["Control", "Shift", "q"]).as_deref(),
            Some("<Control><Shift>q")
        );
        assert_eq!(s(&["F1"]).as_deref(), Some("F1"));
        assert_eq!(s(&["Hyper", "x"]), None);
        assert_eq!(s(&[]), None);
        assert_eq!(s(&["Control", ""]), None);
    }

    #[test]
    fn png_data_wins_over_the_icon_name() {
        let mut root = knoten(&layout()).unwrap();
        root.children[0].icon_data = vec![0x89, b'P', b'N', b'G'];
        assert!(icon(&root.children[0]).unwrap().is::<gio::BytesIcon>());
        assert!(icon(&root.children[7]).is_none());
    }
}
