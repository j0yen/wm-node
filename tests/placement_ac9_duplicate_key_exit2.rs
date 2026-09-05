// AC9: Given a duplicate job key in placement.toml, When doctor runs, Then it exits 2
// naming the duplicate key.

mod common;
use common::{run, stderr, FakeHome};

#[test]
fn doctor_exits_2_and_names_duplicate_key() {
    let home = FakeHome::new();
    home.write_node("redbaron");
    home.write_placement("vibeloop-tick = \"redbaron\"\nvibeloop-tick = \"carbon\"\n");

    let out = run(&home, &["doctor"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("vibeloop-tick"), "expected the duplicate key named in stderr: {err}");
}
