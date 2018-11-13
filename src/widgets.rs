use crate::games::*;

use derive_more::From;
use enum_iter::EnumIterator;
use failure::{format_err, Fallible};
use gdk_pixbuf::Pixbuf;
use gtk::{self, prelude::*, TreeIter};
use librgs;
use std::sync::Arc;

pub trait Widget<O> {
    fn id() -> &'static str;
    fn inner(self) -> O;
}

macro_rules! widget {
    ($name:ident, $inner:ty, $id:expr) => {
        #[derive(Clone, Debug, From)]
        pub struct $name(pub $inner);

        impl Widget<$inner> for $name {
            fn id() -> &'static str {
                $id
            }

            fn inner(self) -> $inner {
                self.0
            }
        }
    };
}

widget!(ServerListFilter, gtk::TreeModelFilter, "ServerListFilter");
widget!(ServerListView, gtk::TreeView, "ServerListView");

widget!(FilterToggle, gtk::ToggleButton, "FilterToggle");
widget!(FiltersPopover, gtk::Popover, "FiltersPopover");
widget!(GameListStore, gtk::ListStore, "GameListStore");
widget!(GameListView, gtk::TreeView, "GameListView");
widget!(MainWindow, gtk::ApplicationWindow, "MainWindow");
widget!(RefreshButton, gtk::Button, "RefreshButton");

widget!(PasswordRequest, gtk::Popover, "PasswordRequest");
widget!(PasswordEntry, gtk::Entry, "PasswordEntry");
widget!(ConnectWithPassword, gtk::Button, "ConnectWithPassword");

pub struct UIBuilder {
    pub inner: gtk::Builder,
}

impl UIBuilder {
    pub fn get_object<T, O>(&self) -> T
    where
        T: Widget<O> + std::convert::From<O>,
        O: glib::IsA<glib::Object>,
    {
        T::from(self.inner.get_object::<O>(T::id()).unwrap())
    }
}

#[derive(Clone, Copy, Debug, EnumIterator)]
pub enum GameStoreColumn {
    Id = 0,
    Name,
    Icon,
    StatusIcon,
}

pub fn append_game(model: &gtk::ListStore, game_id: Game, icon: Pixbuf) {
    let iter = model.insert_with_values(None, &[], &[]);
    for (i, col) in GameStoreColumn::enum_iter().enumerate() {
        let insertable: Option<gtk::Value> = match col {
            GameStoreColumn::Id => Some(From::from(game_id.id().clone())),
            GameStoreColumn::Name => Some(From::from(&game_id.to_string())),
            GameStoreColumn::Icon => Some(From::from(&icon.clone())),
            _ => None,
        };

        if let Some(v) = insertable {
            model.set_value(&iter, i as u32, &v);
        }
    }
}

#[derive(Clone, Copy, Debug, EnumIterator)]
pub enum ServerStoreColumn {
    Host = 0,
    NeedPass,
    PlayerCount,
    PlayerLimit,
    Ping,
    Secure,
    Country,
    Name,
    GameId,
    GameMod,
    GameType,
    Map,
    GameIcon,
    LockIcon,
    SecureIcon,
    CountryIcon,
}

#[derive(Clone, Debug, From)]
pub struct ServerStore(pub gtk::ListStore);

impl Widget<gtk::ListStore> for ServerStore {
    fn id() -> &'static str {
        "ServerStore"
    }

    fn inner(self) -> gtk::ListStore {
        self.0
    }
}

#[derive(Clone, Debug, From)]
pub enum ServerEntryPath {
    Iter(gtk::TreeIter),
    Path(gtk::TreePath),
}

#[derive(Clone, Debug)]
pub struct SelectionData {
    pub game_id: Game,
    pub addr: String,
    pub need_pass: bool,
}

impl ServerStore {
    pub fn append_server(&self, game_id: Game, icon: Pixbuf, name_morpher: Arc<NameMorpher>, srv: librgs::Server) {
        let iter = self.0.insert_with_values(None, &[], &[]);
        for (i, col) in ServerStoreColumn::enum_iter().enumerate() {
            let insertable: Option<gtk::Value> = match col {
                ServerStoreColumn::Host => Some(From::from(&srv.addr.to_string())),
                ServerStoreColumn::NeedPass => Some(From::from(&srv.need_pass.unwrap_or(false))),
                ServerStoreColumn::LockIcon => {
                    if srv.need_pass.unwrap_or(false) {
                        Some(From::from("network-wireless-encrypted-symbolic"))
                    } else {
                        None
                    }
                }
                ServerStoreColumn::PlayerCount => Some(From::from(&srv.num_clients.unwrap_or(0))),
                ServerStoreColumn::PlayerLimit => Some(From::from(&srv.max_clients.unwrap_or(0))),
                ServerStoreColumn::Ping => Some(From::from(
                    &srv.ping
                        .map(|dur| dur.as_secs() * 1000 + dur.subsec_nanos() as u64 / 1000000)
                        .unwrap_or(9999),
                )),
                ServerStoreColumn::Secure => Some(From::from(&srv.secure.unwrap_or(false))),
                ServerStoreColumn::SecureIcon => {
                    if srv.secure.unwrap_or(false) {
                        Some(From::from("security-high-symbolic"))
                    } else {
                        None
                    }
                }
                ServerStoreColumn::Country => Some(From::from(&format!("{:?}", srv.country.clone()))),
                ServerStoreColumn::Name => Some(From::from(&name_morpher.morph(srv.name.clone().unwrap_or_else(Default::default)))),
                ServerStoreColumn::GameId => Some(From::from(&game_id.id().clone())),
                ServerStoreColumn::GameIcon => Some(From::from(&icon.clone())),
                _ => None,
            };

            if let Some(v) = insertable {
                self.0.set_value(&iter, i as u32, &v);
            }
        }
    }

    pub fn get_server(&self, iter: &TreeIter) -> Fallible<SelectionData> {
        let addr = self
            .0
            .get_value(iter, ServerStoreColumn::Host as i32)
            .get::<String>()
            .ok_or(format_err!("Failed to extract address"))?;
        let game_id = Game::from_id(
            &self
                .0
                .get_value(iter, ServerStoreColumn::GameId as i32)
                .get::<String>()
                .ok_or(format_err!("Failed to extract game ID"))?,
        )
        .unwrap();
        let need_pass = self
            .0
            .get_value(iter, ServerStoreColumn::NeedPass as i32)
            .get::<bool>()
            .ok_or(format_err!("Failed to extract need_pass"))?;

        Ok(SelectionData { game_id, addr, need_pass })
    }
}
