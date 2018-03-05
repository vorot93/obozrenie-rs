use gdk_pixbuf::Pixbuf;
use gtk;
use gtk::prelude::*;
use librgs;
use librgs::models::*;

const QUAKE_COLOR_CODE_PATTERN: &str = "[\\^](.)";

#[derive(Clone)]
pub struct GameEntry {
    pub name: String,
    pub p: TProtocol,
    pub icon: Pixbuf,
}

#[derive(Clone, Copy, Debug, EnumIterator)]
pub enum ServerStoreColumn {
    Host,
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

pub fn append_server(model: &gtk::ListStore, games: &[GameEntry], entry: librgs::ServerEntry) {
    let (p, v) = entry.into_inner();
    for game in games {
        if game.p == p {
            let iter = model.insert_with_values(None, &[], &[]);
            for (i, col) in ServerStoreColumn::enum_iter().enumerate() {
                let insertable: Option<gtk::Value> = match col {
                    ServerStoreColumn::Host => Some(From::from(&v.addr.to_string())),
                    ServerStoreColumn::NeedPass => Some(From::from(&v.need_pass.unwrap_or(false))),
                    ServerStoreColumn::PlayerCount => Some(From::from(&v.num_clients.unwrap_or(0))),
                    ServerStoreColumn::PlayerLimit => Some(From::from(&v.max_clients.unwrap_or(0))),
                    ServerStoreColumn::Ping => Some(From::from(&v.ping.unwrap_or(9999))),
                    ServerStoreColumn::Secure => Some(From::from(&v.secure.unwrap_or(false))),
                    ServerStoreColumn::Country => Some(From::from(&format!("{:?}", v.country.clone()))),
                    ServerStoreColumn::Name => Some(From::from(&v.name.clone().unwrap_or_else(Default::default))),
                    ServerStoreColumn::GameId => Some(From::from(&game.name.clone())),
                    ServerStoreColumn::GameIcon => Some(From::from(&game.icon.clone())),
                    _ => None,
                };

                if let Some(v) = insertable {
                    model.set_value(&iter, i as u32, &v);
                }
            }
        }
    }
}
