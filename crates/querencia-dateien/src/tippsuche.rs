//! Type-ahead (Nautilus 2.30, GtkTreeView's interactive search): typing in
//! a view jumps to the first name that begins with the typed text; a small
//! box at the bottom right shows it, red without a match. Up and Down go
//! to the previous and next match; the box closes after five seconds
//! without a key, with Esc, Enter or Tab. GTK 4's list views have none.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;

/// GtkTreeView's and Nautilus 2.30's timeout.
pub const ZEIT: Duration = Duration::from_secs(5);

pub struct Tippsuche {
    pub rahmen: gtk4::Frame,
    text: gtk4::Label,
    eingabe: RefCell<String>,
    quelle: RefCell<Option<glib::SourceId>>,
}

impl Tippsuche {
    pub fn new() -> Rc<Self> {
        let text = gtk4::Label::new(None);
        let rahmen = gtk4::Frame::builder()
            .child(&text)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::End)
            .margin_end(12)
            .margin_bottom(8)
            .visible(false)
            .can_target(false)
            .css_classes(["suchfeld"])
            .build();
        Rc::new(Self {
            rahmen,
            text,
            eingabe: RefCell::new(String::new()),
            quelle: RefCell::new(None),
        })
    }

    pub fn offen(&self) -> bool {
        !self.eingabe.borrow().is_empty()
    }

    pub fn eingabe(&self) -> String {
        self.eingabe.borrow().clone()
    }

    pub fn anhaengen(&self, c: char) {
        self.eingabe.borrow_mut().push(c);
    }

    /// Backspace: the text without its last character (empty = closed).
    pub fn zuruecknehmen(&self) -> String {
        let mut e = self.eingabe.borrow_mut();
        e.pop();
        e.clone()
    }

    /// Shows the box with the text, marked red without a match, and
    /// (re)starts the timeout that closes it.
    pub fn zeigen(self: &Rc<Self>, gefunden: bool) {
        self.text.set_text(&self.eingabe.borrow());
        self.rahmen.set_visible(true);
        if gefunden {
            self.rahmen.remove_css_class("ohne-treffer");
        } else {
            self.rahmen.add_css_class("ohne-treffer");
        }
        let weak = Rc::downgrade(self);
        let quelle = glib::timeout_add_local_once(ZEIT, move || {
            if let Some(t) = weak.upgrade() {
                t.quelle.borrow_mut().take();
                t.schliessen();
            }
        });
        if let Some(alt) = self.quelle.borrow_mut().replace(quelle) {
            alt.remove();
        }
    }

    pub fn schliessen(&self) {
        self.eingabe.borrow_mut().clear();
        if let Some(q) = self.quelle.borrow_mut().take() {
            q.remove();
        }
        self.rahmen.set_visible(false);
    }
}
