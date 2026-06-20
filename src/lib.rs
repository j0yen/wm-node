use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default path for the node identity file
pub fn node_config_path() -> PathBuf {
    dirs_next().join("node.toml")
}

/// Default path for the placement table
pub fn placement_config_path() -> PathBuf {
    dirs_next().join("placement.toml")
}

fn dirs_next() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("wintermute")
}

/// Node identity loaded from `~/.config/wintermute/node.toml`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Short name for this node (e.g. "carbon", "ryzen7")
    pub name: String,
    /// Roles this node fulfils (e.g. ["voice"], ["hub", "builder"])
    #[serde(default)]
    pub roles: Vec<String>,
    /// Fleet this node belongs to
    #[serde(default = "default_fleet")]
    pub fleet: String,
}

fn default_fleet() -> String {
    "wintermute".to_string()
}

impl Node {
    /// Load from the standard config path, falling back to hostname if absent.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&node_config_path())
    }

    /// Load from an explicit path.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let node: Self = toml::from_str(&text)?;
            Ok(node)
        } else {
            // Fallback: use hostname
            let name = hostname_fallback();
            Ok(Self {
                name,
                roles: vec![],
                fleet: default_fleet(),
            })
        }
    }

    /// Return true if this node has the given role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
    }

    /// Emit `WM_NODE=…` / `WM_ROLES=…` lines suitable for systemd `EnvironmentFile`.
    pub fn env_lines(&self) -> String {
        let roles = self.roles.join(",");
        format!("WM_NODE={}\nWM_ROLES={}\nWM_FLEET={}\n", self.name, roles, self.fleet)
    }
}

fn hostname_fallback() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Placement table loaded from `~/.config/wintermute/placement.toml`
/// Shape: `homeward-ingest = "hub"`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Placement {
    #[serde(flatten)]
    pub table: HashMap<String, String>,
}

impl Placement {
    /// Load from the standard placement config path.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&placement_config_path())
    }

    /// Load from an explicit path.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let p: Self = toml::from_str(&text)?;
            Ok(p)
        } else {
            Ok(Self::default())
        }
    }

    /// Return the assigned node name for `daemon`, if any.
    pub fn placement_of(&self, daemon: &str) -> Option<&str> {
        self.table.get(daemon).map(|s| s.as_str())
    }

    /// Return true if `daemon` is assigned to `node_name`.
    pub fn should_run(&self, daemon: &str, node_name: &str) -> bool {
        match self.placement_of(daemon) {
            Some(assigned) => assigned.eq_ignore_ascii_case(node_name),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    // AC1: wm-node id falls back to hostname when no node.toml
    #[test]
    fn test_id_fallback_to_hostname() {
        let node = Node::load_from(Path::new("/nonexistent/node.toml")).unwrap();
        // Should not panic; name comes from /etc/hostname or "unknown"
        assert!(!node.name.is_empty());
    }

    // AC1: wm-node id reads name from node.toml
    #[test]
    fn test_id_reads_name() {
        let f = write_temp(r#"name = "carbon"
roles = ["voice"]
fleet = "wintermute"
"#);
        let node = Node::load_from(f.path()).unwrap();
        assert_eq!(node.name, "carbon");
    }

    // AC2: wm-node role — has_role returns true when role listed
    #[test]
    fn test_has_role_positive() {
        let f = write_temp(r#"name = "carbon"
roles = ["voice", "hub"]
fleet = "wintermute"
"#);
        let node = Node::load_from(f.path()).unwrap();
        assert!(node.has_role("voice"));
        assert!(node.has_role("hub"));
    }

    // AC2: wm-node role — has_role returns false when role absent
    #[test]
    fn test_has_role_negative() {
        let f = write_temp(r#"name = "ryzen7"
roles = ["builder"]
fleet = "wintermute"
"#);
        let node = Node::load_from(f.path()).unwrap();
        assert!(!node.has_role("voice"));
        assert!(!node.has_role("hub"));
        assert!(node.has_role("builder"));
    }

    // AC3: wm-node should-run — exits 0 when assigned to this node
    #[test]
    fn test_should_run_positive() {
        let f = write_temp(r#"homeward-ingest = "hub"
wm-brain = "carbon"
"#);
        let placement = Placement::load_from(f.path()).unwrap();
        assert!(placement.should_run("homeward-ingest", "hub"));
        assert!(!placement.should_run("homeward-ingest", "carbon"));
    }

    // AC3: wm-node should-run — returns false when not assigned
    #[test]
    fn test_should_run_negative() {
        let f = write_temp(r#"homeward-ingest = "hub"
"#);
        let placement = Placement::load_from(f.path()).unwrap();
        assert!(!placement.should_run("wm-brain", "carbon"));
    }

    // AC4: wm-node env — output is valid KEY=VALUE for systemd EnvironmentFile
    #[test]
    fn test_env_output_format() {
        let node = Node {
            name: "carbon".to_string(),
            roles: vec!["voice".to_string()],
            fleet: "wintermute".to_string(),
        };
        let env = node.env_lines();
        // Must contain KEY=VALUE lines only, parseable by splitting on '='
        for line in env.lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            assert_eq!(parts.len(), 2, "line not KEY=VALUE: {:?}", line);
            let key = parts[0];
            assert!(!key.is_empty(), "key must not be empty");
            // Keys must be alphanumeric + underscore only (systemd requirement)
            assert!(
                key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "key contains invalid chars: {:?}",
                key
            );
            // Values must not contain unquoted newlines (they don't since we produce them)
            assert!(!parts[1].contains('\n'), "value must not contain newline");
        }
        assert!(env.contains("WM_NODE=carbon"));
        assert!(env.contains("WM_ROLES=voice"));
        assert!(env.contains("WM_FLEET=wintermute"));
    }

    // AC5 (structural): sigpipe::reset() is the first statement — verified by reading main.rs
    // The actual sigpipe test is: `wm-node id | head` must not panic.
    // We assert the env lines have no quotes needed (values are plain identifiers).
    #[test]
    fn test_env_no_quotes_needed() {
        let node = Node {
            name: "carbon".to_string(),
            roles: vec!["voice".to_string(), "hub".to_string()],
            fleet: "wintermute".to_string(),
        };
        let env = node.env_lines();
        for line in env.lines() {
            if line.is_empty() {
                continue;
            }
            let val = line.splitn(2, '=').nth(1).unwrap_or("");
            assert!(!val.contains('"'), "value should not need quoting: {:?}", line);
            assert!(!val.contains('\''), "value should not need quoting: {:?}", line);
            assert!(!val.contains(' '), "value should not contain spaces: {:?}", line);
        }
    }
}
