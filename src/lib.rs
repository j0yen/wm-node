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

/// Default path for the fleet metadata table (expected-down annotations, known nodes)
pub fn fleet_config_path() -> PathBuf {
    dirs_next().join("fleet.toml")
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

    /// Every placement entry as an `Assignment`, sorted by job name for stable output.
    pub fn assignments(&self, local_node: &str, fleet: &Fleet) -> Vec<Assignment> {
        let mut list: Vec<Assignment> = self
            .table
            .iter()
            .map(|(job, node)| Assignment {
                job: job.clone(),
                node: node.clone(),
                local: node.eq_ignore_ascii_case(local_node),
                may_be_off: fleet.expected_down(node),
                unknown_node: !fleet.is_empty() && !fleet.knows(node),
            })
            .collect();
        list.sort_by(|a, b| a.job.cmp(&b.job));
        list
    }
}

/// Resolution of `wm-node should-run <daemon>` before it becomes an exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShouldRun {
    /// The daemon is assigned to this node — exit 0.
    Yes,
    /// The daemon is assigned elsewhere (or not assigned at all) — exit 1.
    No,
    /// placement.toml does not exist — permissive: exit 0, warn on stderr.
    MissingPlacement,
}

/// Decide should-run status for `daemon` on `node_name`, given an explicit placement.toml
/// path. A missing file is permissive (P1): a half-joined node must not silently kill its
/// jobs. Kept separate from `cmd_should_run` in main.rs so it is testable without touching
/// `$HOME`.
pub fn should_run_status(
    placement_path: &Path,
    daemon: &str,
    node_name: &str,
) -> anyhow::Result<ShouldRun> {
    if !placement_path.exists() {
        return Ok(ShouldRun::MissingPlacement);
    }
    let placement = Placement::load_from(placement_path)?;
    Ok(if placement.should_run(daemon, node_name) {
        ShouldRun::Yes
    } else {
        ShouldRun::No
    })
}

/// One row of `wm-node assignments`: a job, the node it's assigned to, whether that's
/// this node, and whether the assigned node is flagged expected-down in fleet.toml.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assignment {
    pub job: String,
    pub node: String,
    pub local: bool,
    pub may_be_off: bool,
    pub unknown_node: bool,
}

/// Fleet metadata: which nodes exist and which are flagged expected-down (e.g. ryzen7,
/// which may be powered off for days without that being drift). Loaded from
/// `~/.config/wintermute/fleet.toml`:
/// ```toml
/// [nodes.ryzen7]
/// expected_down = true
/// ```
/// Absent file means "no fleet metadata known" — assignments/doctor render without
/// may-be-off or unknown-node annotations rather than failing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Fleet {
    #[serde(default)]
    pub nodes: HashMap<String, FleetNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetNode {
    #[serde(default)]
    pub expected_down: bool,
}

impl Fleet {
    /// Load from the standard fleet config path. Never errors on a missing file.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&fleet_config_path())
    }

    /// Load from an explicit path. A missing file yields an empty (unknown) fleet.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let f: Self = toml::from_str(&text)?;
            Ok(f)
        } else {
            Ok(Self::default())
        }
    }

    /// True if fleet.toml carried no node metadata at all (file absent or empty table) —
    /// used to suppress unknown-node flagging when we simply have no fleet data to check
    /// against, rather than flagging every node as unknown.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// True if `node_name` is a known member of the fleet.
    pub fn knows(&self, node_name: &str) -> bool {
        self.nodes.keys().any(|n| n.eq_ignore_ascii_case(node_name))
    }

    /// True if `node_name` is flagged expected-down (may be legitimately powered off).
    pub fn expected_down(&self, node_name: &str) -> bool {
        self.nodes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(node_name))
            .map(|(_, meta)| meta.expected_down)
            .unwrap_or(false)
    }
}

/// One drift finding: a job whose locally-enabled unit does not match its placement
/// assignment (assigned elsewhere, or the placement entry was deleted entirely).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MisplacedEntry {
    pub job: String,
    /// `None` when the placement entry for this job was deleted outright (unit still
    /// enabled locally with nothing in placement.toml naming it anywhere).
    pub assigned_to: Option<String>,
}

/// A placement entry naming a node that fleet.toml doesn't know about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnknownNodeEntry {
    pub job: String,
    pub node: String,
}

/// `wm-node doctor`'s full findings for the node it ran on.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DoctorReport {
    pub node: String,
    /// Jobs assigned here with an enabled unit here — no action needed.
    pub healthy: Vec<String>,
    /// Case (a): a unit is enabled here for a job assigned to a different node (or to no
    /// node at all, if the placement entry was deleted).
    pub misplaced_enabled: Vec<MisplacedEntry>,
    /// Case (b): a job assigned here has no enabled unit here.
    pub assigned_but_disabled: Vec<String>,
    /// Placement entries naming a node fleet.toml doesn't recognize.
    pub unknown_node: Vec<UnknownNodeEntry>,
    /// True when placement.toml itself was missing (reported loudly per P1; forces a
    /// non-zero exit even though there is nothing else to report).
    pub placement_missing: bool,
}

impl DoctorReport {
    /// Exit 1 when there's drift (case a or b) or placement.toml itself was missing;
    /// exit 0 when everything assigned here is enabled here and nothing stray is enabled.
    /// (Duplicate-key parse errors are handled before a report is ever built — see
    /// `should_run_status`'s sibling call site in main.rs, which exits 2 instead.)
    pub fn exit_code(&self) -> i32 {
        if self.placement_missing
            || !self.misplaced_enabled.is_empty()
            || !self.assigned_but_disabled.is_empty()
        {
            1
        } else {
            0
        }
    }
}

/// Build a `DoctorReport` from already-loaded placement/fleet tables and the set of job
/// names this node currently has an enabled systemd unit for. `enabled_jobs_here` is the
/// probe's *output*, not the probe itself — this keeps the classification logic testable
/// without shelling out to systemctl (the fixture pattern used elsewhere in this crate).
pub fn doctor_report(
    placement: &Placement,
    fleet: &Fleet,
    node_name: &str,
    enabled_jobs_here: &[String],
) -> DoctorReport {
    let mut report = DoctorReport {
        node: node_name.to_string(),
        ..Default::default()
    };

    // Case (a) + healthy: walk every locally-enabled job.
    for job in enabled_jobs_here {
        match placement.placement_of(job) {
            Some(assigned) if assigned.eq_ignore_ascii_case(node_name) => {
                report.healthy.push(job.clone());
            }
            Some(assigned) => {
                report.misplaced_enabled.push(MisplacedEntry {
                    job: job.clone(),
                    assigned_to: Some(assigned.to_string()),
                });
            }
            None => {
                report.misplaced_enabled.push(MisplacedEntry {
                    job: job.clone(),
                    assigned_to: None,
                });
            }
        }
    }

    // Case (b): jobs assigned here with no enabled unit here.
    for (job, node) in placement.table.iter() {
        if node.eq_ignore_ascii_case(node_name)
            && !enabled_jobs_here.iter().any(|j| j == job)
        {
            report.assigned_but_disabled.push(job.clone());
        }
    }

    // unknown-node: any placement entry naming a node fleet.toml doesn't recognize
    // (skipped entirely when fleet.toml carries no data — nothing to check against).
    if !fleet.is_empty() {
        for (job, node) in placement.table.iter() {
            if !fleet.knows(node) {
                report.unknown_node.push(UnknownNodeEntry {
                    job: job.clone(),
                    node: node.clone(),
                });
            }
        }
    }

    report.healthy.sort();
    report
        .misplaced_enabled
        .sort_by(|a, b| a.job.cmp(&b.job));
    report.assigned_but_disabled.sort();
    report.unknown_node.sort_by(|a, b| a.job.cmp(&b.job));
    report
}

impl DoctorReport {
    /// The exact `systemctl --user` commands that would converge this node — P2's
    /// `doctor --fix`. Printed, never executed.
    pub fn fix_commands(&self) -> Vec<String> {
        let mut cmds = Vec::new();
        for m in &self.misplaced_enabled {
            let reason = match &m.assigned_to {
                Some(n) => format!("assigned to {n}"),
                None => "no longer assigned anywhere".to_string(),
            };
            cmds.push(format!(
                "systemctl --user disable --now {}.timer  # {reason}",
                m.job
            ));
        }
        for job in &self.assigned_but_disabled {
            cmds.push(format!(
                "systemctl --user enable --now {job}.timer  # assigned here per placement.toml"
            ));
        }
        cmds
    }
}

/// Map a systemd unit file name (as printed by `systemctl --user list-unit-files`) to the
/// job name it should be gated on in placement.toml. Strips the `.service`/`.timer`
/// suffix and a leading `claude-` prefix (the vibeloop units are `claude-vibeloop-tick.timer`
/// for job `vibeloop-tick`); `fleet-*` and `wm-*` units are already named after their job
/// (`fleet-janitor.timer` for job `fleet-janitor`) so no prefix is stripped for those.
pub fn unit_to_job(unit_name: &str) -> String {
    let base = unit_name
        .strip_suffix(".service")
        .or_else(|| unit_name.strip_suffix(".timer"))
        .unwrap_or(unit_name);
    base.strip_prefix("claude-").unwrap_or(base).to_string()
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

    // Edge case (PRD-wm-node-loop-placement): a placement entry naming a node fleet.toml
    // doesn't know about is flagged unknown-node, but only once fleet.toml has any data
    // at all — an absent/empty fleet.toml must not flag every node as unknown.
    #[test]
    fn doctor_flags_unknown_node_when_fleet_toml_has_data() {
        let placement_file = write_temp("stray-job = \"nonexistent-node\"\n");
        let placement = Placement::load_from(placement_file.path()).unwrap();
        let fleet_file = write_temp("[nodes.redbaron]\nexpected_down = false\n");
        let fleet = Fleet::load_from(fleet_file.path()).unwrap();

        let report = doctor_report(&placement, &fleet, "redbaron", &[]);
        assert_eq!(report.unknown_node.len(), 1);
        assert_eq!(report.unknown_node[0].job, "stray-job");
        assert_eq!(report.unknown_node[0].node, "nonexistent-node");
    }

    #[test]
    fn doctor_skips_unknown_node_check_when_fleet_toml_absent() {
        let placement_file = write_temp("stray-job = \"nonexistent-node\"\n");
        let placement = Placement::load_from(placement_file.path()).unwrap();
        let fleet = Fleet::load_from(Path::new("/nonexistent/fleet.toml")).unwrap();

        let report = doctor_report(&placement, &fleet, "redbaron", &[]);
        assert!(report.unknown_node.is_empty());
    }

    #[test]
    fn unit_to_job_strips_claude_prefix_and_suffix() {
        assert_eq!(unit_to_job("claude-vibeloop-tick.timer"), "vibeloop-tick");
        assert_eq!(unit_to_job("claude-vibeloop-tick.service"), "vibeloop-tick");
        assert_eq!(unit_to_job("fleet-janitor.timer"), "fleet-janitor");
        assert_eq!(unit_to_job("wm-brain.service"), "wm-brain");
    }

    #[test]
    fn should_run_status_is_permissive_on_missing_placement() {
        let status =
            should_run_status(Path::new("/nonexistent/placement.toml"), "anything", "redbaron")
                .unwrap();
        assert_eq!(status, ShouldRun::MissingPlacement);
    }

    #[test]
    fn fix_commands_cover_both_drift_directions() {
        let mut report = DoctorReport {
            node: "redbaron".to_string(),
            ..Default::default()
        };
        report.misplaced_enabled.push(MisplacedEntry {
            job: "vibeloop-tick".to_string(),
            assigned_to: Some("carbon".to_string()),
        });
        report.assigned_but_disabled.push("vibeloop-measure".to_string());

        let cmds = report.fix_commands();
        assert!(cmds.iter().any(|c| c.contains("disable") && c.contains("vibeloop-tick")));
        assert!(cmds.iter().any(|c| c.contains("enable") && c.contains("vibeloop-measure")));
    }
}
