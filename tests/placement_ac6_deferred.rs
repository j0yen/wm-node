//! AC6: Given the vibeloop timer units in the dotfiles repo, When the build
//! lands, Then each carries an ExecCondition wm-node should-run line and no
//! local-only placement edit remains on RedBaron.
//!
//! This AC is deferred (see PRD-wm-node-loop-placement.md's `deferred_acs:`
//! line) — it requires editing the dotfiles repo's unit sources, which is a
//! different repo entirely, not this crate. wm-node's own mechanism
//! (`should-run`, `assignments`, `doctor`) is built and tested by the other
//! ACs in this suite; AC6 is the follow-on cross-repo wiring that consumes
//! it. Rather than fabricate a test against code this crate doesn't own,
//! this test asserts the deferral is actually documented where a human (or
//! the next PRD) would look for it, so the deferral can't silently rot into
//! an untracked gap.

use std::fs;
use std::path::Path;

#[test]
fn ac6_deferral_is_documented_in_the_landed_prd() {
    let prd_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("PRD-wm-node-loop-placement.md");
    let text = fs::read_to_string(&prd_path)
        .unwrap_or_else(|e| panic!("expected {} to exist: {e}", prd_path.display()));

    assert!(
        text.contains("deferred_acs: AC6"),
        "PRD-wm-node-loop-placement.md must name AC6 in its deferred_acs \
         frontmatter line, or this deferral is undocumented"
    );
    assert!(
        text.contains("dotfiles repo"),
        "the deferred_acs line should explain AC6 requires the dotfiles \
         repo's unit sources, not this crate"
    );
}
