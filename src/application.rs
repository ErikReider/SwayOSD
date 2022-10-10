use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;

use gtk::gio::{ApplicationFlags, Cancellable};
use gtk::glib::{clone, SignalHandlerId, VariantTy};
use gtk::prelude::*;
use gtk::*;

use crate::osd_window::SwayosdWindow;
use crate::utils::*;

use gtk::gio::SimpleAction;

const ACTION_NAME: &str = "action";
const ACTION_FORMAT: &str = "s";

#[derive(PartialEq, Debug)]
pub enum OsdTypes {
    NONE = 0,
    VOLUME = 1,
}

impl OsdTypes {
    fn as_str(&self) -> &'static str {
        match self {
            OsdTypes::NONE => "NONE",
            OsdTypes::VOLUME => "VOLUME",
        }
    }

    fn parse(value: &str) -> Self {
        // TODO: Fix ALWAYS BEING "NONE"
        println!("VAL: {} {}", value, value == "VOLUME".to_string());
        match value {
            "VOLUME" => OsdTypes::VOLUME,
            _ => OsdTypes::NONE,
        }
    }
}

pub enum DisplayEvent {
    Opened(gdk::Display),
    Closed(bool),
    Added(gdk::Display, gdk::Monitor),
    Removed(gdk::Display),
}

#[derive(Clone, Shrinkwrap)]
pub struct SwayOSDApplication {
    #[shrinkwrap(main_field)]
    app: gtk::Application,
    action_id: Rc<RefCell<Option<SignalHandlerId>>>,
    windows: Rc<RefCell<Vec<SwayosdWindow>>>,
}

impl SwayOSDApplication {
    pub fn new() -> Self {
        SwayOSDApplication {
            app: Application::new(Some("org.erikreider.swayosd"), ApplicationFlags::FLAGS_NONE),
            action_id: Rc::new(RefCell::new(None)),
            windows: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn start(&self) -> i32 {
        let s = self.clone();
        self.app.connect_activate(move |_| s.activate());

        match VariantTy::new(ACTION_FORMAT) {
            Ok(variant_ty) => {
                let action = SimpleAction::new(ACTION_NAME, Some(variant_ty));
                let s = self.clone();
                self.action_id.replace(Some(
                    action.connect_activate(move |sa, v| s.action_activated(sa, v)),
                ));
                self.app.add_action(&action);
                let _ = self.app.register(Cancellable::NONE);
                self.app
                    .activate_action(ACTION_NAME, Some(&OsdTypes::VOLUME.as_str().to_variant()));
            }
            Err(x) => {
                eprintln!("VARIANT TYPE ERROR: {}", x.message);
                std::process::exit(1);
            }
        }

        return self.app.run();
    }

    fn activate(&self) {
        if self.app.windows().len() > 0 {
            return;
        }

        let (tx, rx) = async_channel::unbounded();

        self.initialize(tx);

        // Processes all application events received from signals
        let s = self.clone();
        let event_handler = async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    DisplayEvent::Opened(d) => {
                        s.init_windows(&d);
                    }
                    DisplayEvent::Closed(is_error) => {
                        if is_error {
                            eprintln!("Display closed due to errors...");
                        }
                        s.close_all_windows();
                    }
                    DisplayEvent::Added(d, mon) => {
                        s.add_window(&d, &mon);
                    }
                    DisplayEvent::Removed(d) => {
                        s.init_windows(&d);
                    }
                }
            }
        };
        spawn(event_handler);
    }

    fn action_activated(&self, action: &SimpleAction, variant: Option<&glib::Variant>) {
        let variant: &glib::Variant = match variant {
            Some(x) => x,
            _ => return,
        };
        println!("Variant: {} {:#?}", variant.print(true), OsdTypes::parse(&variant.to_string()));
        let osd_type = match OsdTypes::parse(&variant.to_string()) {
            OsdTypes::NONE => return,
            x => x,
        };

        println!("TYPE: {:#?}", osd_type);

        match self.action_id.take() {
            Some(action_id) => action.disconnect(action_id),
            None => return,
        }

        for window in self.app.windows() {
            // let window = window.upcast();
            // println!("{}", window.);
        }

        let s = self.clone();
        let id = action.connect_activate(move |sa, v| s.action_activated(sa, v));
        self.action_id.replace(Some(id));
    }

    fn initialize(&self, tx: Sender<DisplayEvent>) {
        let display: gdk::Display = match gdk::Display::default() {
            Some(x) => x,
            _ => return,
        };

        self.init_windows(&display);

        display.connect_opened(clone!(@strong tx => move |d| {
            spawn(clone!(@strong tx, @strong d => async move {
                let _ = (&tx).send(DisplayEvent::Opened(d)).await;
            }));
        }));

        display.connect_closed(clone!(@strong tx => move |_d, is_error| {
            spawn(clone!(@strong tx => async move {
                let _ = tx.send(DisplayEvent::Closed(is_error)).await;
            }));
        }));

        display.connect_monitor_added(clone!(@strong tx => move |d, mon| {
            spawn(clone!(@strong tx, @strong d, @strong mon => async move {
                let _ = tx.send(DisplayEvent::Added(d, mon)).await;
            }));
        }));

        display.connect_monitor_removed(clone!(@strong tx => move |d, _mon| {
            spawn(clone!(@strong tx, @strong d => async move {
                let _ = tx.send(DisplayEvent::Removed(d)).await;
            }));
        }));
    }

    fn add_window(&self, display: &gdk::Display, monitor: &gdk::Monitor) {
        let win = SwayosdWindow::new(&self.app, &display, &monitor);
        win.present();
        self.windows.borrow_mut().push(win);
    }

    fn init_windows(&self, display: &gdk::Display) {
        self.close_all_windows();

        for i in 0..display.n_monitors() {
            let monitor: gdk::Monitor = match display.monitor(i) {
                Some(x) => x,
                _ => continue,
            };
            self.add_window(&display, &monitor);
        }
    }

    fn close_all_windows(&self) {
        for window in self.app.windows() {
            window.close();
        }
        self.windows.borrow_mut().clear();
    }
}
