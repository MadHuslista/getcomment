//! By-symbol JSON writer: groups records under their qualified symbol or YAML
//! key path for quick lookup (`comments_docstrings_by_symbol.json`).

use crate::inventory::model::InventoryRecord;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

pub fn write(records: &[InventoryRecord], dir: &Path) -> Result<()> {
    let path = dir.join("comments_docstrings_by_symbol.json");

    let mut groups: BTreeMap<String, Vec<&InventoryRecord>> = BTreeMap::new();
    for record in records {
        groups.entry(symbol_key(record)).or_default().push(record);
    }

    let value = json!({
        "schema_version": crate::inventory::model::SCHEMA_VERSION,
        "symbols": groups
            .into_iter()
            .map(|(symbol, recs)| {
                json!({
                    "symbol": symbol,
                    "count": recs.len(),
                    "record_ids": recs.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
    });

    let json = serde_json::to_string_pretty(&value).context("serialize by-symbol")?;
    std::fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn symbol_key(r: &InventoryRecord) -> String {
    r.qualified_symbol
        .clone()
        .or_else(|| {
            r.c_family
                .as_ref()
                .and_then(|c| c.target_declaration_name.clone())
        })
        .or_else(|| r.yaml.as_ref().and_then(|y| y.key_path.clone()))
        .or_else(|| r.scope_name.clone())
        .or_else(|| module_symbol(r))
        .unwrap_or_else(|| format!("{}:{}", r.path, r.line_start))
}

/// Derive a dotted module symbol from the file path for module-scope Python
/// docstrings, which otherwise have no qualified symbol (GAP-4).
fn module_symbol(r: &InventoryRecord) -> Option<String> {
    let is_module_docstring = r
        .python
        .as_ref()
        .is_some_and(|p| p.docstring_scope.as_deref() == Some("module"));
    if !is_module_docstring {
        return None;
    }
    let path = r.path.replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    let path = path.strip_suffix(".py").unwrap_or(path);
    let parts: Vec<&str> = path
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != "src")
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::model::{EvidenceKind, Priority, PythonFields, SCHEMA_VERSION};

    fn module_docstring_record(path: &str) -> InventoryRecord {
        InventoryRecord {
            schema_version: SCHEMA_VERSION,
            id: "docstring:x:1:1:abcd".into(),
            repo: "r".into(),
            path: path.into(),
            language: "python".into(),
            line_start: 1,
            line_end: 1,
            byte_start: 0,
            byte_end: 1,
            kind: EvidenceKind::Docstring,
            comment_style: "docstring".into(),
            raw_text: String::new(),
            normalized_text: String::new(),
            source_hash: String::new(),
            markers: vec![],
            requirement_ids: vec![],
            domain_terms: vec![],
            claim_types: vec![],
            score: 3,
            priority: Priority::Low,
            score_reasons: vec![],
            scope_type: Some("module".into()),
            scope_name: None,
            qualified_symbol: None,
            nearby_signature: None,
            nearest_previous_symbol: None,
            nearest_following_symbol: None,
            file_origin: None,
            contradiction_targets: vec![],
            confidence: None,
            python: Some(PythonFields {
                is_docstring: true,
                docstring_scope: Some("module".into()),
                ..Default::default()
            }),
            c_family: None,
            yaml: None,
        }
    }

    #[test]
    fn module_docstring_uses_dotted_path_symbol() {
        // GAP-4: module docstrings group under a module-derived symbol, not path:line.
        let r =
            module_docstring_record("./Handgrip_Calibration/scripts/analyze_step_relaxation.py");
        assert_eq!(
            symbol_key(&r),
            "Handgrip_Calibration.scripts.analyze_step_relaxation"
        );
    }

    #[test]
    fn module_symbol_drops_src_segment() {
        let r = module_docstring_record("src/handgrip_calibration/cli.py");
        assert_eq!(symbol_key(&r), "handgrip_calibration.cli");
    }
}
