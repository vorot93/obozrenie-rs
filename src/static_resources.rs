use enum_iter::EnumIterator;
use failure;
use futures::prelude::*;
use gdk_pixbuf::Pixbuf;
use gio::{resources_register, Error, Resource};
use glib::Bytes;
use gtk;
use librgs::{
    self,
    dns::Resolver,
    ping::{DummyPinger, Pinger},
};
use log::warn;
use regex::Regex;
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    process::Command,
    rc::Rc,
    sync::Arc,
};
use tokio::net::UdpSocket;
use tokio_core::reactor::Core;
use tokio_dns;
use tokio_ping;

use rigsofrods::*;
use widgets;

const RES_ROOT_PATH: &str = "/io/obozrenie";

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
pub struct FlatpakLauncher {
    pub id_source: Arc<dyn FlatpakIdentifiable>,
}

impl Launcher for FlatpakLauncher {
    fn launch_cmd(&self, _data: &LaunchData) -> Option<Command> {
        self.id_source.id().map(|flatpak_id| {
            let mut cmd = Command::new("flatpak");

            cmd.arg("run");

            cmd.arg(format!("{}/x86_64/stable", flatpak_id));

            cmd
        })
    }
}

#[derive(Clone)]
pub struct QuakeLauncher {
    pub flatpak_launcher: FlatpakLauncher,
}

impl Launcher for QuakeLauncher {
    fn launch_cmd(&self, data: &LaunchData) -> Option<Command> {
        self.flatpak_launcher.launch_cmd(data).map(|mut cmd| {
            cmd.arg("+connect");
            cmd.arg(&data.addr);

            if let Some(password) = data.password.as_ref() {
                cmd.arg("+password");
                cmd.arg(password);
            }

            cmd
        })
    }
}

#[derive(Clone)]
pub struct OpenTTDLauncher {
    pub flatpak_launcher: FlatpakLauncher,
}

impl Launcher for OpenTTDLauncher {
    fn launch_cmd(&self, data: &LaunchData) -> Option<Command> {
        self.flatpak_launcher.launch_cmd(data).map(|mut cmd| {
            cmd.arg("-n");
            cmd.arg(&data.addr);

            cmd
        })
    }
}

#[derive(Clone)]
pub struct DummyLauncher;

impl Launcher for DummyLauncher {}

/// Used to normalize server name.
pub trait NameMorpher {
    fn morph(&self, v: String) -> String;
}

#[derive(Clone, Debug)]
pub struct DummyMorpher;

impl NameMorpher for DummyMorpher {
    fn morph(&self, v: String) -> String {
        v
    }
}

/// Scrubs color codes off the server names
#[derive(Clone)]
pub struct QuakeMorpher {
    scrubbing_pattern: Regex,
}

impl Default for QuakeMorpher {
    fn default() -> Self {
        Self {
            scrubbing_pattern: Regex::new("[\\^](.)").unwrap(),
        }
    }
}

impl NameMorpher for QuakeMorpher {
    fn morph(&self, v: String) -> String {
        self.scrubbing_pattern.replace_all(&v, "").into_owned()
    }
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

pub trait Querier: Send + Sync {
    fn query(&self) -> Box<dyn Stream<Item = librgs::Server, Error = failure::Error> + Send>;
}

#[derive(Clone)]
pub struct RigsOfRodsQuerier {
    pub master_addr: String,
    pub resolver: Arc<dyn Resolver>,
    pub pinger: Arc<dyn Pinger>,
}

impl Querier for RigsOfRodsQuerier {
    fn query(&self) -> Box<dyn Stream<Item = librgs::Server, Error = failure::Error> + Send> {
        Box::new(RigsOfRodsQuery::new(&self.master_addr, self.resolver.clone(), self.pinger.clone()))
    }
}

#[derive(Clone)]
pub struct RGSQuerier {
    pub protocol: librgs::TProtocol,
    pub master_servers: Vec<(String, u16)>,
    pub port: u16,
    pub resolver: Arc<dyn Resolver>,
    pub pinger: Arc<dyn Pinger>,
}

impl Querier for RGSQuerier {
    fn query(&self) -> Box<dyn Stream<Item = librgs::Server, Error = failure::Error> + Send> {
        let mut query_builder = librgs::UdpQueryBuilder::default();

        query_builder = query_builder.with_pinger(self.pinger.clone());

        let socket = UdpSocket::bind(&format!("[::]:{}", self.port).parse().unwrap()).unwrap();
        let mut q = query_builder.build(socket);

        for entry in &self.master_servers {
            q.start_send(librgs::UserQuery {
                protocol: self.protocol.clone(),
                host: entry.clone().into(),
            })
            .unwrap();
        }

        Box::new(q.map(|e| e.data))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIterator)]
pub enum RGSGame {
    OpenArena,
    OpenTTD,
    QuakeIII,
    Xonotic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumIterator)]
pub enum Game {
    OpenArena,
    OpenTTD,
    QuakeIII,
    RigsOfRods,
    Xonotic,
}

impl From<RGSGame> for Game {
    fn from(v: RGSGame) -> Game {
        match v {
            RGSGame::OpenArena => Game::OpenArena,
            RGSGame::OpenTTD => Game::OpenTTD,
            RGSGame::QuakeIII => Game::QuakeIII,
            RGSGame::Xonotic => Game::Xonotic,
        }
    }
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

pub trait FlatpakIdentifiable: Send + Sync {
    fn id(&self) -> Option<&'static str>;
}

impl FlatpakIdentifiable for Game {
    fn id(&self) -> Option<&'static str> {
        match self {
            Game::OpenArena => Some("ws.openarena.OpenArena"),
            Game::OpenTTD => Some("org.openttd.OpenTTD"),
            Game::Xonotic => Some("org.xonotic.Xonotic"),
            _ => None,
        }
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

#[derive(Clone)]
pub struct GameList(pub HashMap<Game, GameEntry>);

pub trait GameIconSource {
    fn get_icon(&self, game: Game) -> Pixbuf;
}

impl GameIconSource for Resource {
    fn get_icon(&self, game: Game) -> Pixbuf {
        for format in ["png", "svg"].into_iter() {
            if let Ok(pixbuf) =
                Pixbuf::new_from_resource_at_scale(&format!("{}/game_icons/{}.{}", RES_ROOT_PATH, game.id(), format), 24, 24, false)
            {
                return pixbuf;
            }
        }

        panic!("Failed to load icon for {}", game);
    }
}

impl GameList {
    pub fn new(icon_source: &dyn GameIconSource) -> GameList {
        let starting_port = 5600;
        let pinger = Core::new()
            .unwrap()
            .run(tokio_ping::Pinger::new())
            .map(|pinger| Arc::new(pinger) as Arc<Pinger>)
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
                                let flatpak_launcher = FlatpakLauncher { id_source: Arc::new(id) };
                                match id {
                                    Game::QuakeIII | Game::Xonotic | Game::OpenArena => Arc::new(QuakeLauncher { flatpak_launcher }),
                                    Game::OpenTTD => Arc::new(OpenTTDLauncher { flatpak_launcher }),
                                    _ => Arc::new(DummyLauncher),
                                }
                            },
                            name_morpher: match id {
                                Game::QuakeIII | Game::OpenArena => Arc::new(QuakeMorpher::default()),
                                _ => Arc::new(DummyMorpher),
                            },
                            querier: {
                                let resolver = resolver.clone();
                                let pinger = pinger.clone();
                                match id {
                                    Game::RigsOfRods => Arc::new(RigsOfRodsQuerier {
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

                                        RGSQuerier {
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

pub struct Resources {
    pub game_list: GameList,
    pub ui: widgets::UIBuilder,
}

pub(crate) fn init() -> Result<Rc<Resources>, Error> {
    // load the gresource binary at build time and include/link it into the final binary.
    let res_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/resources.gresource"));

    // Create Resource, it will live as long the value lives.
    let gbytes = Bytes::from(res_bytes.as_ref());
    let resource = Resource::new_from_data(&gbytes)?;

    // Register the resource so It wont be dropped and will continue to live in memory.
    resources_register(&resource);

    let out = Rc::new(Resources {
        game_list: GameList::new(&resource),
        ui: widgets::UIBuilder {
            inner: gtk::Builder::new_from_resource(&format!("{}/ui/app.ui", RES_ROOT_PATH)),
        },
    });

    Ok(out)
}
