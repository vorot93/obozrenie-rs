// Obozrenie Game Server Browser
// Copyright (C) 2018-2019  Artem Vorotnikov
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

#![feature(async_await, generators, gen_future)]

use futures::{compat::*, prelude::*};
use gio::prelude::*;
use gtk::prelude::*;
use log::*;
use static_resources::Resources;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc::{channel, TryRecvError},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tokio::prelude::StreamExt;

mod filters;
mod games;
mod static_resources;
mod widgets;

use crate::widgets::*;

#[derive(Clone, Debug)]
enum AppEvent {
    AddServer((games::Game, rgs::models::Server)),
    RefreshComplete,
}

#[derive(Clone)]
enum AppCommand {
    StartRefresh(HashMap<games::Game, Arc<dyn games::Querier>>),
}

fn build_filters(resources: &Rc<Resources>) {
    let filter_model = resources.ui.get_object::<ServerListFilter, _>().0;

    let filter_toggle = resources.ui.get_object::<FilterToggle, _>().0;
    let filters = resources.ui.get_object::<FiltersPopover, _>().0;

    // Fill list of games in filter menu
    let game_list = resources.ui.get_object::<GameListStore, _>();

    for (id, entry) in resources.game_list.0.iter() {
        game_list.append_game(*id, entry.icon.clone());
    }

    let filter_data = Arc::new(Mutex::new(filters::Filters::default()));

    // Refilter on changes
    resources
        .ui
        .get_object::<GameListView, _>()
        .0
        .get_selection()
        .connect_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            let game_list = game_list.clone();
            move |s| {
                {
                    let value = {
                        let (selection, model) = s.get_selected_rows();

                        selection
                            .into_iter()
                            .map(|path| {
                                let iter = model.get_iter(&path).unwrap();

                                game_list.get_game(&iter).0
                            })
                            .collect::<HashSet<_>>()
                    };
                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).games;

                    *v = value;
                }

                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<ModFilter, _>()
        .0
        .connect_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w
                        .get_text()
                        .map(|s| s.to_string())
                        .unwrap_or_else(String::new);
                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).game_mod;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<GameTypeFilter, _>()
        .0
        .connect_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w
                        .get_text()
                        .map(|s| s.to_string())
                        .unwrap_or_else(String::new);
                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).game_type;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<MapFilter, _>()
        .0
        .connect_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w
                        .get_text()
                        .map(|s| s.to_string())
                        .unwrap_or_else(String::new);
                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).map;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<PingFilter, _>()
        .0
        .connect_value_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = std::time::Duration::from_millis(w.get_value_as_int() as u64);
                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).max_ping;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<AntiCheatFilter, _>()
        .0
        .connect_changed({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = match w.get_active_text().unwrap().as_str() {
                        "Enabled" => Some(true),
                        "Disabled" => Some(false),
                        "Ignore" => None,
                        other => unreachable!(format!("Invalid variant: {}", other)),
                    };

                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).anticheat;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<NotFullFilter, _>()
        .0
        .connect_toggled({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w.get_active();

                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).not_full;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<NotEmptyFilter, _>()
        .0
        .connect_toggled({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w.get_active();

                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).not_empty;

                    *v = value;
                }
                filter_model.refilter();
            }
        });
    resources
        .ui
        .get_object::<NoPasswordFilter, _>()
        .0
        .connect_toggled({
            let filter_data = filter_data.clone();
            let filter_model = filter_model.clone();
            move |w| {
                {
                    let value = w.get_active();

                    let mut f = filter_data.lock().unwrap();

                    let v = &mut (*f).no_password;

                    *v = value;
                }
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
        let filter_data = filter_data.clone();
        move |model, iter| {
            let list_store = model.clone().downcast::<gtk::ListStore>().unwrap();

            let (game, server) = ServerStore(list_store).get_server(iter.into());

            trace!("Refiltering... {:?}", server);

            filter_data.lock().unwrap().matches(game, &server)
        }
    });
}

fn build_ui(
    app: &gtk::Application,
    executor: tokio::runtime::TaskExecutor,
    resources: &Rc<Resources>,
) {
    let (cmd_sink, cmd_faucet) = channel::<AppCommand>();
    let (event_sink, event_faucet) = channel::<AppEvent>();

    let refresher = resources.ui.get_object::<RefreshButton, _>().0;

    let server_list = resources.ui.get_object::<ServerStore, _>();

    let server_list_view = resources.ui.get_object::<ServerListView, _>().0;

    server_list_view.connect_row_activated({
        let resources = resources.clone();
        let server_list = server_list.clone();
        move |_, path, _| {
            let (
                game_id,
                rgs::models::Server {
                    addr, need_pass, ..
                },
            ) = server_list.get_server(&server_list.0.get_iter(path).unwrap());

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
                                .launch_cmd(&games::LaunchData {
                                    addr: addr.to_string(),
                                    password,
                                })
                                .map(|mut cmd| cmd.spawn());
                        }
                    });
                }
            }) as Rc<dyn Fn(Option<String>)>;

            if let Some(true) = need_pass {
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
                    move |_| (f)(password_entry.get_text().map(|s| s.to_string()))
                });

                password_request.popup();
            } else {
                (f)(None)
            }
        }
    });

    let present_servers = Arc::new(Mutex::new(HashSet::new()));

    refresher.connect_clicked({
        let cmd_sink = cmd_sink.clone();
        let refresher = refresher.clone();
        let resources = resources.clone();
        let server_list = server_list.clone();
        let present_servers = present_servers.clone();
        move |_| {
            refresher.set_sensitive(false);
            server_list.0.clear();
            present_servers.lock().unwrap().clear();

            cmd_sink
                .send(AppCommand::StartRefresh(
                    resources
                        .game_list
                        .clone()
                        .0
                        .into_iter()
                        .map(|(id, e)| (id, e.querier))
                        .collect(),
                ))
                .unwrap();
        }
    });

    build_filters(resources);

    executor.spawn({
        let cmd_sink = cmd_sink.clone();
        let event_sink = event_sink.clone();

        async move {
            use TryRecvError::*;

            let _ = cmd_sink.clone();

            loop {
                match cmd_faucet.try_recv() {
                    Ok(cmd) => match cmd {
                        AppCommand::StartRefresh(task_list) => {
                            let total_queried = Arc::new(AtomicUsize::new(0));

                            let timeout = std::time::Duration::from_secs(10);

                            debug!("Starting query");

                            tokio::spawn({
                                use futures01::{future as future01, prelude::*};

                                future01::join_all(task_list.into_iter().map({
                                    let event_sink = event_sink.clone();
                                    let total_queried = total_queried.clone();

                                    move |(game_id, querier)| {
                                        querier
                                            .query()
                                            .inspect({
                                                let event_sink = event_sink.clone();
                                                let total_queried = total_queried.clone();
                                                move |srv| {
                                                    event_sink
                                                        .send(AppEvent::AddServer((
                                                            game_id,
                                                            srv.clone(),
                                                        )))
                                                        .unwrap();
                                                    total_queried.fetch_add(1, Ordering::Relaxed);
                                                }
                                            })
                                            .map_err(move |e| {
                                                debug!(
                                                    "Error while querying {} returned an error: {:?}",
                                                    game_id, e
                                                );
                                                e
                                            })
                                            .timeout(timeout)
                                            .for_each(|_| Ok(()))
                                            .inspect(move |_| debug!("{} query complete", game_id))
                                    }
                                }))
                                    .then({
                                        let event_sink = event_sink.clone();
                                        move |_| {
                                            debug!(
                                                "Queried {} servers",
                                                total_queried.load(Ordering::Relaxed)
                                            );

                                            event_sink.send(AppEvent::RefreshComplete).unwrap();

                                            Ok(())
                                        }
                                    })
                            });
                        }
                    },
                    Err(e) => match e {
                        Empty => {}
                        Disconnected => {
                            return;
                        }
                    },
                }

                tokio::timer::Delay::new(Instant::now() + Duration::from_millis(100))
                    .compat()
                    .await
                    .unwrap()
            }
        }
            .map(|_| Ok(()))
            .boxed()
            .compat()
    });

    gtk::timeout_add(10, {
        let event_sink = event_sink.clone();
        let refresher = refresher.clone();
        let server_list = server_list.clone();
        let resources = resources.clone();
        let present_servers = present_servers.clone();
        move || {
            use TryRecvError::*;

            let _ = event_sink.clone();

            glib::Continue({
                match event_faucet.try_recv() {
                    // Insert new server entry and continue
                    Ok(ev) => {
                        match ev {
                            AppEvent::AddServer((game_id, srv)) => {
                                // Prevent duplicates
                                if present_servers.lock().unwrap().insert(srv.addr) {
                                    let game_entry = resources.game_list.0[&game_id].clone();
                                    server_list.append_server(
                                        game_id,
                                        game_entry.icon.clone(),
                                        game_entry.name_morpher.clone(),
                                        srv,
                                    );
                                }
                            }
                            AppEvent::RefreshComplete => {
                                refresher.set_sensitive(true);
                            }
                        };

                        true
                    }
                    Err(e) => match e {
                        Empty => true,
                        Disconnected => false,
                    },
                }
            })
        }
    });

    refresher.clicked();

    let window = resources.ui.get_object::<MainWindow, _>().0;
    window.connect_delete_event(|_, _| Inhibit(false));

    window.show_all();

    app.add_window(&window);
}

fn main() {
    env_logger::init();

    let rt = tokio::runtime::Runtime::new().unwrap();

    let application =
        gtk::Application::new(Some("io.obozrenie"), gio::ApplicationFlags::empty()).unwrap();
    let resources = static_resources::init().expect("GResource initialization failed.");
    application.connect_startup({
        let executor = rt.executor();
        move |app| {
            build_ui(app, executor.clone(), &resources);
        }
    });
    application.connect_activate(|_| {});

    application.run(&std::env::args().collect::<Vec<_>>());
}
