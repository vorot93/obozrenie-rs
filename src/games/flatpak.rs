use super::{Game, LaunchData};

use std::process::Command;
use std::sync::Arc;

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

#[derive(Clone)]
pub struct Launcher {
    pub id_source: Arc<dyn FlatpakIdentifiable>,
}

impl super::Launcher for Launcher {
    fn launch_cmd(&self, _data: &LaunchData) -> Option<Command> {
        self.id_source.id().map(|flatpak_id| {
            let mut cmd = Command::new("flatpak");

            cmd.arg("run");

            cmd.arg(format!("{}/x86_64/stable", flatpak_id));

            cmd
        })
    }
}
