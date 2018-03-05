#[macro_use]
extern crate enum_iter;
extern crate env_logger;
extern crate futures;
extern crate futures_timer;
extern crate gdk_pixbuf;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate librgs;
#[macro_use]
extern crate log;
extern crate tokio;

use env_logger::Builder as EnvLogBuilder;
use futures::prelude::*;
use futures_timer::*;
use gio::prelude::*;
use gtk::prelude::*;
use librgs::ServerEntry;
use static_resources::Resources;
use std::env;
use std::rc::Rc;
use std::sync::{
    mpsc::{channel, TryRecvError},
    Arc, Mutex,
};

mod static_resources;
mod treemodel;

fn build_filters(resources: &Rc<Resources>) {
    let filter_toggle = resources.ui.get_object::<gtk::ToggleButton>("filter_toggle").unwrap();
    let filters = resources.ui.get_object::<gtk::Popover>("filters").unwrap();

    filter_toggle.connect_toggled({
        let filters = filters.clone();
        move |toggle| {
            if toggle.get_active() {
                filters.popup();
            } else {
                filters.popdown();
            }
        }
    });

    filters.connect_closed({
        let filter_toggle = filter_toggle.clone();
        move |_| {
            filter_toggle.set_active(false);
        }
    });
}

fn build_refresher(resources: &Rc<Resources>) {
    let refresher = resources.ui.get_object::<gtk::Button>("refresh_button").unwrap();

    let server_list = resources.ui.get_object::<gtk::ListStore>("server-list-store").unwrap();

    refresher.connect_clicked({
        let refresher = refresher.clone();
        let resources = resources.clone();
        move |_| {
            refresher.set_sensitive(false);
            server_list.clear();

            let pconfig = librgs::protocols::make_default_protocols();

            let (tx, rx) = channel::<ServerEntry>();

            // Do the UI part of the server fetch
            gtk::timeout_add(25, {
                let pconfig = pconfig.clone();
                let resources = resources.clone();
                let refresher = refresher.clone();
                let server_list = server_list.clone();
                move || {
                    use TryRecvError::*;

                    glib::Continue(match rx.try_recv() {
                        // Add and continue
                        Ok(entry) => {
                            treemodel::append_server(
                                &server_list,
                                &[
                                    treemodel::GameEntry {
                                        name: "q3a".into(),
                                        p: pconfig.get("q3s").unwrap().clone(),
                                        icon: resources.game_icons.get("q3a").unwrap().clone(),
                                    },
                                    treemodel::GameEntry {
                                        name: "openttd".into(),
                                        p: pconfig.get("openttds").unwrap().clone(),
                                        icon: resources.game_icons.get("openttd").unwrap().clone(),
                                    },
                                ],
                                entry,
                            );
                            true
                        }
                        Err(e) => match e {
                            // Check again later
                            Empty => true,
                            // Reset the button and exit after fetch thread dies
                            Disconnected => {
                                refresher.set_sensitive(true);
                                false
                            }
                        },
                    })
                }
            });

            std::thread::spawn(move || {
                let requests = vec![
                    librgs::UserQuery {
                        protocol: pconfig.get("openttdm".into()).unwrap().clone(),
                        host: librgs::Host::S(
                            librgs::StringAddr {
                                host: "master.openttd.org".into(),
                                port: 3978,
                            }.into(),
                        ),
                    },
                    librgs::UserQuery {
                        protocol: pconfig.get("q3m".into()).unwrap().clone(),
                        host: librgs::Host::S(
                            librgs::StringAddr {
                                host: "master3.idsoftware.com".into(),
                                port: 27950,
                            }.into(),
                        ),
                    },
                ];

                let timeout = std::time::Duration::from_secs(10);

                let total_queried = Arc::new(Mutex::new(0));

                debug!("Starting reactor");

                tokio::run(
                    librgs::simple_udp_query(requests)
                        .inspect({
                            let total_queried = total_queried.clone();
                            move |entry| {
                                tx.send(entry.clone()).unwrap();
                                *total_queried.lock().unwrap() += 1;
                            }
                        })
                        .map_err(|e| {
                            debug!("UdpQuery returned an error: {:?}", e);
                            e
                        })
                        .timeout(timeout)
                        .for_each(|_| Ok(()))
                        .map(|_| ())
                        .map_err(|_| ()),
                );

                debug!("Queried {} servers", total_queried.lock().unwrap());
            });
        }
    });
}

fn build_ui(app: &gtk::Application, resources: &Rc<Resources>) {
    build_refresher(resources);
    build_filters(resources);

    let window = resources.ui.get_object::<gtk::ApplicationWindow>("main_window").unwrap();
    window.connect_delete_event(|_, _| Inhibit(false));

    window.show_all();

    app.add_window(&window);
}

fn init_logging() {
    let mut builder = EnvLogBuilder::new();
    if let Ok(v) = env::var("RUST_LOG") {
        builder.parse(&v);
    }
    let stdio_logger = builder.build();
    let log_level = stdio_logger.filter();
    log::set_max_level(log_level);
    log::set_boxed_logger(Box::new(stdio_logger)).expect("Failed to install logger");
}

fn main() {
    init_logging();

    let application = gtk::Application::new("io.obozrenie", gio::ApplicationFlags::empty()).unwrap();
    let resources = static_resources::init().expect("GResource initialization failed.");
    application.connect_startup({
        move |app| {
            build_ui(app, &resources);
            resources.ui.get_object::<gtk::Button>("refresh_button").unwrap().clicked();
        }
    });
    application.connect_activate(|_| {});

    application.run(&std::env::args().collect::<Vec<_>>());
}
