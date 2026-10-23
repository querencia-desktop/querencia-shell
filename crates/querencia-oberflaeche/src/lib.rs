//! Querencia Oberfläche: the GTK building blocks that panel, desktop and the
//! coming file manager share, instead of including the panel's source files
//! through `#[path]` (until Panel 0.3.0 and Desktop 0.2.6).
//!
//! - [`classic_menu`]: GNOME 2 menus drawn from `gio::MenuModel`s, with
//!   icons, shortcuts, submenus beside their row and GTK 2 keyboard; a row
//!   of main menu buttons acts as one menu bar.
//! - [`symbole`]: theme icons without GTK's preloading in the thread pool
//!   (QD-106); every program that shows many icons uses it.
//! - [`icon_theme`]: the user's icon theme, else GNOME Human, followed at
//!   run time.
//! - [`farbschema`]: the dark rules of a program's own style sheet, only
//!   while a dark colour scheme meets a theme with a dark variant.
//! - [`portal`]: the settings portal at start, with GSettings as fallback.
//! - [`i18n`]: the library's text domain `querencia-oberflaeche`; a program
//!   binds it to its catalog folder with [`i18n::bind`].
//!
//! The library sets no style of its own: the CSS classes it uses
//! (`classic_menu::MENU_CLASS` and the others) are styled by the program.

pub mod classic_menu;
pub mod farbschema;
pub mod i18n;
pub mod icon_theme;
pub mod portal;
pub mod symbole;
