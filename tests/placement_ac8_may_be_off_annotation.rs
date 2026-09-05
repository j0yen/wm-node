// AC8: Given a job assigned to ryzen7 flagged may-be-off, When assignments renders, Then
// the entry carries the may-be-off annotation.

mod common;
use common::{run, stdout, FakeHome};

#[test]
fn assignments_text_annotates_expected_down_node() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("some-batch-job = \"ryzen7\"\n");
    home.write_fleet("[nodes.ryzen7]\nexpected_down = true\n");

    let out = run(&home, &["assignments"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("some-batch-job"));
    assert!(text.contains("may be off"), "expected may-be-off annotation, got: {text}");
}

#[test]
fn assignments_json_carries_may_be_off_boolean() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("some-batch-job = \"ryzen7\"\nvibeloop-tick = \"redbaron\"\n");
    home.write_fleet("[nodes.ryzen7]\nexpected_down = true\n[nodes.redbaron]\nexpected_down = false\n");

    let out = run(&home, &["assignments", "--format", "json"]);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout(&out)).unwrap();
    let ryzen_entry = parsed.iter().find(|e| e["job"] == "some-batch-job").unwrap();
    assert_eq!(ryzen_entry["may_be_off"], true);
    let redbaron_entry = parsed.iter().find(|e| e["job"] == "vibeloop-tick").unwrap();
    assert_eq!(redbaron_entry["may_be_off"], false);
}
