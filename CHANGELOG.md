# Changelog

## v0.2.1 — 2026-09-06

Repo gate onboarding for wm-node (blocked at HEAD de1b28e: 9/25 receipts,
missing autobuilder scaffolding). This release adds the remaining
gate-required build harness — `rust-toolchain.toml` (pinned 1.85.0),
`deny.toml` (cargo-deny supply-chain policy), `scripts/audit.sh` (BAD_RUST
risk-gate producer), and `.github/workflows/ci.yml` (check/clippy/test/deny/
audit on push and PR) — matching the shape already proven in wm-router/
ac-judge/skill-manifest. `agent/proof-lanes.toml` gains a `ci-infra` lane so
these newly-onboarded paths route correctly under vti-plan, and the `deps`
lane now runs `cargo deny check`. Fixes one real audit finding surfaced by
turning scripts/audit.sh on for the first time: the SIGPIPE-handling
`unsafe` block in `main.rs` gets a `SAFETY:` comment (behavior unchanged).
`agent/intent-card.json` is refreshed from PRD-wm-node-loop-placement so it
describes this crate's real, already-shipped assignments/doctor feature
(v0.2.0) instead of the placeholder card `onboard-repo.sh` generated before
any PRD was known to be the author.

## v0.2.0

wm-node v0.1.0 shipped the placement mechanism (`placement.toml` +
`ExecCondition=wm-node should-run <job>`) and nothing used it: the vibeloop
tick ran on RedBaron because of a local-only script edit no repo records,
janitor and beacon are about to need placement, and there was no way to ask
"what runs where" fleet-wide. This release makes placement the enforced
source of truth for every recurring fleet job and gives it a doctor.

- Add `wm-node assignments [--format json]` — prints every `placement.toml`
  entry with its assigned node, marking entries assigned to the local node.
- Add `wm-node doctor [--format json] [--fix]` — cross-references
  `placement.toml` against this node's enabled `claude-*`/`fleet-*`/`wm-*`
  systemd units, reporting misplaced-enabled and assigned-but-disabled drift
  (exit 1), duplicate placement keys (exit 2), and healthy (exit 0).
  `--fix` prints, never runs, the converging `systemctl --user` commands.
- Add optional `~/.config/wintermute/fleet.toml` for expected-down
  annotations (`assignments` renders `(may be off)` for a job assigned to a
  flagged node) and unknown-node detection in `doctor`.
- `wm-node should-run` is now permissive (exit 0, stderr warning) when
  `placement.toml` is absent, instead of failing closed, so a half-joined
  node doesn't silently kill its jobs; `doctor` reports the same absence
  loudly and exits 1.
