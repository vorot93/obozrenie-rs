#[macro_use]
extern crate enum_iter;
extern crate env_logger;
extern crate failure;
extern crate futures;
extern crate futures_timer;
extern crate gdk_pixbuf;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate librgs;
#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
extern crate tokio;

use env_logger::Builder as EnvLogBuilder;
use futures::{future, prelude::*};
use futures_timer::*;
use gio::prelude::*;
use gtk::prelude::*;
use static_resources::Resources;
use std::collections::{HashMap, HashSet};
use std::env;
use std::rc::Rc;
use std::sync::{
    mpsc::{channel, TryRecvError},
    Arc, Mutex,
};

mod static_resources;
mod treemodel;

use static_resources::*;
use treemodel::*;

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

    let server_list_view = resources.ui.get_object::<gtk::TreeView>("server_list_view").unwrap();

    server_list_view.connect_row_activated({
        let resources = resources.clone();
        let server_list = server_list.clone();
        let server_list_view = server_list_view.clone();
        move |_, path, _| {
            let (game, addr, need_pass) = get_selection_data(&server_list, &server_list.get_iter(path).unwrap());

            println!("Connecting to {} server at {}", game, addr);

            let launcher_fn = resources.game_list.0[&game].launcher_fn.clone();

            std::thread::spawn(move || (launcher_fn)(LaunchData { addr, password: None }).map(|mut cmd| cmd.spawn()));
        }
    });

    refresher.connect_clicked({
        let refresher = refresher.clone();
        let resources = resources.clone();
        move |_| {
            refresher.set_sensitive(false);
            server_list.clear();

            let (tx, rx) = channel::<(Game, librgs::Server)>();

            // Do the UI part of the server fetch
            gtk::timeout_add(10, {
                let refresher = refresher.clone();
                let server_list = server_list.clone();
                let resources = resources.clone();
                let present_servers = Arc::new(Mutex::new(HashSet::new()));
                move || {
                    use TryRecvError::*;

                    glib::Continue(match rx.try_recv() {
                        // Add and continue
                        Ok((game_id, srv)) => {
                            // Prevent duplicates
                            if present_servers.lock().unwrap().insert(srv.addr) {
                                treemodel::append_server(&server_list, game_id, resources.game_list.0[&game_id].icon.clone(), srv);
                            }
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

            let task_list = resources
                .game_list
                .clone()
                .0
                .into_iter()
                .map(|(id, e)| (id, e.query_fn))
                .collect::<HashMap<_, _>>();

            std::thread::spawn(move || {
                let timeout = std::time::Duration::from_secs(10);

                let total_queried = Arc::new(Mutex::new(0));

                debug!("Starting reactor");

                tokio::run(future::ok::<(), ()>(()).and_then({
                    let total_queried = total_queried.clone();
                    move |_| {
                        for (game_id, query_fn) in task_list {
                            let tx = tx.clone();
                            tokio::spawn(
                                (query_fn)()
                                    .inspect({
                                        let total_queried = total_queried.clone();
                                        move |srv| {
                                            tx.send((game_id, srv.clone())).unwrap();
                                            *total_queried.lock().unwrap() += 1;
                                        }
                                    })
                                    .map_err(move |e| {
                                        debug!("Error while querying {} returned an error: {:?}", game_id, e);
                                        e
                                    })
                                    .timeout(timeout)
                                    .for_each(|_| Ok(()))
                                    .map(|_| ())
                                    .map_err(|_| ()),
                            );
                        }

                        future::ok(())
                    }
                }));

                debug!("Queried {} servers", total_queried.lock().unwrap());
            });
        }
    });

    refresher.clicked();
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
        }
    });
    application.connect_activate(|_| {});

    application.run(&std::env::args().collect::<Vec<_>>());
}
