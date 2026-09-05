# wm-node

Node identity + placement CLI for the **wintermute** fleet.

## TL;DR

No node identity existed anywhere in wintermute — daemons "belonged" to a
machine only because that is where they happened to be installed. `wm-node`
fixes this by providing a small declarative identity layer: a node has a
**name** and one or more **roles** (`voice`, `hub`, `builder`), and a placement
table maps daemons to their assigned node. Systemd units can gate themselves
with `ExecCondition=wm-node role hub` and placement with
`ExecCondition=wm-node should-run homeward-ingest`.

## Configuration

**`~/.config/wintermute/node.toml`**
```toml
name = "carbon"           # WM_NODE — short machine name
roles = ["voice"]         # any of: voice, hub, builder
fleet = "wintermute"
```

**`~/.config/wintermute/placement.toml`**
```toml
homeward-ingest = "hub"
wm-brain        = "carbon"
```

**`~/.config/wintermute/fleet.toml`** (optional — powers `assignments`'
may-be-off annotation and `doctor`'s unknown-node check; absent means "no
fleet metadata known" rather than an error)
```toml
[nodes.ryzen7]
expected_down = true   # may be legitimately powered off — not drift

[nodes.redbaron]
expected_down = false
```

## Usage

```sh
wm-node id                        # prints node name, falls back to hostname
wm-node role <role>                # exit 0 if this node has the role, 1 otherwise
wm-node should-run <daemon>        # exit 0 if daemon is assigned here
wm-node env                        # emit WM_NODE= / WM_ROLES= for EnvironmentFile=
wm-node assignments [--format json]        # "what runs where", fleet-wide
wm-node doctor [--format json] [--fix]     # does reality match placement.toml, here
```

### `assignments`

Prints every `placement.toml` entry with its assigned node, marking entries
assigned to the local node and annotating entries assigned to a node flagged
`expected_down` in `fleet.toml` (`may be off`) or to a node `fleet.toml`
doesn't know about (`unknown-node`).

### `doctor`

Cross-references `placement.toml` against this node's enabled `claude-*` /
`fleet-*` / `wm-*` systemd units (`systemctl --user list-unit-files
--state=enabled`) and reports:

- **misplaced-enabled** — a unit is enabled here for a job assigned to a
  different node (or to no node at all, if the placement entry was deleted).
- **assigned-but-disabled** — a job is assigned here with no enabled unit.
- **healthy** — assigned here and enabled here.

Exits 0 when only `healthy`, 1 when either drift list is non-empty or
`placement.toml` itself is missing, 2 (naming the offending key) when
`placement.toml` has a duplicate job key. `--fix` prints the exact
`systemctl --user enable/disable` commands that would converge the node —
it never runs them.

For tests or systemd-less environments, `WM_NODE_DOCTOR_FAKE_UNITS` (a
comma-separated list of job names) overrides the real unit probe.

### systemd integration

```ini
[Service]
ExecCondition=wm-node role hub
EnvironmentFile=%h/.cache/wm-node-env
```

Generate the env file at boot:
```sh
wm-node env > ~/.cache/wm-node-env
```

## Acceptance criteria

1. `wm-node id` prints `name` from `node.toml`; falls back to hostname when absent.
2. `wm-node role hub` exits 0 on a hub node, 1 otherwise — verified with two fixture files.
3. `wm-node should-run homeward-ingest` consults `placement.toml`; exits 0 only on the assigned node.
4. `wm-node env` output is valid `KEY=VALUE` for systemd `EnvironmentFile=`, asserted by a parse test.
5. `cargo test --release` green; binary installs to `~/.local/bin/wm-node`; no SIGPIPE panic.
6. `wm-node assignments [--format json]` lists every placement entry with its node and a local-assignment boolean.
7. `wm-node doctor [--format json]` flags units enabled here for jobs assigned elsewhere (misplaced-enabled) and jobs assigned here with no enabled unit (assigned-but-disabled), exiting 1 on either; reports healthy and exits 0 otherwise.
8. `wm-node should-run` is permissive (exit 0, stderr warning) when `placement.toml` is absent; `doctor` reports the same absence loudly and exits 1.
9. `doctor` exits 2, naming the key, when `placement.toml` has a duplicate job key.
10. `wm-node doctor --fix` prints the exact converging `systemctl --user enable/disable` commands and executes none of them.

See `PRD-wm-node-loop-placement` for the full acceptance criteria this build
implements (`tests/placement_ac*.rs`).

## Install

```sh
cargo install --path .
```

or copy the pre-built binary:

```sh
cp target/release/wm-node ~/.local/bin/
```

## License

MIT — Joe Yen
