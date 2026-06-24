//! Integration tests for `uncomment inventory` (docs/test-and-acceptance-plan.md).

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = "fixtures/inventory";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_uncomment")
}

fn run_inventory(paths: &str, out: &Path, extra: &[&str]) {
    let mut cmd = Command::new(bin());
    cmd.arg("inventory")
        .arg(paths)
        .arg("--output-dir")
        .arg(out)
        .arg("--languages")
        .arg("python,c,cpp,yaml");
    cmd.args(extra);
    let status = cmd.status().expect("run inventory");
    assert!(status.success(), "inventory exited with failure");
}

fn read_jsonl(out: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(out.join("comments_docstrings.jsonl")).expect("read jsonl");
    text.lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("valid json line"))
        .collect()
}

fn collect_files(dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(&path, into);
        } else {
            into.insert(path.clone(), std::fs::read(&path).unwrap());
        }
    }
}

#[test]
fn does_not_mutate_source_files() {
    // AC-001.
    let mut before = BTreeMap::new();
    collect_files(Path::new(FIXTURES), &mut before);

    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);

    let mut after = BTreeMap::new();
    collect_files(Path::new(FIXTURES), &mut after);
    assert_eq!(before, after, "inventory must not modify source files");
}

#[test]
fn jsonl_is_valid_and_nonempty() {
    // AC-007.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);
    let records = read_jsonl(out.path());
    assert!(!records.is_empty());
    for r in &records {
        assert!(r.get("id").is_some());
        assert!(r.get("schema_version").is_some());
        assert!(r.get("priority").is_some());
    }
}

#[test]
fn output_is_deterministic() {
    // FR-012.
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, a.path(), &["--format", "jsonl"]);
    run_inventory(FIXTURES, b.path(), &["--format", "jsonl"]);
    let ja = std::fs::read_to_string(a.path().join("comments_docstrings.jsonl")).unwrap();
    let jb = std::fs::read_to_string(b.path().join("comments_docstrings.jsonl")).unwrap();
    assert_eq!(ja, jb, "JSONL output must be byte-for-byte stable");
}

#[test]
fn c_macro_doxygen_is_high_priority() {
    // AC-003.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);
    let records = read_jsonl(out.path());
    let macro_rec = records
        .iter()
        .find(|r| {
            r.get("c_family")
                .and_then(|c| c.get("target_declaration_name"))
                .and_then(|n| n.as_str())
                == Some("FRAME_VALIDATE_ROW_STRIDE_BITS")
        })
        .expect("macro record present");
    assert_eq!(macro_rec["priority"], "high");
    let claims: Vec<&str> = macro_rec["claim_types"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(claims.contains(&"model_parameter") || claims.contains(&"constant_contract"));
}

#[test]
fn user_code_region_metadata_and_marker_suppression() {
    // AC-004.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);
    let records = read_jsonl(out.path());

    let todo = records
        .iter()
        .find(|r| {
            r.get("c_family")
                .and_then(|c| c.get("inside_user_code_region"))
                .and_then(Value::as_bool)
                == Some(true)
                && r["markers"]
                    .as_array()
                    .is_some_and(|m| m.iter().any(|v| v.as_str() == Some("TODO")))
        })
        .expect("user-code TODO present");
    assert_eq!(todo["c_family"]["user_code_region"], "Includes");
    assert_eq!(todo["priority"], "high");

    let marker_emitted = records.iter().any(|r| {
        r["raw_text"]
            .as_str()
            .unwrap_or("")
            .contains("USER CODE BEGIN")
    });
    assert!(
        !marker_emitted,
        "USER CODE markers must not be emitted as claims"
    );
}

#[test]
fn yaml_key_path_and_allowed_values() {
    // AC-005, AC-006.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);
    let records = read_jsonl(out.path());

    let has_key_path = records.iter().any(|r| {
        r.get("yaml")
            .and_then(|y| y.get("key_path"))
            .and_then(|k| k.as_str())
            == Some("streams.reference.nominal_srate_hz")
    });
    assert!(has_key_path, "nested YAML key path must be emitted");

    let mode = records
        .iter()
        .find(|r| {
            r.get("yaml")
                .and_then(|y| y.get("key_path"))
                .and_then(|k| k.as_str())
                == Some("mode")
                && r["yaml"]["allowed_values_hint"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
        })
        .expect("mode allowed-values record");
    let allowed: Vec<&str> = mode["yaml"]["allowed_values_hint"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(allowed, vec!["modbus_rtu", "active_send"]);
    assert_eq!(mode["yaml"]["value"], "active_send");
}

#[test]
fn preprocessor_diagnostic_is_high_priority() {
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "jsonl"]);
    let records = read_jsonl(out.path());
    let diags: Vec<&Value> = records
        .iter()
        .filter(|r| r["kind"] == "preprocessor_diagnostic")
        .collect();
    assert!(!diags.is_empty(), "expected preprocessor diagnostics");
    for d in diags {
        assert_eq!(d["priority"], "high");
        let claims: Vec<&str> = d["claim_types"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(claims.contains(&"build_assumption"));
    }
}

#[test]
fn markdown_has_retrieval_context_for_macro() {
    // AC-008.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "markdown"]);
    let md = std::fs::read_to_string(out.path().join("comments_docstrings.md")).unwrap();
    assert!(md.contains("FRAME_VALIDATE_ROW_STRIDE_BITS"));
    assert!(md.contains("Priority: high"));
    assert!(md.contains("Extracted text:"));
}

#[test]
fn summary_contains_totals() {
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "summary"]);
    let s = std::fs::read_to_string(out.path().join("comments_docstrings_summary.md")).unwrap();
    assert!(s.contains("Total records:"));
    assert!(s.contains("Records by language"));
    assert!(s.contains("Records by priority"));
    assert!(s.contains("Records by claim type"));
}

#[test]
fn manifest_is_written_with_all_formats() {
    let out = tempfile::tempdir().unwrap();
    run_inventory(
        FIXTURES,
        out.path(),
        &["--format", "jsonl,markdown,summary,by-symbol"],
    );
    for f in [
        "comments_docstrings.jsonl",
        "comments_docstrings.md",
        "comments_docstrings_summary.md",
        "comments_docstrings_by_symbol.json",
        "inventory_manifest.json",
    ] {
        assert!(out.path().join(f).exists(), "missing artifact: {f}");
    }
}

#[test]
fn python_docstrings_distinguished_from_strings() {
    // AC-002.
    let out = tempfile::tempdir().unwrap();
    run_inventory(
        "fixtures/inventory/python/docstrings.py",
        out.path(),
        &["--format", "jsonl"],
    );
    let records = read_jsonl(out.path());
    let docstrings: Vec<&Value> = records
        .iter()
        .filter(|r| r["kind"] == "docstring")
        .collect();
    // module + class + method = 3 docstrings; the assigned triple-quoted string
    // must NOT be one of them.
    assert!(
        docstrings.len() >= 3,
        "expected >=3 docstrings, got {}",
        docstrings.len()
    );
    assert!(
        !records.iter().any(|r| r["normalized_text"]
            .as_str()
            .unwrap_or("")
            .contains("not a docstring")),
        "assigned string must not be treated as a docstring"
    );
}

#[test]
fn module_docstring_scored_above_ignore_by_default() {
    // GAP-1: module-scope docstrings carry a +3 base bonus, so they survive the
    // default min_priority=low filter instead of falling to ignore.
    let out = tempfile::tempdir().unwrap();
    run_inventory(
        "fixtures/inventory/python/docstrings.py",
        out.path(),
        &["--format", "jsonl"],
    );
    let records = read_jsonl(out.path());
    let module_doc = records
        .iter()
        .find(|r| {
            r["kind"] == "docstring"
                && r.get("python")
                    .and_then(|p| p.get("docstring_scope"))
                    .and_then(|s| s.as_str())
                    == Some("module")
        })
        .expect("module docstring must be present at default settings");
    let reasons: Vec<&str> = module_doc["score_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        reasons.contains(&"docstring attached to module"),
        "reasons: {reasons:?}"
    );
}

#[test]
fn marker_not_flagged_inside_code_span() {
    // GAP-2: ``WARNING`` enumerated in a backtick span is a value, not a marker.
    let out = tempfile::tempdir().unwrap();
    run_inventory(
        "fixtures/inventory/python/docstrings.py",
        out.path(),
        &["--format", "jsonl", "--min-priority", "ignore"],
    );
    let records = read_jsonl(out.path());
    let configure_doc = records
        .iter()
        .find(|r| {
            r["normalized_text"]
                .as_str()
                .unwrap_or("")
                .contains("Console verbosity")
        })
        .expect("configure_logging docstring present");
    let markers = configure_doc["markers"].as_array().unwrap();
    assert!(
        markers.is_empty(),
        "no marker may fire inside a code span, got {markers:?}"
    );
}

#[test]
fn numpy_docstring_sections_populated() {
    // GAP-3: NumPy underline sections populate python.docstring_sections.
    let out = tempfile::tempdir().unwrap();
    run_inventory(
        "fixtures/inventory/python/docstrings.py",
        out.path(),
        &["--format", "jsonl", "--min-priority", "ignore"],
    );
    let records = read_jsonl(out.path());
    let configure_doc = records
        .iter()
        .find(|r| {
            r["normalized_text"]
                .as_str()
                .unwrap_or("")
                .contains("Console verbosity")
        })
        .expect("configure_logging docstring present");
    let sections = configure_doc["python"]["docstring_sections"]
        .as_object()
        .expect("docstring_sections object");
    assert!(
        sections.contains_key("Parameters"),
        "sections: {sections:?}"
    );
}

#[test]
fn by_symbol_uses_module_derived_symbol() {
    // GAP-4: module docstrings group under a dotted module symbol, not path:line.
    let out = tempfile::tempdir().unwrap();
    run_inventory(FIXTURES, out.path(), &["--format", "by-symbol"]);
    let json: Value = serde_json::from_str(
        &std::fs::read_to_string(out.path().join("comments_docstrings_by_symbol.json")).unwrap(),
    )
    .unwrap();
    let symbols: Vec<&str> = json["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["symbol"].as_str())
        .collect();
    assert!(
        symbols.iter().any(|s| s.ends_with(".docstrings")),
        "expected a module-derived symbol ending in .docstrings, got {symbols:?}"
    );
}
