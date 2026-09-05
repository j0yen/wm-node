// AC1: Given placement.toml assigns vibeloop-tick to redbaron, When `wm-node should-run
// vibeloop-tick` runs on redbaron, Then exit 0; and on carbon, Then exit 1.

mod common;
use common::{run, FakeHome};

#[test]
fn should_run_exits_0_on_assigned_node() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("vibeloop-tick = \"redbaron\"\n");

    let out = run(&home, &["should-run", "vibeloop-tick"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn should_run_exits_1_on_other_node() {
    let home = FakeHome::new();
    home.write_node("carbon");
    home.write_placement("vibeloop-tick = \"redbaron\"\n");

    let out = run(&home, &["should-run", "vibeloop-tick"]);
    assert_eq!(out.status.code(), Some(1));
}
