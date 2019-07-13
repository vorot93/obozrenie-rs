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

use super::{flatpak, LaunchData};

use std::process::Command;

#[derive(Clone)]
pub struct Launcher {
    pub flatpak_launcher: flatpak::Launcher,
}

impl super::Launcher for Launcher {
    fn launch_cmd(&self, data: &LaunchData) -> Option<Command> {
        self.flatpak_launcher.launch_cmd(data).map(|mut cmd| {
            cmd.arg("-n");
            cmd.arg(&data.addr);

            if let Some(pass) = data.password.as_ref() {
                cmd.arg("-p");
                cmd.arg(pass);
            }

            cmd
        })
    }
}
