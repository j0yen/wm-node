// AC2: Given the populated placement.toml, When `wm-node assignments --format json` runs,
// Then every entry appears with its node and a boolean marking local assignment.

mod common;
use common::{run, stdout, FakeHome};
use serde_json::Value;

#[test]
fn assignments_json_lists_every_entry_with_node_and_local_flag() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement(
        "vibeloop-tick = \"redbaron\"\nvibeloop-measure = \"redbaron\"\nself-review = \"carbon\"\n",
    );

    let out = run(&home, &["assignments", "--format", "json"]);
    assert_eq!(out.status.code(), Some(0));

    let parsed: Vec<Value> = serde_json::from_str(&stdout(&out)).expect("valid json array");
    assert_eq!(parsed.len(), 3);

    for entry in &parsed {
        assert!(entry.get("job").is_some());
        assert!(entry.get("node").is_some());
        assert!(entry.get("local").and_then(Value::as_bool).is_some());
    }

    let by_job = |job: &str| parsed.iter().find(|e| e["job"] == job).unwrap();
    assert_eq!(by_job("vibeloop-tick")["node"], "redbaron");
    assert_eq!(by_job("vibeloop-tick")["local"], true);
    assert_eq!(by_job("self-review")["node"], "carbon");
    assert_eq!(by_job("self-review")["local"], false);
}
