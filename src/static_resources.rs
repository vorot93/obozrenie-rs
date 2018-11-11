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

/// Used to normalize server name.
pub trait NameMorpher {
    fn morph(&self, v: String) -> String;
}

#[derive(Clone, Debug)]
pub struct EmptyMorpher;

impl NameMorpher for EmptyMorpher {
    fn morph(&self, v: String) -> String {
        v
    }
}

/// Scrubs color codes off the server names
#[derive(Clone)]
pub struct QuakeMorpher {
    regex: Regex,
}

impl Default for QuakeMorpher {
    fn default() -> Self {
        Self {
            regex: Regex::new("[\\^](.)").unwrap(),
        }
    }
}

impl NameMorpher for QuakeMorpher {
    fn morph(&self, v: String) -> String {
        self.regex.replace_all(&v, "").into_owned()
    }
}

#[derive(Clone)]
pub struct GameEntry {
    pub icon: Pixbuf,
    pub launcher_fn: Arc<Fn(LaunchData) -> Option<Command> + Send + Sync>,
    pub name_morpher: Arc<NameMorpher>,
    pub query_fn: Arc<Fn() -> Box<Stream<Item = librgs::Server, Error = failure::Error> + Send> + Send + Sync>,
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

    pub fn flatpak_id(self) -> Option<&'static str> {
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
                Arc::new(DummyPinger) as Arc<Pinger>
            });

        let resolver = Arc::new(tokio_dns::CpuPoolResolver::new(16)) as Arc<Resolver>;

        GameList(
            Game::enum_iter()
                .enumerate()
                .map(|(i, id)| {
                    (
                        id,
                        GameEntry {
                            icon: icon_source.get_icon(id),
                            launcher_fn: Arc::new(move |data| {
                                id.flatpak_id().and_then(|flatpak_id| {
                                    let mut cmd = Command::new("flatpak");

                                    cmd.arg("run");

                                    cmd.arg(format!("{}/x86_64/stable", flatpak_id));

                                    match id {
                                        Game::OpenArena | Game::QuakeIII | Game::Xonotic => {
                                            cmd.arg("+connect");
                                            cmd.arg(data.addr);

                                            if let Some(password) = data.password {
                                                cmd.arg("+password");
                                                cmd.arg(password);
                                            }
                                        }
                                        Game::OpenTTD => {
                                            cmd.arg("-n");
                                            cmd.arg(data.addr);
                                        }
                                        _ => unreachable!(),
                                    }

                                    Some(cmd)
                                })
                            }),
                            name_morpher: match id {
                                Game::QuakeIII | Game::OpenArena => Arc::new(QuakeMorpher::default()),
                                _ => Arc::new(EmptyMorpher),
                            },
                            query_fn: {
                                let resolver = resolver.clone();
                                let pinger = pinger.clone();
                                match id {
                                    Game::RigsOfRods => Arc::new(move || {
                                        Box::new(RigsOfRodsQuery::new(
                                            "http://multiplayer.rigsofrods.org/server-list",
                                            resolver.clone(),
                                            pinger.clone(),
                                        ))
                                    }),
                                    _ => Arc::new({
                                        move || {
                                            let protocols = librgs::protocols::make_default_protocols();

                                            let (protocol, masters) = match id {
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
                                                                    proto.rule_names.insert(
                                                                        librgs::protocols::q3s::Rule::ServerName,
                                                                        "hostname".into(),
                                                                    );
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

                                            let mut query_builder = librgs::UdpQueryBuilder::default();

                                            query_builder = query_builder.with_pinger(pinger.clone());

                                            let socket = UdpSocket::bind(&format!("[::]:{}", starting_port + i).parse().unwrap()).unwrap();
                                            let mut q = query_builder.build(socket);

                                            for entry in masters {
                                                q.start_send(librgs::UserQuery {
                                                    protocol: protocol.clone(),
                                                    host: entry.into(),
                                                })
                                                .unwrap();
                                            }

                                            Box::new(q.map(|e| e.data))
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
