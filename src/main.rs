use clap::{Parser, Subcommand, ValueEnum};
use std::process;
use wm_node::{DoctorReport, Fleet, Node, Placement, ShouldRun};

fn main() {
    // SIGPIPE safety: prevent panic on broken pipe (e.g. `wm-node id | head`)
    #[cfg(unix)]
    // SAFETY: `libc::signal` is FFI but sound here — SIGPIPE/SIG_DFL are valid `c_int` constants (no pointer/lifetime involved), and this runs once at process start before any other thread exists, so there is no concurrent-signal-table race.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::Id => cmd_id(),
        Commands::Role { role } => cmd_role(&role),
        Commands::ShouldRun { daemon } => cmd_should_run(&daemon),
        Commands::Env => cmd_env(),
        Commands::Assignments { format } => cmd_assignments(format),
        Commands::Doctor { format, fix } => cmd_doctor(format, fix),
    };
    process::exit(exit_code);
}

#[derive(Parser)]
#[command(name = "wm-node", about = "Node identity + placement for the wintermute fleet")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// Print the node name (falls back to hostname if node.toml absent)
    Id,
    /// Exit 0 if this node has the given role, 1 otherwise
    Role {
        /// Role to test (voice, hub, builder, …)
        role: String,
    },
    /// Exit 0 if this daemon is assigned to this node in placement.toml
    ShouldRun {
        /// Daemon name to check
        daemon: String,
    },
    /// Emit WM_NODE= / WM_ROLES= lines for use as systemd EnvironmentFile
    Env,
    /// Print every placement.toml entry — "what runs where", fleet-wide
    Assignments {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Check whether this node's enabled systemd units match placement.toml
    Doctor {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        /// Print (never run) the systemctl commands that would converge this node
        #[arg(long)]
        fix: bool,
    },
}

fn cmd_id() -> i32 {
    match Node::load() {
        Ok(node) => {
            println!("{}", node.name);
            0
        }
        Err(e) => {
            eprintln!("wm-node id: error: {e}");
            1
        }
    }
}

fn cmd_role(role: &str) -> i32 {
    match Node::load() {
        Ok(node) => {
            if node.has_role(role) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("wm-node role: error: {e}");
            1
        }
    }
}

fn cmd_should_run(daemon: &str) -> i32 {
    let node = match Node::load() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("wm-node should-run: error loading node: {e}");
            return 1;
        }
    };
    let path = wm_node::placement_config_path();
    match wm_node::should_run_status(&path, daemon, &node.name) {
        Ok(ShouldRun::Yes) => 0,
        Ok(ShouldRun::No) => 1,
        Ok(ShouldRun::MissingPlacement) => {
            eprintln!(
                "wm-node should-run: warning: {} not found; permissive (exit 0)",
                path.display()
            );
            0
        }
        Err(e) => {
            eprintln!("wm-node should-run: error loading placement: {e}");
            1
        }
    }
}

fn cmd_env() -> i32 {
    match Node::load() {
        Ok(node) => {
            print!("{}", node.env_lines());
            0
        }
        Err(e) => {
            eprintln!("wm-node env: error: {e}");
            1
        }
    }
}

fn cmd_assignments(format: Format) -> i32 {
    let node = match Node::load() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("wm-node assignments: error loading node: {e}");
            return 1;
        }
    };
    let placement = match Placement::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("wm-node assignments: error loading placement: {e}");
            return 1;
        }
    };
    let fleet = match Fleet::load() {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "wm-node assignments: warning: error loading fleet.toml, ignoring: {e}"
            );
            Fleet::default()
        }
    };

    let assignments = placement.assignments(&node.name, &fleet);

    match format {
        Format::Json => match serde_json::to_string_pretty(&assignments) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("wm-node assignments: error serializing json: {e}");
                return 1;
            }
        },
        Format::Text => {
            if assignments.is_empty() {
                println!("(no placement entries)");
            }
            for a in &assignments {
                let marker = if a.local { "*" } else { " " };
                let mut suffix = String::new();
                if a.may_be_off {
                    suffix.push_str(" (may be off)");
                }
                if a.unknown_node {
                    suffix.push_str(" (unknown-node)");
                }
                println!("{marker} {:<24} -> {}{}", a.job, a.node, suffix);
            }
        }
    }
    0
}

fn cmd_doctor(format: Format, fix: bool) -> i32 {
    let node = match Node::load() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("wm-node doctor: error loading node: {e}");
            return 1;
        }
    };

    let placement_path = wm_node::placement_config_path();
    if !placement_path.exists() {
        let report = DoctorReport {
            node: node.name.clone(),
            placement_missing: true,
            ..Default::default()
        };
        print_doctor_report(&report, format, fix, Some(&placement_path));
        return report.exit_code();
    }

    let placement = match Placement::load_from(&placement_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("wm-node doctor: error parsing {}: {e}", placement_path.display());
            return 2;
        }
    };

    let fleet = match Fleet::load() {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "wm-node doctor: warning: error loading fleet.toml, ignoring: {e}"
            );
            Fleet::default()
        }
    };

    let enabled_jobs = match probe_enabled_units() {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln!(
                "wm-node doctor: warning: could not probe systemd units, assuming none enabled: {e}"
            );
            Vec::new()
        }
    };

    let report = wm_node::doctor_report(&placement, &fleet, &node.name, &enabled_jobs);
    print_doctor_report(&report, format, fix, None);
    report.exit_code()
}

fn print_doctor_report(
    report: &DoctorReport,
    format: Format,
    fix: bool,
    missing_path: Option<&std::path::Path>,
) {
    match format {
        Format::Json => match serde_json::to_string_pretty(report) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("wm-node doctor: error serializing json: {e}"),
        },
        Format::Text => {
            if report.placement_missing {
                let path = missing_path
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                println!("placement.toml missing at {path} — cannot verify placement here");
            } else if report.misplaced_enabled.is_empty()
                && report.assigned_but_disabled.is_empty()
            {
                println!("healthy: {} job(s) assigned and enabled here", report.healthy.len());
            } else {
                for m in &report.misplaced_enabled {
                    match &m.assigned_to {
                        Some(n) => println!("misplaced-enabled: {} (assigned to {n}, enabled here)", m.job),
                        None => println!("misplaced-enabled: {} (no placement entry, enabled here)", m.job),
                    }
                }
                for job in &report.assigned_but_disabled {
                    println!("assigned-but-disabled: {job} (assigned here, no enabled unit)");
                }
            }
            for u in &report.unknown_node {
                println!("unknown-node: {} assigned to {} (not in fleet.toml)", u.job, u.node);
            }
        }
    }

    if fix {
        let cmds = report.fix_commands();
        if cmds.is_empty() {
            println!("--fix: nothing to converge");
        } else {
            println!("--fix: run the following to converge this node (not executed):");
            for c in cmds {
                println!("  {c}");
            }
        }
    }
}

/// Probe locally-enabled systemd units for the fleet's job-gated services/timers, mapped
/// to job names via `unit_to_job`. Injectable for tests/CI where `systemctl --user` may not
/// be available: set `WM_NODE_DOCTOR_FAKE_UNITS` to a comma-separated list of job names
/// (already-mapped, not raw unit names) to bypass the real probe entirely.
fn probe_enabled_units() -> anyhow::Result<Vec<String>> {
    if let Ok(fake) = std::env::var("WM_NODE_DOCTOR_FAKE_UNITS") {
        return Ok(fake
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect());
    }

    let output = std::process::Command::new("systemctl")
        .args([
            "--user",
            "list-unit-files",
            "--state=enabled",
            "--no-legend",
            "claude-*",
            "fleet-*",
            "wm-*",
        ])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "systemctl exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(wm_node::unit_to_job)
        .collect())
}
