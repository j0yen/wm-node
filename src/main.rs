use clap::{Parser, Subcommand};
use std::process;
use wm_node::{Node, Placement};

fn main() {
    // SIGPIPE safety: prevent panic on broken pipe (e.g. `wm-node id | head`)
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::Id => cmd_id(),
        Commands::Role { role } => cmd_role(&role),
        Commands::ShouldRun { daemon } => cmd_should_run(&daemon),
        Commands::Env => cmd_env(),
    };
    process::exit(exit_code);
}

#[derive(Parser)]
#[command(name = "wm-node", about = "Node identity + placement for the wintermute fleet")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    let placement = match Placement::load() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("wm-node should-run: error loading placement: {e}");
            return 1;
        }
    };
    if placement.should_run(daemon, &node.name) {
        0
    } else {
        1
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
