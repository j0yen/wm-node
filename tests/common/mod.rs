// Shared test harness for the `placement` acceptance tests (PRD-wm-node-loop-placement).
//
// Each acceptance test spins up an isolated fake `$HOME` (a tempdir) so tests never touch
// the real `~/.config/wintermute/*.toml` on the machine running them, then invokes the
// real `wm-node` binary as a subprocess. `WM_NODE_DOCTOR_FAKE_UNITS` (consumed by
// `probe_enabled_units` in src/main.rs) is the injectable systemd-unit-probe seam the PRD
// calls for — it lets doctor tests run in any environment, systemd or not.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct FakeHome {
    dir: tempfile::TempDir,
}

impl FakeHome {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".config").join("wintermute")).unwrap();
        Self { dir }
    }

    fn config_dir(&self) -> PathBuf {
        self.dir.path().join(".config").join("wintermute")
    }

    pub fn write_node(&self, name: &str) {
        std::fs::write(
            self.config_dir().join("node.toml"),
            format!("name = \"{name}\"\nroles = []\nfleet = \"wintermute\"\n"),
        )
        .unwrap();
    }

    pub fn write_placement(&self, toml: &str) {
        std::fs::write(self.config_dir().join("placement.toml"), toml).unwrap();
    }

    pub fn write_fleet(&self, toml: &str) {
        std::fs::write(self.config_dir().join("fleet.toml"), toml).unwrap();
    }

    pub fn home(&self) -> &Path {
        self.dir.path()
    }
}

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wm-node"))
}

/// Run `wm-node <args>` with HOME pointed at `home`, no fake systemd units.
pub fn run(home: &FakeHome, args: &[&str]) -> Output {
    run_with_units(home, args, None)
}

/// Run `wm-node <args>` with HOME pointed at `home` and, optionally, the doctor unit probe
/// overridden to report exactly `fake_units` (comma-joined job names) as enabled.
pub fn run_with_units(home: &FakeHome, args: &[&str], fake_units: Option<&str>) -> Output {
    let mut cmd = Command::new(bin_path());
    cmd.args(args);
    cmd.env("HOME", home.home());
    match fake_units {
        Some(units) => {
            cmd.env("WM_NODE_DOCTOR_FAKE_UNITS", units);
        }
        None => {
            cmd.env("WM_NODE_DOCTOR_FAKE_UNITS", "");
        }
    }
    cmd.output().expect("failed to run wm-node binary")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
