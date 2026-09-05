// AC5: Given all assigned jobs enabled and no strays, When doctor runs, Then it reports
// healthy and exits 0.

mod common;
use common::{run_with_units, stdout, FakeHome};

#[test]
fn doctor_reports_healthy_and_exits_0() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement(
        "vibeloop-tick = \"redbaron\"\nvibeloop-measure = \"redbaron\"\nself-review = \"carbon\"\n",
    );

    let out = run_with_units(&home, &["doctor"], Some("vibeloop-tick,vibeloop-measure"));
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("healthy"));
}

#[test]
fn doctor_healthy_json_has_empty_drift_lists() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("vibeloop-tick = \"redbaron\"\n");

    let out = run_with_units(&home, &["doctor", "--format", "json"], Some("vibeloop-tick"));
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["misplaced_enabled"].as_array().unwrap().len(), 0);
    assert_eq!(v["assigned_but_disabled"].as_array().unwrap().len(), 0);
    assert_eq!(v["healthy"].as_array().unwrap().len(), 1);
}
