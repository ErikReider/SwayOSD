use gtk::glib;

pub fn spawn<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    glib::MainContext::default().spawn_local(future);
}
