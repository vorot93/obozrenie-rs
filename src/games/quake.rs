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
