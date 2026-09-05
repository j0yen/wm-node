// AC3: Given a unit enabled locally for a job assigned elsewhere, When `wm-node doctor`
// runs, Then the job is listed under misplaced-enabled and doctor exits 1.

mod common;
use common::{run_with_units, stdout, FakeHome};

#[test]
fn doctor_flags_misplaced_enabled_and_exits_1() {
    let home = FakeHome::new();
    home.write_node("carbon");
    // vibeloop-tick is assigned to redbaron, but carbon has it enabled locally.
    home.write_placement("vibeloop-tick = \"redbaron\"\n");

    let out = run_with_units(&home, &["doctor"], Some("vibeloop-tick"));
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("misplaced-enabled: vibeloop-tick"));
    assert!(stdout(&out).contains("redbaron"));
}

#[test]
fn doctor_flags_misplaced_enabled_for_deleted_placement_entry() {
    let home = FakeHome::new();
    home.write_node("carbon");
    // placement.toml exists but no longer mentions this job at all.
    home.write_placement("self-review = \"carbon\"\n");

    let out = run_with_units(&home, &["doctor"], Some("orphaned-job"));
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("misplaced-enabled: orphaned-job"));
}
