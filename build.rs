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
