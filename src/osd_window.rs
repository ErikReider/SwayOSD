use gtk::prelude::*;
use gtk::*;

/// A window that our application can open that contains the main project view.
#[derive(Shrinkwrap)]
pub struct SwayosdWindow {
    #[shrinkwrap(main_field)]
    window: gtk::ApplicationWindow,
    pub container: gtk::Box,
    pub display: gdk::Display,
    pub monitor: gdk::Monitor,
}

impl SwayosdWindow {
    /// Create a new window and assign it to the given application.
    pub fn new(app: &Application, display: &gdk::Display, monitor: &gdk::Monitor) -> Self {
        let window = gtk::ApplicationWindow::new(app);

        gtk_layer_shell::init_for_window(&window);
        gtk_layer_shell::set_monitor(&window, &monitor);
        gtk_layer_shell::set_namespace(&window, "swayosd");

        gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay);
        gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Top, true);

        let margin =
            ((monitor.workarea().height() - window.height_request()) as f32 * 0.75).round() as i32;
        gtk_layer_shell::set_margin(&window, gtk_layer_shell::Edge::Top, margin);

        // Set up a widget
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.add(&container);

        let label = gtk::Label::new(Some(""));
        label.set_markup("<span font_desc=\"20.0\">GTK Layer Shell example!</span>");
        container.add(&label);
        window.set_border_width(12);
        window.show_all();

        return Self {
            window,
            container,
            display: display.clone(),
            monitor: monitor.clone(),
        };
    }
}
