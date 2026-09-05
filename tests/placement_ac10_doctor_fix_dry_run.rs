// AC10: Given one misplaced and one missing unit, When `wm-node doctor --fix` runs, Then
// it prints the exact disable and enable commands and executes neither.

mod common;
use common::{run_with_units, stdout, FakeHome};

#[test]
fn doctor_fix_prints_commands_without_running_them() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    // vibeloop-measure assigned here but not enabled (case b); vibeloop-tick assigned to
    // carbon but enabled here (case a).
    home.write_placement(
        "vibeloop-measure = \"redbaron\"\nvibeloop-tick = \"carbon\"\n",
    );

    let out = run_with_units(&home, &["doctor", "--fix"], Some("vibeloop-tick"));
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);

    assert!(text.contains("--fix"));
    assert!(
        text.contains("disable") && text.contains("vibeloop-tick"),
        "expected a disable command for the misplaced unit: {text}"
    );
    assert!(
        text.contains("enable") && text.contains("vibeloop-measure"),
        "expected an enable command for the missing unit: {text}"
    );

    // Dry run only: the printed commands must never actually have been executed, i.e.
    // running the same doctor check again (units unchanged) reproduces the same drift.
    let out2 = run_with_units(&home, &["doctor"], Some("vibeloop-tick"));
    assert_eq!(out2.status.code(), Some(1));
}
