//! Tests for the integration-test harness itself.
//!
//! A skipped test is indistinguishable from a passing one, so CI sets
//! `MOMO_REQUIRE_TOOLS` and a missing tool must fail loudly there.

mod common;

use common::require_tool_when;

/// A program no machine has on `PATH`.
const MISSING: &str = "momo-tool-that-does-not-exist";

#[test]
#[should_panic(expected = "MOMO_REQUIRE_TOOLS")]
fn a_missing_tool_fails_when_tools_are_required() {
    require_tool_when(MISSING, true);
}

#[test]
fn a_missing_tool_skips_when_tools_are_optional() {
    assert!(!require_tool_when(MISSING, false));
}
