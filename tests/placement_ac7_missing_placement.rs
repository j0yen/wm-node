// AC7: Given placement.toml is absent, When `wm-node should-run anything` runs, Then
// exit 0 with a stderr warning; and When doctor runs, Then it reports the missing file
// and exits 1.

mod common;
use common::{run, stderr, stdout, FakeHome};

#[test]
fn should_run_is_permissive_when_placement_missing() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    // No placement.toml written at all.

    let out = run(&home, &["should-run", "anything"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!stderr(&out).is_empty(), "expected a stderr warning");
}

#[test]
fn doctor_reports_missing_placement_loudly_and_exits_1() {
    let home = FakeHome::new();
    home.write_node("redbaron");

    let out = run(&home, &["doctor"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).contains("missing"));
}

#[test]
fn doctor_json_reports_placement_missing_field() {
    let home = FakeHome::new();
    home.write_node("redbaron");

    let out = run(&home, &["doctor", "--format", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["placement_missing"], true);
}
