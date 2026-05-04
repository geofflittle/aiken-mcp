//! Parsing of Aiken CLI output.
//!
//! When stdout is non-TTY (which it is when invoked from this MCP server),
//! `aiken check` emits a structured JSON document per the schema returned by
//! `aiken check --show-json-schema`. We parse that whenever possible and
//! fall back to a line-based scanner for human-formatted output.
//!
//! Keep parsers small and well-tested. When Aiken's output format changes,
//! only this module needs updating.

use serde::Deserialize;

use aiken_mcp_core::diagnostic::{Diagnostic, Severity};
use aiken_mcp_core::runner::TestResult;

/// Best-effort extraction of diagnostics from `aiken check` output.
/// Scans stdout + stderr for lines that look like compiler diagnostics.
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

/// Parse `aiken check` JSON output (emitted when stdout is non-TTY).
/// Returns parsed test results when JSON parsing succeeds; falls back to
/// line-based parsing otherwise.
pub fn parse_test(stdout: &str) -> Vec<TestResult> {
    if let Some(v) = try_parse_test_json(stdout) {
        return v;
    }
    parse_test_lines(stdout)
}

fn try_parse_test_json(stdout: &str) -> Option<Vec<TestResult>> {
    let trimmed = stdout.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let parsed: AikenCheckJson = serde_json::from_str(trimmed).ok()?;
    let mut out = Vec::new();
    let modules = parsed.cmd_check.modules.unwrap_or_default();
    for module in modules {
        for test in module.test {
            out.push(TestResult {
                name: format!("{}::{}", module.name, test.title),
                passed: test.status.eq_ignore_ascii_case("pass")
                    || test.status.eq_ignore_ascii_case("passed"),
                mem: test.execution_units.as_ref().and_then(|u| u.mem),
                cpu: test.execution_units.as_ref().and_then(|u| u.cpu),
                message: test.assertion,
            });
        }
    }
    Some(out)
}

fn parse_test_lines(stdout: &str) -> Vec<TestResult> {
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

/// Best-effort artifact path extraction.
pub fn parse_artifacts(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.contains("plutus.json"))
        .map(|l| l.trim().to_string())
        .collect()
}

#[derive(Debug, Deserialize)]
struct AikenCheckJson {
    #[serde(rename = "command[check]")]
    cmd_check: CmdCheck,
}

#[derive(Debug, Deserialize)]
struct CmdCheck {
    #[serde(default)]
    modules: Option<Vec<ModuleEntry>>,
}

#[derive(Debug, Deserialize)]
struct ModuleEntry {
    name: String,
    #[serde(default)]
    test: Vec<TestEntry>,
}

#[derive(Debug, Deserialize)]
struct TestEntry {
    title: String,
    status: String,
    #[serde(default)]
    execution_units: Option<ExecUnits>,
    #[serde(default)]
    assertion: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecUnits {
    #[serde(default)]
    mem: Option<u64>,
    #[serde(default)]
    cpu: Option<u64>,
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
    fn parses_test_pass_fail_lines() {
        let stdout = "PASS test_one\nsome other line\nFAIL test_two\n";
        let tests = parse_test(stdout);
        assert_eq!(tests.len(), 2);
        assert!(tests[0].passed);
        assert!(!tests[1].passed);
    }

    #[test]
    fn parses_test_json_with_budget() {
        let stdout = r#"{"command[check]":{"modules":[
            {"name":"foo","test":[
                {"kind":"unit","title":"adds_two","status":"pass","on_failure":"fail",
                 "execution_units":{"mem":1234,"cpu":5678}}
            ]}
        ]}}"#;
        let tests = parse_test(stdout);
        assert_eq!(tests.len(), 1);
        assert!(tests[0].passed);
        assert_eq!(tests[0].mem, Some(1234));
        assert_eq!(tests[0].cpu, Some(5678));
        assert_eq!(tests[0].name, "foo::adds_two");
    }
}
