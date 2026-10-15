use std::collections::BTreeMap;
use crate::bencode::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestResultType {
    Pass,
    Fail,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestAssertionResult {
    pub result_type: TestResultType,
    pub index: u32,
    pub context: Option<String>,
    pub message: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub error: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub ns: String,
    pub var: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestSummary {
    pub test: u32,
    pub var: u32,
    pub pass: u32,
    pub fail: u32,
    pub error: u32,
    pub ns: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestReport {
    pub summary: TestSummary,
    pub results: BTreeMap<String, BTreeMap<String, Vec<TestAssertionResult>>>,
}

pub fn parse_deftest_name(form_text: &str) -> Option<String> {
    let trimmed = form_text.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    let inner = &trimmed[1..];
    let mut tokens = inner.split_whitespace();
    let head = tokens.next()?;
    if head != "deftest" && !head.ends_with("/deftest") {
        return None;
    }

    for token in tokens {
        if token.starts_with('^') || token.starts_with('"') {
            continue;
        }
        let clean = token.trim_matches(|c| c == ')' || c == '(' || c == ',');
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }
    None
}

pub fn parse_test_response(messages: &[Value]) -> Result<TestReport, String> {
    for msg in messages {
        if let Some(statuses) = msg.get("status").and_then(Value::as_list) {
            for status in statuses {
                if status.as_str() == Some("unknown-op") {
                    return Err("Running tests requires cider-nrepl middleware".to_string());
                }
            }
        }
    }

    let mut report = TestReport::default();
    let mut found_any = false;

    for msg in messages {
        if let Some(summary_val) = msg.get("summary") {
            report.summary = parse_summary(summary_val);
            found_any = true;
        }
        if let Some(results_val) = msg.get("results") {
            parse_results(results_val, &mut report.results);
            found_any = true;
        }
    }

    if !found_any {
        return Err("Empty response from test runner".to_string());
    }

    Ok(report)
}

fn parse_summary(val: &Value) -> TestSummary {
    let get_u32 = |key: &str| -> u32 {
        val.get(key)
            .and_then(|v| {
                v.as_int()
                    .map(|n| n as u32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0)
    };

    TestSummary {
        test: get_u32("test"),
        var: get_u32("var"),
        pass: get_u32("pass"),
        fail: get_u32("fail"),
        error: get_u32("error"),
        ns: get_u32("ns"),
    }
}

fn parse_results(
    val: &Value,
    out: &mut BTreeMap<String, BTreeMap<String, Vec<TestAssertionResult>>>,
) {
    let Some(ns_dict) = val.as_dict() else {
        return;
    };
    for (ns_bytes, var_dict_val) in ns_dict {
        let Some(var_dict) = var_dict_val.as_dict() else {
            continue;
        };
        let ns = String::from_utf8_lossy(ns_bytes).into_owned();
        let ns_entry = out.entry(ns.clone()).or_default();
        for (var_bytes, items_val) in var_dict {
            let Some(items) = items_val.as_list() else {
                continue;
            };
            let var_name = String::from_utf8_lossy(var_bytes).into_owned();
            let var_entry = ns_entry.entry(var_name.clone()).or_default();
            for item in items {
                if let Some(assertion) = parse_assertion_item(item, &ns, &var_name) {
                    var_entry.push(assertion);
                }
            }
        }
    }
}

fn parse_assertion_item(val: &Value, ns: &str, var_name: &str) -> Option<TestAssertionResult> {
    let type_str = val.get("type").and_then(Value::as_str)?;
    let result_type = match type_str {
        "pass" => TestResultType::Pass,
        "fail" => TestResultType::Fail,
        _ => TestResultType::Error,
    };

    let index = val.get("index").and_then(Value::as_int).unwrap_or(0) as u32;

    let get_str = |key: &str| -> Option<String> {
        val.get(key)
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let context = get_str("context");
    let message = get_str("message");
    let expected = get_str("expected");
    let actual = get_str("actual");
    let error = get_str("error");
    let file = get_str("file");
    let line = val.get("line").and_then(|v| {
        v.as_int()
            .map(|n| n as u32)
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    });

    Some(TestAssertionResult {
        result_type,
        index,
        context,
        message,
        expected,
        actual,
        error,
        file,
        line,
        ns: ns.to_string(),
        var: var_name.to_string(),
    })
}

pub fn format_console_summary(summary: &TestSummary) -> String {
    let tests_word = if summary.var == 1 { "test" } else { "tests" };
    let assertions_word = if summary.test == 1 {
        "assertion"
    } else {
        "assertions"
    };
    let failures_word = if summary.fail == 1 {
        "failure"
    } else {
        "failures"
    };
    let errors_word = if summary.error == 1 {
        "error"
    } else {
        "errors"
    };

    format!(
        ";; Ran {} {}, {} {}: {} {}, {} {}\n",
        summary.var,
        tests_word,
        summary.test,
        assertions_word,
        summary.fail,
        failures_word,
        summary.error,
        errors_word
    )
}

pub fn format_console_failure(assertion: &TestAssertionResult) -> String {
    let kind = if assertion.result_type == TestResultType::Error {
        "ERROR"
    } else {
        "FAIL"
    };

    let location = match (&assertion.file, assertion.line) {
        (Some(file), Some(line)) => format!(" {file}:{line}"),
        (Some(file), None) => format!(" {file}"),
        (None, Some(line)) => format!(" line:{line}"),
        (None, None) => String::new(),
    };

    let mut out = format!("{kind} in ({}){}", assertion.var, location);

    if let Some(expected) = &assertion.expected {
        out.push_str(&format!("\n  expected: {expected}"));
    }
    if let Some(actual) = &assertion.actual {
        out.push_str(&format!("\n    actual: {actual}"));
    }
    if let Some(error) = &assertion.error {
        out.push_str(&format!("\n     error: {error}"));
    }
    if let Some(msg) = &assertion.message {
        if !msg.is_empty() {
            out.push_str(&format!("\n   message: {msg}"));
        }
    }
    out.push('\n');
    out
}

pub fn format_toast_summary(summary: &TestSummary) -> String {
    format!(
        "Ran {} tests, {} assertions: {} failures, {} errors",
        summary.var, summary.test, summary.fail, summary.error
    )
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::bencode::{self, DecodeOutcome, Value, dict, encode};
    use crate::forms::{
        closing_brackets, ns_form_text, parse_namespace, text_current_form, text_top_level_form,
    };
    use crate::jack_in::{
        JackInCommand, ProjectKind, detect_project_kind_with, parse_port_from_output,
    };
    use crate::nrepl::{NreplClientState, RequestKind};
    use crate::paredit::{
        barf_forwards, raise, slurp_forwards, splice, wrap,
    };
    use std::path::Path;

    #[test]
    fn test_bencode_encoding_and_decoding() {
        let v_int = Value::Int(42);
        assert_eq!(encode(&v_int), b"i42e");
        let v_neg = Value::Int(-10);
        assert_eq!(encode(&v_neg), b"i-10e");
        let v_zero = Value::Int(0);
        assert_eq!(encode(&v_zero), b"i0e");

        let v_str = Value::str("hello");
        assert_eq!(encode(&v_str), b"5:hello");

        let v_list = Value::List(vec![Value::Int(1), Value::str("two")]);
        assert_eq!(encode(&v_list), b"li1e3:twoe");

        let v_dict = dict([("b", Value::Int(2)), ("a", Value::Int(1))]);
        // Keys sorted lexicographically
        assert_eq!(encode(&v_dict), b"d1:ai1e1:bi2ee");

        // Roundtrip decode
        let decoded = bencode::decode_one(&encode(&v_dict)).unwrap();
        assert_eq!(
            decoded,
            DecodeOutcome::Value {
                value: v_dict,
                consumed: 14,
            }
        );
    }

    #[test]
    fn test_bencode_partial_buffer() {
        let full = b"d2:op4:eval4:code7:(+ 1 2)e";
        let partial = &full[..10];
        let res = bencode::decode_one(partial).unwrap();
        assert_eq!(res, DecodeOutcome::Incomplete);

        let mut buf = partial.to_vec();
        let avail = bencode::decode_available(&mut buf).unwrap();
        assert_eq!(avail.len(), 0);
        assert_eq!(buf.len(), 10);

        buf.extend_from_slice(&full[10..]);
        let avail2 = bencode::decode_available(&mut buf).unwrap();
        assert_eq!(avail2.len(), 1);
        assert_eq!(buf.len(), 0);
        assert_eq!(avail2[0].get("op").unwrap().as_str().unwrap(), "eval");
    }

    #[test]
    fn test_nrepl_response_assembly() {
        let mut state = NreplClientState::new("127.0.0.1".to_string(), 7888);
        let req_id = "perry-10".to_string();
        state.register_request(
            req_id.clone(),
            RequestKind::LoadFile {
                path: "test.clj".to_string(),
            },
        );

        // Frame 1: stdout chunk
        let f1 = encode(&dict([
            ("id", Value::str(&req_id)),
            ("out", Value::str("computing...\n")),
        ]));
        // Frame 2: value
        let f2 = encode(&dict([
            ("id", Value::str(&req_id)),
            ("value", Value::str("3")),
        ]));
        // Frame 3: done status
        let f3 = encode(&dict([
            ("id", Value::str(&req_id)),
            ("status", Value::List(vec![Value::str("done")])),
            ("ns", Value::str("user")),
        ]));

        // Feed first 2 frames together
        let mut combined = Vec::new();
        combined.extend_from_slice(&f1);
        combined.extend_from_slice(&f2);

        let (completed, _) = state.feed_bytes(&combined).unwrap();
        assert_eq!(completed.len(), 0, "Not done yet");

        // Feed frame 3
        let (completed2, _) = state.feed_bytes(&f3).unwrap();
        assert_eq!(completed2.len(), 1, "Now done");
        let done_resp = &completed2[0];
        assert_eq!(done_resp.id, req_id);
        assert_eq!(done_resp.out(), "computing...\n");
        assert_eq!(done_resp.value().as_deref(), Some("3"));
        assert_eq!(done_resp.ns().as_deref(), Some("user"));
    }

    #[test]
    fn test_forms_detection_and_ns() {
        let text = ";; Header comment\n(ns my-proj.core\n  (:require [clojure.string :as str]))\n\n(defn add [a b]\n  (+ a b))\n";
        assert_eq!(parse_namespace(text).as_deref(), Some("my-proj.core"));

        let ns_form = ns_form_text(text).unwrap();
        assert!(ns_form.starts_with("(ns my-proj.core"));

        let cursor_in_defn = text.find("(+ a b)").unwrap() + "(+ a b)".len();
        let top_level = text_top_level_form(text, cursor_in_defn).unwrap();
        assert_eq!(&text[top_level], "(defn add [a b]\n  (+ a b))");

        let cur_form = text_current_form(text, cursor_in_defn).unwrap();
        assert_eq!(&text[cur_form], "(+ a b)");

        assert_eq!(closing_brackets("(defn f [x] (let [a 1"), "]))");
    }

    #[test]
    fn test_paredit_slurp_and_barf() {
        let text = "(foo [1 2] 3)";
        // Cursor inside [1 2]
        let offset = text.find("1").unwrap();
        let slurp_res = slurp_forwards(text, offset).unwrap();
        assert_eq!(slurp_res.replacement_text, "[1 2 3]");

        let barf_res = barf_forwards("(foo [1 2 3])", offset).unwrap();
        assert_eq!(barf_res.replacement_text, "[1 2] 3");

        let defn_text = "(defn f [] 42)";
        let raise_res = raise(defn_text, defn_text.find("42").unwrap()).unwrap();
        assert_eq!(raise_res.replacement_text, "42");

        let splice_res = splice("(foo 1 2 3)", 5).unwrap();
        assert_eq!(splice_res.replacement_text, "foo 1 2 3");

        let wrap_res = wrap("hello", 0..5, "(", ")").unwrap();
        assert_eq!(wrap_res.replacement_text, "(hello)");
    }

    #[test]
    fn test_jack_in_detection_and_port_parsing() {
        assert_eq!(
            detect_project_kind_with(|file| file == "deps.edn"),
            Some(ProjectKind::ClojureCli)
        );
        assert_eq!(
            detect_project_kind_with(|file| file == "project.clj"),
            Some(ProjectKind::Leiningen)
        );
        assert_eq!(
            detect_project_kind_with(|file| file == "shadow-cljs.edn"),
            Some(ProjectKind::ShadowCljs)
        );
        assert_eq!(
            detect_project_kind_with(|file| file == "bb.edn"),
            Some(ProjectKind::Babashka)
        );
        assert_eq!(
            detect_project_kind_with(|file| file == "deps.edn" || file == "project.clj"),
            Some(ProjectKind::ClojureCli)
        );
        assert_eq!(detect_project_kind_with(|_| false), None);

        let stdout = "nREPL server started on port 51234 on host 127.0.0.1 - nrepl://127.0.0.1:51234\n";
        assert_eq!(parse_port_from_output(stdout), Some(51234));

        let stdout2 = "Started nREPL server at 127.0.0.1:7888\n";
        assert_eq!(parse_port_from_output(stdout2), Some(7888));

        let cmd = JackInCommand::new(ProjectKind::ClojureCli, Path::new("/tmp"), 7888);
        assert_eq!(cmd.program, "clojure");
        assert!(cmd.args.contains(&"nrepl.cmdline".to_string()));
        assert!(cmd.args.contains(&"7888".to_string()));
        assert!(cmd.args.iter().any(|arg| arg.contains("cider-nrepl")));
    }

    #[test]
    fn test_cider_test_response_parsing() {
        let messages = vec![dict([
            (
                "summary",
                dict([
                    ("test", Value::Int(2)),
                    ("var", Value::Int(1)),
                    ("pass", Value::Int(1)),
                    ("fail", Value::Int(1)),
                    ("error", Value::Int(0)),
                    ("ns", Value::Int(1)),
                ]),
            ),
            (
                "results",
                dict([(
                    "my-ns",
                    dict([(
                        "my-test",
                        Value::List(vec![
                            dict([
                                ("type", Value::str("pass")),
                                ("index", Value::Int(0)),
                            ]),
                            dict([
                                ("type", Value::str("fail")),
                                ("index", Value::Int(1)),
                                ("expected", Value::str("4")),
                                ("actual", Value::str("5")),
                                ("file", Value::str("test/my_test.clj")),
                                ("line", Value::Int(12)),
                            ]),
                        ]),
                    )]),
                )]),
            ),
            ("status", Value::List(vec![Value::str("done")])),
        ])];

        let report = parse_test_response(&messages).unwrap();
        assert_eq!(report.summary.test, 2);
        assert_eq!(report.summary.fail, 1);
        assert_eq!(report.summary.pass, 1);
        assert!(report.results.contains_key("my-ns"));

        let fail_output =
            format_console_failure(&report.results["my-ns"]["my-test"][1]);
        assert!(fail_output.contains("FAIL in (my-test)"));
        assert!(fail_output.contains("expected: 4"));
        assert!(fail_output.contains("actual: 5"));
    }
}
