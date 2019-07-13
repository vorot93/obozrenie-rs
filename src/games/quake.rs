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

use super::LaunchData;

use regex::Regex;
use std::process::Command;

/// Scrubs color codes off the server names
#[derive(Clone)]
pub struct NameMorpher {
    pub scrubbing_pattern: Regex,
}

impl Default for NameMorpher {
    fn default() -> Self {
        Self {
            scrubbing_pattern: Regex::new("[\\^](.)").unwrap(),
        }
    }
}

impl super::NameMorpher for NameMorpher {
    fn morph(&self, v: String) -> String {
        self.scrubbing_pattern.replace_all(&v, "").into_owned()
    }
}

#[derive(Clone)]
pub struct Launcher {
    pub flatpak_launcher: super::flatpak::Launcher,
}

impl super::Launcher for Launcher {
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
