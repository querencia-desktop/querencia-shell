//! The isolated GUI test of the classic menus (HMP-04/05) with the panel's
//! own buttons, style sheet and desktop entries; the test itself is
//! `querencia_oberflaeche::classic_menu::gui_test` (until 0.3.0
//! `classic_menu/gui_test.rs` of the panel). Runs only inside the isolated
//! session (`QUERENCIA_ISOLATED_GUI=1`, throw-away runtime directory).

use querencia_oberflaeche::classic_menu::gui_test::{Panel, isolated_gui_menus};

#[test]
#[ignore = "nur in der isolierten GUI-Umgebung (gui-test/isoliert.py)"]
fn isolated_gui_menus_of_the_panel() {
    isolated_gui_menus(&Panel {
        // as in main(): catalogs bound after GTK set the locale (the test
        // looks for German labels; catalogs from QUERENCIA_PANEL_LOCALEDIR)
        install_style: || {
            crate::i18n::init();
            crate::install_panel_style();
        },
        main_menus: || {
            [
                crate::app_menu::button(crate::layout::DEFAULT_HEIGHT),
                crate::places_menu::button(),
                crate::system_menu::button(),
            ]
        },
        menu_button: |label, menu| crate::app_menu::menu_button(label, None, menu),
    });
}
