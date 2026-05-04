//! Parsing of Aiken CLI output.
//!
//! Aiken's CLI is human-oriented. We surface raw stdout/stderr unconditionally
//! so the model has a fallback if best-effort parsers miss something. The
//! parsers below extract structured signals where the format is stable enough.
//!
//! Keep these parsers small and well-tested. When Aiken's output format
//! changes, only this module needs updating.

use aiken_mcp_core::diagnostic::{Diagnostic, Severity};
use aiken_mcp_core::runner::TestResult;

/// Best-effort extraction of diagnostics from `aiken check` output.
///
/// Aiken emits diagnostics across stdout + stderr depending on category.
/// We scan both for lines that look like compiler diagnostics.
pub fn parse_check(stdout: &str, stderr: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for raw in stdout.lines().chain(stderr.lines()) {
        if let Some(d) = try_parse_diag_line(raw) {
            out.push(d);
        }
    }
    out
}

fn try_parse_diag_line(line: &str) -> Option<Diagnostic> {
    let trimmed = line.trim_start();

    let (severity, rest) = if let Some(rest) = trimmed.strip_prefix("error:") {
        (Severity::Error, rest)
    } else if let Some(rest) = trimmed.strip_prefix("warning:") {
        (Severity::Warning, rest)
    } else {
        return None;
    };

    Some(Diagnostic {
        severity,
        message: rest.trim().to_string(),
        span: None,
        code: None,
    })
}

/// Best-effort extraction of test results from `aiken check` output.
pub fn parse_test(stdout: &str) -> Vec<TestResult> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("PASS ") {
            out.push(TestResult {
                name: name.split_whitespace().next().unwrap_or(name).to_string(),
                passed: true,
                mem: None,
                cpu: None,
                message: None,
            });
        } else if let Some(name) = trimmed.strip_prefix("FAIL ") {
            out.push(TestResult {
                name: name.split_whitespace().next().unwrap_or(name).to_string(),
                passed: false,
                mem: None,
                cpu: None,
                message: None,
            });
        }
    }
    out
}

/// Best-effort artifact path extraction from `aiken build` output.
pub fn parse_artifacts(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.split("plutus.json").next().map(|_| ()).and(Some(l)))
        .filter(|l| l.contains("plutus.json"))
        .map(|l| l.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_error_line() {
        let out = parse_check("error: undefined variable foo\n", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);
        assert!(out[0].message.contains("undefined variable foo"));
    }

    #[test]
    fn parses_warning_line() {
        let out = parse_check("", "warning: unused import\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn ignores_unrelated_lines() {
        let out = parse_check("Compiling foo\n", "");
        assert!(out.is_empty());
    }

    #[test]
    fn parses_test_pass_fail() {
        let stdout = "PASS test_one\nsome other line\nFAIL test_two\n";
        let tests = parse_test(stdout);
        assert_eq!(tests.len(), 2);
        assert!(tests[0].passed);
        assert!(!tests[1].passed);
        assert_eq!(tests[0].name, "test_one");
        assert_eq!(tests[1].name, "test_two");
    }
}
