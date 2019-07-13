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

use std::collections::HashSet;
use std::time::Duration;

use crate::games::Game;

#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub games: HashSet<Game>,
    pub game_mod: String,
    pub game_type: String,
    pub map: String,
    pub max_ping: Duration,
    pub anticheat: Option<bool>,
    pub not_full: bool,
    pub not_empty: bool,
    pub no_password: bool,
}

impl Filters {
    pub fn matches(&self, game: Game, srv: &rgs::models::Server) -> bool {
        if !self.games.is_empty() {
            if !self.games.contains(&game) {
                return false;
            }
        }

        if let Some(v) = srv.mod_name.as_ref() {
            if !v.starts_with(&self.game_mod) {
                return false;
            }
        }

        if let Some(v) = srv.game_type.as_ref() {
            if !v.starts_with(&self.game_type) {
                return false;
            }
        }

        if let Some(v) = srv.map.as_ref() {
            if !v.starts_with(&self.map) {
                return false;
            }
        }

        if self.max_ping > std::time::Duration::from_millis(0) {
            if let Some(value) = srv.ping {
                if value > self.max_ping {
                    return false;
                }
            }
        }

        if let Some(filter) = self.anticheat {
            if let Some(value) = srv.secure {
                if filter != value {
                    return false;
                }
            }
        }

        if self.not_full {
            if let Some(num_clients) = srv.num_clients {
                if let Some(max_clients) = srv.max_clients {
                    if num_clients >= max_clients {
                        return false;
                    }
                }
            }
        }

        if self.not_empty {
            if let Some(num_clients) = srv.num_clients {
                if num_clients == 0 {
                    return false;
                }
            }
        }

        if self.no_password {
            if let Some(need_pass) = srv.need_pass {
                if need_pass {
                    return false;
                }
            }
        }

        true
    }
}
