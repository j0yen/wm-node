// AC4: Given a job assigned here with no enabled unit, When doctor runs, Then it is
// listed under assigned-but-disabled and doctor exits 1.

mod common;
use common::{run_with_units, stdout, FakeHome};

#[test]
fn doctor_flags_assigned_but_disabled_and_exits_1() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("vibeloop-tick = \"redbaron\"\n");

    // No enabled units at all -> vibeloop-tick is assigned here but nothing gates it.
    let out = run_with_units(&home, &["doctor"], Some(""));
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("assigned-but-disabled: vibeloop-tick"));
}
