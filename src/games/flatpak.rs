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
