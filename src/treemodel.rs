use static_resources::*;

use gdk_pixbuf::Pixbuf;
use gtk::{self, prelude::*, TreeIter};
use librgs::{self, models::*};
use std::sync::Arc;

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub p: TProtocol,
    pub icon: Pixbuf,
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

pub fn append_server(
    model: &gtk::ListStore,
    game_id: Game,
    icon: Pixbuf,
    name_morpher: Arc<Fn(String) -> String + Send + Sync>,
    srv: librgs::Server,
) {
    let iter = model.insert_with_values(None, &[], &[]);
    for (i, col) in ServerStoreColumn::enum_iter().enumerate() {
        let insertable: Option<gtk::Value> = match col {
            ServerStoreColumn::Host => Some(From::from(&srv.addr.to_string())),
            ServerStoreColumn::NeedPass => Some(From::from(&srv.need_pass.unwrap_or(false))),
            ServerStoreColumn::PlayerCount => Some(From::from(&srv.num_clients.unwrap_or(0))),
            ServerStoreColumn::PlayerLimit => Some(From::from(&srv.max_clients.unwrap_or(0))),
            ServerStoreColumn::Ping => Some(From::from(
                &srv.ping
                    .map(|dur| dur.as_secs() * 1000 + dur.subsec_nanos() as u64 / 1000000)
                    .unwrap_or(9999),
            )),
            ServerStoreColumn::Secure => Some(From::from(&srv.secure.unwrap_or(false))),
            ServerStoreColumn::Country => Some(From::from(&format!("{:?}", srv.country.clone()))),
            ServerStoreColumn::Name => Some(From::from(&(name_morpher)(srv.name.clone().unwrap_or_else(Default::default)))),
            ServerStoreColumn::GameId => Some(From::from(&game_id.id().clone())),
            ServerStoreColumn::GameIcon => Some(From::from(&icon.clone())),
            _ => None,
        };

        if let Some(v) = insertable {
            model.set_value(&iter, i as u32, &v);
        }
    }
}

pub struct SelectionData {
    pub game_id: Game,
    pub addr: String,
    pub need_pass: bool,
}

impl SelectionData {
    pub fn extract(model: &gtk::ListStore, iter: &TreeIter) -> Self {
        let addr = model.get_value(iter, ServerStoreColumn::Host as i32).get::<String>().unwrap();
        let game_id = Game::from_id(&model.get_value(iter, ServerStoreColumn::GameId as i32).get::<String>().unwrap()).unwrap();
        let need_pass = model.get_value(iter, ServerStoreColumn::NeedPass as i32).get::<bool>().unwrap();

        Self { game_id, addr, need_pass }
    }
}
