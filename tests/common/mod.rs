//! Helpers shared by the integration tests.
//!
//! Each test binary includes this module but exercises a different subset of
//! the helpers, so the unused ones are expected rather than dead.

#![allow(dead_code)]

use std::process::Command;

/// Whether `program` can be spawned at all.
pub fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Skip when `program` is missing; fail when `require` says it must be present.
///
/// A bare early return is indistinguishable from a pass, so CI sets
/// `MOMO_REQUIRE_TOOLS`: there a missing `jj` or `mise` is a hard failure and
/// the strongest regression tests cannot go green by not running. A local run
/// without the tool still skips, to keep `cargo test` frictionless.
pub fn require_tool_when(program: &str, require: bool) -> bool {
    if have(program) {
        return true;
    }
    assert!(
        !require,
        "{program} is required by MOMO_REQUIRE_TOOLS but is not on PATH"
    );
    eprintln!("skipping: {program} is not on PATH");
    false
}

/// Whether `program` is available, honouring `MOMO_REQUIRE_TOOLS`.
pub fn require_tool(program: &str) -> bool {
    require_tool_when(program, std::env::var_os("MOMO_REQUIRE_TOOLS").is_some())
}

/// The dates a release heading may carry: today or yesterday in UTC.
///
/// The binary stamps the date when it runs and the test reads it back a moment
/// later, so allowing the previous day keeps a run that straddles midnight UTC
/// from flaking — the straddle itself is not the contract under test.
pub fn today_or_yesterday() -> [String; 2] {
    let now = time::OffsetDateTime::now_utc().date();
    let yesterday = now.previous_day().expect("a date before today");
    [now.to_string(), yesterday.to_string()]
}

/// The date in the `## [version] - <date>` heading of `changelog`.
pub fn release_date(changelog: &str, version: &str) -> String {
    let prefix = format!("## [{version}] - ");
    changelog
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no `{prefix}` heading in:\n{changelog}"))
        .trim()
        .to_owned()
}
