//! Extract pass/fail counts from the output of the supported test runners.
//!
//! Supports `node --test` (TAP and the default spec reporter) and Python `unittest`.
//! When counts cannot be determined they stay `None`; exit codes remain authoritative.

use pulsebench_types::TestSummary;

pub fn parse_test_summary(stdout: &str, stderr: &str) -> TestSummary {
    let combined = format!("{stdout}\n{stderr}");
    if let Some(s) = parse_node_test(&combined) {
        return s;
    }
    if let Some(s) = parse_unittest(&combined) {
        return s;
    }
    TestSummary::default()
}

/// Lines such as `# tests 5` (TAP) or `ℹ tests 5` (spec reporter).
fn parse_node_test(text: &str) -> Option<TestSummary> {
    let mut tests = None;
    let mut pass = None;
    let mut fail = None;
    let mut skipped = None;
    for line in text.lines() {
        let l = line.trim_start();
        let rest = l.strip_prefix('#').or_else(|| l.strip_prefix('ℹ'));
        let Some(rest) = rest else { continue };
        let mut it = rest.split_whitespace();
        let (Some(key), Some(val)) = (it.next(), it.next()) else { continue };
        let Ok(n) = val.parse::<u32>() else { continue };
        match key {
            "tests" => tests = Some(n),
            "pass" => pass = Some(n),
            "fail" => fail = Some(n),
            "skipped" => skipped = Some(n),
            _ => {}
        }
    }
    tests.map(|total| TestSummary { total: Some(total), passed: pass, failed: fail, skipped })
}

fn parse_unittest(text: &str) -> Option<TestSummary> {
    let ran = text.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("Ran ")?;
        let n: u32 = rest.split_whitespace().next()?.parse().ok()?;
        Some(n)
    })?;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with("FAILED (") || l.starts_with("OK (") || l == "OK" {
            let inner = l.split_once('(').map(|(_, r)| r.trim_end_matches(')')).unwrap_or("");
            for part in inner.split(',') {
                if let Some((k, v)) = part.trim().split_once('=') {
                    let n: u32 = v.trim().parse().unwrap_or(0);
                    match k.trim() {
                        "failures" | "errors" => failed += n,
                        "skipped" => skipped += n,
                        _ => {}
                    }
                }
            }
        }
    }
    Some(TestSummary { total: Some(ran), passed: Some(ran.saturating_sub(failed + skipped)), failed: Some(failed), skipped: Some(skipped) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_tap() {
        let out = "TAP version 13\nok 1 - a\nnot ok 2 - b\n1..2\n# tests 2\n# suites 0\n# pass 1\n# fail 1\n# cancelled 0\n# skipped 0\n# todo 0\n";
        let s = parse_test_summary(out, "");
        assert_eq!((s.total, s.passed, s.failed), (Some(2), Some(1), Some(1)));
    }

    #[test]
    fn node_spec_reporter() {
        let out = "✔ works (0.5ms)\nℹ tests 3\nℹ suites 0\nℹ pass 3\nℹ fail 0\nℹ cancelled 0\nℹ skipped 0\n";
        let s = parse_test_summary(out, "");
        assert_eq!((s.total, s.passed, s.failed), (Some(3), Some(3), Some(0)));
    }

    #[test]
    fn unittest_ok_and_failed() {
        let ok = "..\n----------------------------------------------------------------------\nRan 2 tests in 0.001s\n\nOK\n";
        let s = parse_test_summary("", ok);
        assert_eq!((s.total, s.passed, s.failed), (Some(2), Some(2), Some(0)));
        let bad = "Ran 5 tests in 0.002s\n\nFAILED (failures=2, errors=1)\n";
        let s = parse_test_summary("", bad);
        assert_eq!((s.total, s.passed, s.failed), (Some(5), Some(2), Some(3)));
        let skip = "Ran 4 tests in 0.002s\n\nOK (skipped=1)\n";
        let s = parse_test_summary("", skip);
        assert_eq!((s.passed, s.skipped), (Some(3), Some(1)));
    }

    #[test]
    fn unknown_output_has_no_counts() {
        let s = parse_test_summary("hello", "world");
        assert_eq!(s, TestSummary::default());
    }

    #[test]
    fn unittest_import_error_has_no_tests_run() {
        let err = "ImportError: cannot import name 'x'\nRan 1 test in 0.000s\n\nFAILED (errors=1)\n";
        let s = parse_test_summary("", err);
        assert_eq!((s.total, s.passed, s.failed), (Some(1), Some(0), Some(1)));
    }
}
