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

use gdk_pixbuf::Pixbuf;
use gio::{resources_register, Error, Resource};
use glib::Bytes;
use gtk;
use std::rc::Rc;

use crate::games;
use crate::widgets;

const RES_ROOT_PATH: &str = "/io/obozrenie";

impl games::GameIconSource for Resource {
    fn get_icon(&self, game: games::Game) -> Pixbuf {
        for format in ["png", "svg"].into_iter() {
            if let Ok(pixbuf) = Pixbuf::new_from_resource_at_scale(
                &format!("{}/game_icons/{}.{}", RES_ROOT_PATH, game.id(), format),
                24,
                24,
                false,
            ) {
                return pixbuf;
            }
        }

        panic!("Failed to load icon for {}", game);
    }
}

pub struct Resources {
    pub game_list: games::GameList,
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
        game_list: games::GameList::new(&resource),
        ui: widgets::UIBuilder {
            inner: gtk::Builder::new_from_resource(&format!("{}/ui/app.ui", RES_ROOT_PATH)),
        },
    });

    Ok(out)
}
