use enum_iter::EnumIterator;
use futures::prelude::*;
use gdk_pixbuf::Pixbuf;
use librgs::{
    dns::Resolver,
    ping::{DummyPinger, Pinger},
};
use log::warn;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::process::Command;
use std::sync::Arc;
use tokio_core::reactor::Core;

mod flatpak;
mod openttd;
mod quake;
mod rgs;
mod rigsofrods;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIterator)]
pub enum Game {
    OpenArena,
    OpenTTD,
    QuakeIII,
    RigsOfRods,
    Xonotic,
}

impl Game {
    pub fn id(self) -> &'static str {
        match self {
            Game::OpenArena => "openarena",
            Game::OpenTTD => "openttd",
            Game::QuakeIII => "q3a",
            Game::RigsOfRods => "rigsofrods",
            Game::Xonotic => "xonotic",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "openarena" => Game::OpenArena,
            "openttd" => Game::OpenTTD,
            "q3a" => Game::QuakeIII,
            "rigsofrods" => Game::RigsOfRods,
            "xonotic" => Game::Xonotic,
            _ => {
                return None;
            }
        })
    }
}

impl Display for Game {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use self::Game::*;

        write!(
            f,
            "{}",
            match self {
                OpenArena => "OpenArena",
                OpenTTD => "OpenTTD",
                QuakeIII => "Quake III Arena",
                RigsOfRods => "Rigs of Rods",
                Xonotic => "Xonotic",
            }
        )
    }
}

pub trait Querier: Send + Sync {
    fn query(&self) -> Box<dyn Stream<Item = librgs::Server, Error = failure::Error> + Send>;
}

/// Used to normalize server name.
pub trait NameMorpher {
    fn morph(&self, v: String) -> String {
        v
    }
}

#[derive(Clone, Debug)]
pub struct DummyMorpher;
impl NameMorpher for DummyMorpher {}

#[derive(Clone, Debug)]
pub struct LaunchData {
    pub addr: String,
    pub password: Option<String>,
}

pub trait Launcher: Send + Sync {
    fn launch_cmd(&self, _data: &LaunchData) -> Option<Command> {
        None
    }
}

#[derive(Clone)]
pub struct DummyLauncher;
impl Launcher for DummyLauncher {}

pub trait GameIconSource {
    fn get_icon(&self, game: Game) -> Pixbuf;
}

#[derive(Clone)]
pub struct GameEntry {
    /// Game's icon
    pub icon: Pixbuf,
    /// Fetches server list for this game
    pub querier: Arc<dyn Querier>,
    /// Adapts server name for the server list
    pub name_morpher: Arc<dyn NameMorpher>,
    /// Launch command builder
    pub launcher: Arc<dyn Launcher>,
}

#[derive(Clone)]
pub struct GameList(pub HashMap<Game, GameEntry>);

impl GameList {
    pub fn new(icon_source: &dyn GameIconSource) -> GameList {
        let starting_port = 5600;
        let pinger = Core::new()
            .unwrap()
            .run(tokio_ping::Pinger::new())
            .map(|pinger| Arc::new(pinger) as Arc<dyn Pinger>)
            .unwrap_or_else(|e| {
                warn!("Failed to spawn pinger: {}. Using manual latency measurement.", e);
                Arc::new(DummyPinger) as Arc<dyn Pinger>
            });

        let resolver = Arc::new(tokio_dns::CpuPoolResolver::new(16)) as Arc<dyn Resolver>;

        GameList(
            Game::enum_iter()
                .enumerate()
                .map(|(i, id)| {
                    (
                        id,
                        GameEntry {
                            icon: icon_source.get_icon(id),
                            launcher: {
                                let flatpak_launcher = flatpak::Launcher { id_source: Arc::new(id) };
                                match id {
                                    Game::QuakeIII | Game::Xonotic | Game::OpenArena => Arc::new(quake::Launcher { flatpak_launcher }),
                                    Game::OpenTTD => Arc::new(openttd::Launcher { flatpak_launcher }),
                                    _ => Arc::new(DummyLauncher),
                                }
                            },
                            name_morpher: match id {
                                Game::QuakeIII | Game::OpenArena => Arc::new(quake::NameMorpher::default()),
                                _ => Arc::new(DummyMorpher),
                            },
                            querier: {
                                let resolver = resolver.clone();
                                let pinger = pinger.clone();
                                match id {
                                    Game::RigsOfRods => Arc::new(rigsofrods::Querier {
                                        master_addr: "http://multiplayer.rigsofrods.org/server-list".into(),
                                        resolver,
                                        pinger,
                                    }),
                                    _ => Arc::new({
                                        let protocols = librgs::protocols::make_default_protocols();

                                        let (protocol, mut master_servers) = match id {
                                            Game::OpenArena => (
                                                {
                                                    let version = 71 as u32;
                                                    librgs::protocols::q3m::ProtocolImpl {
                                                        q3s_protocol: Some(
                                                            {
                                                                let mut proto = librgs::protocols::q3s::ProtocolImpl {
                                                                    version,
                                                                    ..Default::default()
                                                                };
                                                                proto
                                                                    .rule_names
                                                                    .insert(librgs::protocols::q3s::Rule::Mod, "gamename".into());
                                                                proto.server_filter = librgs::protocols::q3s::ServerFilter(Arc::new(
                                                                    |srv: librgs::Server| {
                                                                        if let Some(ver) = srv.rules.get("version") {
                                                                            if let Value::String(ver) = ver {
                                                                                if ver.starts_with("ioq3+oa") {
                                                                                    return Some(srv.clone());
                                                                                }
                                                                            }
                                                                        }
                                                                        None
                                                                    },
                                                                ));
                                                                proto
                                                            }
                                                            .into(),
                                                        ),
                                                        version,
                                                        ..Default::default()
                                                    }
                                                    .into()
                                                },
                                                vec![
                                                    ("master3.idsoftware.com", 27950),
                                                    ("master.ioquake3.org", 27950),
                                                    ("dpmaster.deathmask.net", 27950),
                                                ],
                                            ),
                                            Game::OpenTTD => (protocols["openttdm"].clone(), vec![("master.openttd.org", 3978)]),
                                            Game::QuakeIII => (protocols["q3m"].clone(), vec![("master3.idsoftware.com", 27950)]),
                                            Game::Xonotic => (
                                                {
                                                    let version = 3 as u32;
                                                    librgs::protocols::q3m::ProtocolImpl {
                                                        request_tag: Some("Xonotic".to_string()),
                                                        version,
                                                        q3s_protocol: Some(
                                                            {
                                                                let mut proto = librgs::protocols::q3s::ProtocolImpl::default();
                                                                proto
                                                                    .rule_names
                                                                    .insert(librgs::protocols::q3s::Rule::ServerName, "hostname".into());
                                                                proto
                                                                    .rule_names
                                                                    .insert(librgs::protocols::q3s::Rule::Mod, "modname".into());
                                                                proto
                                                            }
                                                            .into(),
                                                        ),
                                                    }
                                                }
                                                .into(),
                                                vec![("dpmaster.deathmask.net", 27950)],
                                            ),
                                            _ => unreachable!(),
                                        };

                                        let master_servers =
                                            master_servers.into_iter().map(|(addr, port)| (addr.to_string(), port)).collect();

                                        rgs::Querier {
                                            protocol,
                                            master_servers,
                                            port: starting_port + i as u16,
                                            pinger,
                                            resolver,
                                        }
                                    }),
                                }
                            },
                        },
                    )
                })
                .collect(),
        )
    }
}
