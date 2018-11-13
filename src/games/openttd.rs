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

            cmd
        })
    }
}
