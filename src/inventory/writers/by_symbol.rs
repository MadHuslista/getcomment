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
        .unwrap_or_else(|| format!("{}:{}", r.path, r.line_start))
}
