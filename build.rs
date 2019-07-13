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

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    // Compile Gresource
    let out = Command::new("glib-compile-resources")
        .args(&[
            "--target",
            &Path::new(&env::var("OUT_DIR").unwrap())
                .join("resources.gresource")
                .to_string_lossy()
                .to_owned(),
            "resources.xml",
        ])
        .current_dir("res")
        .status()
        .expect("failed to generate resources");
    assert!(out.success());
}
