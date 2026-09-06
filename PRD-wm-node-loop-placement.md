# PRD: wm-node-loop-placement — recurring jobs run where the placement table says, not where someone last edited

- Status: in_progress
- Blocked: extend-gate.sh blocked at HEAD de1b28e (9 of 25 receipts) — intake, vti-plan, proof-receipt, risk-gate, reviewer-agent, session-trace missing because wm-node has never had autobuilder scaffolding (agent/intent-card.json, agent/proof-lanes.toml); ci-checks blocked (no GitHub Actions workflow / no runs for this SHA); semver-check and ac-traceability blocked (likely need the same scaffolding to run). Code, tests, docs, CHANGELOG, and version bump are done and pushed at de1b28e; the gap is the repo's pre-existing lack of autobuilder wiring, not this PRD's crate work.
- build_target: rust-extend
- build_into: /home/jsy/wintermute/wm-node
- build_version_bump: minor
- build_priority: normal
- test_prefix: [placement]
- publish: j0yen/private
- Vision: visions/fleet-autonomy.md
- PM: Joe
- deferred_acs: AC6 (vibeloop timer units in the dotfiles repo gain ExecCondition lines; RedBaron's local-only claude-vibeloop-tick.sh edit is reverted) — requires editing the dotfiles repo's unit sources, not the wm-node crate; not touched by this build to avoid fabricating cross-repo wiring. wm-node's own mechanism (should-run, assignments, doctor) is done and ready for that follow-on edit to consume. fleet-janitor/fleet-beacon placement.toml entries mentioned in Requirements P0 are reserved for their own PRDs (not yet in a buildable state) and are likewise not populated here.
- Drafted: 2026-09-05
- Engineering target: j0yen/wm-node (`should-run`, new `assignments` and `doctor` subcommands), plus placement.toml entries and ExecCondition lines for the fleet's recurring jobs

## TL;DR

wm-node v0.1.0 shipped the placement mechanism (`placement.toml` + `ExecCondition=wm-node should-run <job>`) and nothing uses it: the vibeloop tick runs on RedBaron because of a local-only script edit no repo records, janitor and beacon are about to need placement, and there is no way to ask "what runs where" fleet-wide. This PRD makes placement the enforced source of truth for every recurring fleet job and gives it a doctor.

## Problem statement

The fleet's recurring jobs (vibeloop tick + measure, self-review, digest timers, and the incoming janitor and beacon) are placed by hand: a unit enabled on one node, a script edited locally on another. The vibeloop tick's RedBaron placement exists only as a local edit to `claude-vibeloop-tick.sh` — invisible to fleet-sync, unrecoverable on re-image, and silently duplicable (a second node enabling the same timer would double-run the loop against one repo and one hub). Nobody can answer "what is supposed to run where" without ssh-ing into every node and listing timers.

## Goals

- `placement.toml` becomes the single fleet-synced source of truth for job→node assignment; every recurring fleet job's unit carries the `ExecCondition` gate.
- `wm-node assignments` answers "what runs where" from data; `wm-node doctor` answers "does reality match" on the node it runs on.
- Double-run of a singleton job (two nodes both believing they own it) becomes structurally impossible while placement is synced.

## Non-goals

- No scheduler: placement is static data a human edits; no automatic failover in this PRD (a powered-off ryzen7 job stays down and is reported, not migrated).
- No changes to what the jobs do — only to where they are allowed to run.
- No hub-side placement service; the table is a file carried by fleet-sync like FLEET.md.

## User stories

1. As Joe, I move the vibeloop tick to another node by editing one line in placement.toml and waiting for fleet-sync, instead of ssh-editing scripts on two machines.
2. As a Claude session, I run `wm-node assignments` and see every placed job with its node, so "where does measure run" stops being archaeology.
3. As /self-review on any node, I run `wm-node doctor --format json` and get every locally-enabled fleet timer that is not assigned here, and every assigned job with no enabled unit — drift in either direction.
4. As the janitor's unit (PRD-fleet-janitor), my `ExecCondition=wm-node should-run fleet-janitor` line means enabling me on a wrong node is a silent no-op, not a double patrol.
5. As Joe re-imaging RedBaron, the joined node picks up placement.toml via fleet-sync and `wm-node doctor` tells me exactly which units to enable — placement survives the machine.

## Requirements

P0 — placement.toml gains the fleet's real recurring jobs as data: `vibeloop-tick = "redbaron"`, `vibeloop-measure = "redbaron"`, `vibeloop-digest = "redbaron"`, `self-review = "carbon"` (plus one entry per node when others adopt it), `fleet-janitor` and `fleet-beacon` entries reserved for their PRDs. The file is carried by fleet-sync (added to its manifest) so every node reads the same table.
P0 — `wm-node assignments [--format json]`: print every placement entry with assigned node, marking entries assigned to this node.
P0 — `wm-node doctor [--format json]`: on this node, cross-reference placement.toml against `systemctl --user list-unit-files 'claude-*' 'fleet-*' 'wm-*'` state — report (a) enabled units whose job is assigned elsewhere, (b) jobs assigned here with no enabled unit, (c) jobs assigned here and enabled (healthy). Exit 1 when (a) or (b) is non-empty.
P0 — The vibeloop units on RedBaron (`claude-vibeloop-tick`, `-measure`, `-digest` timers/services) gain `ExecCondition=%h/.local/bin/wm-node should-run <job>` lines, committed to the repo that owns those units (dotfiles), replacing the local-only placement edit.
P1 — `wm-node should-run` behavior on a missing placement.toml stays permissive (exit 0 with a warning to stderr) so a half-joined node does not silently kill its jobs; `doctor` reports the missing file loudly. (v0.1.0 behavior verified at build time; changed only if it currently fails closed.)
P1 — A `fleet.toml`-aware note in assignments output when the assigned node is flagged expected-down (ryzen7), rendering `assigned: ryzen7 (may be off)` so a down singleton is visible as accepted risk, not drift.
P2 — `wm-node doctor --fix` prints (never runs) the exact `systemctl --user enable/disable` commands to converge this node.

Edge cases: placement.toml absent (should-run permissive + doctor loud, per P1); job assigned to a node not in fleet.toml (doctor flags `unknown-node`); duplicate job key in the TOML (parse error, doctor exits 2 naming the line); unit enabled but placement entry deleted (case (a) drift); systemctl unavailable in a test environment (doctor's unit probe is injectable for tests).

## Success metrics

| metric | baseline | target | method | timeframe |
|---|---|---|---|---|
| recurring fleet jobs gated by placement | 0 of ~5 | all vibeloop jobs + every new fleet job | grep ExecCondition in unit sources | at ship |
| "what runs where" answerable without ssh | no | `wm-node assignments`, one command | usage | at ship |
| local-only placement edits in force | ≥1 (vibeloop tick) | 0 | RedBaron script diff vs repo | at ship |

## Technical considerations

Extends the existing wm-node crate (clap subcommands beside `id`/`role`/`should-run`/`env`); the placement.toml format is unchanged, only populated. Doctor's systemd probe shells to `systemctl --user` with an injectable command for tests (fixture pattern per repo conventions). The dotfiles repo owns the vibeloop unit files — the ExecCondition change lands there and rides fleet-sync; the build must not hand-edit RedBaron's live units outside the repo (that is the exact failure mode this PRD removes).

## Migration / compatibility

The ExecCondition lines are additive: on nodes where placement.toml assigns the job locally, behavior is unchanged. RedBaron's local `claude-vibeloop-tick.sh` edit is superseded and reverted to the repo version as part of the unit change. wm-node v0.1.0 consumers (`role`, `env`) are untouched; version bumps minor.

## Open questions

| question | owner | due |
|---|---|---|
| Which node inherits a singleton when its assigned node is down for days — manual re-placement stays acceptable? | Joe | after a real outage |
| self-review placement entries for redbaron/ryzen7 (adopt the skill there?) | Joe | open-ended |

## Acceptance criteria

1. P0 — Given placement.toml assigns vibeloop-tick to redbaron, When `wm-node should-run vibeloop-tick` runs on redbaron, Then exit 0; and on carbon, Then exit 1.
2. P0 — Given the populated placement.toml, When `wm-node assignments --format json` runs, Then every entry appears with its node and a boolean marking local assignment.
3. P0 — Given a unit enabled locally for a job assigned elsewhere, When `wm-node doctor` runs, Then the job is listed under misplaced-enabled and doctor exits 1.
4. P0 — Given a job assigned here with no enabled unit, When doctor runs, Then it is listed under assigned-but-disabled and doctor exits 1.
5. P0 — Given all assigned jobs enabled and no strays, When doctor runs, Then it reports healthy and exits 0.
6. P0 — Given the vibeloop timer units in the dotfiles repo, When the build lands, Then each carries an ExecCondition wm-node should-run line and no local-only placement edit remains on RedBaron.
7. P1 — Given placement.toml is absent, When `wm-node should-run anything` runs, Then exit 0 with a stderr warning; and When doctor runs, Then it reports the missing file and exits 1.
8. P1 — Given a job assigned to ryzen7 flagged may-be-off, When assignments renders, Then the entry carries the may-be-off annotation.
9. P0 — Given a duplicate job key in placement.toml, When doctor runs, Then it exits 2 naming the duplicate key.
10. P2 — Given one misplaced and one missing unit, When `wm-node doctor --fix` runs, Then it prints the exact disable and enable commands and executes neither.
