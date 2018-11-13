#![feature(generators)]

extern crate derive_more;
extern crate enum_iter;
extern crate env_logger;
extern crate failure;
extern crate futures_await as futures;
extern crate futures_timer;
extern crate gdk_pixbuf;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate librgs;
extern crate log;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate tokio_core;
extern crate tokio_dns;
extern crate tokio_ping;

use env_logger::Builder as EnvLogBuilder;
use futures::{future, prelude::*};
use futures_timer::*;
use gio::prelude::*;
use gtk::prelude::*;
use log::debug;
use static_resources::Resources;
use std::collections::{HashMap, HashSet};
use std::env;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc::{channel, TryRecvError},
    Arc, Mutex,
};

mod games;
mod static_resources;
mod widgets;

use widgets::*;

fn build_filters(resources: &Rc<Resources>) {
    let filter_model = resources.ui.get_object::<ServerListFilter, _>().0;

    let filter_toggle = resources.ui.get_object::<FilterToggle, _>().0;
    let filters = resources.ui.get_object::<FiltersPopover, _>().0;

    // Fill list of games in filter menu
    let game_list = resources.ui.get_object::<GameListStore, _>().0;

    for (id, entry) in resources.game_list.0.iter() {
        append_game(&game_list, *id, entry.icon.clone());
    }

    // Refilter on changes
    resources.ui.get_object::<GameListView, _>().0.get_selection().connect_changed({
        let filter_model = filter_model.clone();
        move |_| {
            filter_model.refilter();
        }
    });

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

    filter_model.set_visible_func({
        move |model, iter| {
            let list_store = model.clone().downcast::<gtk::ListStore>().unwrap();
            if list_store.get_iter_first().is_some() {
                let selection_data = ServerStore(list_store).get_server(iter.into());
                debug!("Refiltering... {:?}", selection_data);
            }
            true
        }
    });
}

fn build_refresher(resources: &Rc<Resources>) {
    let refresher = resources.ui.get_object::<RefreshButton, _>().0;

    let server_list = resources.ui.get_object::<ServerStore, _>();

    let server_list_view = resources.ui.get_object::<ServerListView, _>().0;

    server_list_view.connect_row_activated({
        let resources = resources.clone();
        let server_list = server_list.clone();
        move |_, path, _| {
            let SelectionData { game_id, addr, need_pass } = server_list.get_server(&server_list.0.get_iter(path).unwrap()).unwrap();

            let f = Rc::new({
                let addr = addr.clone();
                let game_launcher = resources.game_list.0[&game_id].launcher.clone();

                move |password: Option<String>| {
                    let addr = addr.clone();
                    let game_launcher = game_launcher.clone();

                    println!("Connecting to {} server at {}", game_id, addr);

                    std::thread::spawn({
                        move || {
                            game_launcher
                                .launch_cmd(&games::LaunchData { addr, password })
                                .map(|mut cmd| cmd.spawn());
                        }
                    });
                }
            }) as Rc<Fn(Option<String>)>;

            if need_pass {
                let password_request = resources.ui.get_object::<PasswordRequest, _>().0;
                let password_entry = resources.ui.get_object::<PasswordEntry, _>().0;
                let connect_button = resources.ui.get_object::<ConnectWithPassword, _>().0;

                password_entry.connect_changed({
                    let connect_button = connect_button.clone();
                    let password_entry = password_entry.clone();
                    move |_| {
                        connect_button.set_sensitive(password_entry.get_text_length() > 0);
                    }
                });

                connect_button.connect_clicked({
                    let f = f.clone();
                    move |_| (f)(password_entry.get_text())
                });
                password_request.popup();
            } else {
                (f)(None)
            }
        }
    });

    refresher.connect_clicked({
        let refresher = refresher.clone();
        let resources = resources.clone();
        move |_| {
            refresher.set_sensitive(false);
            server_list.0.clear();

            let (sink, fountain) = channel::<(games::Game, librgs::Server)>();

            // Do the UI part of the server fetch
            gtk::timeout_add(10, {
                let refresher = refresher.clone();
                let server_list = server_list.clone();
                let resources = resources.clone();
                let present_servers = Arc::new(Mutex::new(HashSet::new()));
                move || {
                    use TryRecvError::*;

                    glib::Continue(match fountain.try_recv() {
                        // Insert new server entry and continue
                        Ok((game_id, srv)) => {
                            // Prevent duplicates
                            if present_servers.lock().unwrap().insert(srv.addr) {
                                let game_entry = resources.game_list.0[&game_id].clone();
                                server_list.append_server(game_id, game_entry.icon.clone(), game_entry.name_morpher.clone(), srv);
                            }
                            true
                        }
                        Err(e) => match e {
                            // No new entries, check again later
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
                .map(|(id, e)| (id, e.querier))
                .collect::<HashMap<_, _>>();

            std::thread::spawn(move || {
                let timeout = std::time::Duration::from_secs(10);

                let total_queried = Arc::new(AtomicUsize::new(0));

                debug!("Starting query");

                tokio::run(future::ok::<(), ()>(()).and_then({
                    let total_queried = total_queried.clone();
                    move |_| {
                        for (game_id, querier) in task_list {
                            let tx = sink.clone();
                            tokio::spawn(
                                querier
                                    .query()
                                    .inspect({
                                        let total_queried = total_queried.clone();
                                        move |srv| {
                                            tx.send((game_id, srv.clone())).unwrap();
                                            total_queried.fetch_add(1, Ordering::Relaxed);
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

                debug!("Queried {} servers", total_queried.load(Ordering::Relaxed));
            });
        }
    });

    refresher.clicked();
}

fn build_ui(app: &gtk::Application, resources: &Rc<Resources>) {
    build_refresher(resources);
    build_filters(resources);

    let window = resources.ui.get_object::<MainWindow, _>().0;
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
