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

## Usage

```sh
wm-node id                  # prints node name, falls back to hostname
wm-node role <role>         # exit 0 if this node has the role, 1 otherwise
wm-node should-run <daemon> # exit 0 if daemon is assigned here
wm-node env                 # emit WM_NODE= / WM_ROLES= for EnvironmentFile=
```

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
