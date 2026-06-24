//! Summary writer (docs/output-contract.md §8): counts by language, priority,
//! claim type, marker, and file origin, plus top high-priority files/symbols.

use crate::inventory::model::{InventoryRecord, Priority};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

pub fn write(records: &[InventoryRecord], suppressed: usize, dir: &Path) -> Result<()> {
    let path = dir.join("comments_docstrings_summary.md");
    let mut out = String::from("# Comment Inventory Summary\n\n");

    let _ = writeln!(out, "Total records: {}\n", records.len());

    write_counts(
        &mut out,
        "Records by language",
        count_by(records, |r| r.language.clone()),
    );
    write_counts(
        &mut out,
        "Records by priority",
        count_by(records, |r| r.priority.as_str().to_string()),
    );
    write_counts(
        &mut out,
        "Records by claim type",
        count_multi(records, |r| r.claim_types.clone()),
    );
    write_counts(
        &mut out,
        "Records by marker",
        count_multi(records, |r| r.markers.clone()),
    );
    write_counts(
        &mut out,
        "Records by file origin",
        count_by(records, |r| {
            r.file_origin.clone().unwrap_or_else(|| "unknown".into())
        }),
    );

    let high: Vec<&InventoryRecord> = records
        .iter()
        .filter(|r| r.priority == Priority::High)
        .collect();
    write_counts(
        &mut out,
        "Top high-priority files",
        count_refs(&high, |r| r.path.clone()),
    );
    write_counts(
        &mut out,
        "Top high-priority symbols/key paths",
        count_refs(&high, symbol_of),
    );

    let _ = writeln!(
        out,
        "## Generated/vendor records suppressed or down-ranked\n\n{suppressed}\n"
    );

    std::fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn symbol_of(r: &InventoryRecord) -> String {
    r.qualified_symbol
        .clone()
        .or_else(|| {
            r.c_family
                .as_ref()
                .and_then(|c| c.target_declaration_name.clone())
        })
        .or_else(|| r.yaml.as_ref().and_then(|y| y.key_path.clone()))
        .or_else(|| r.scope_name.clone())
        .unwrap_or_else(|| "(none)".into())
}

fn count_by(
    records: &[InventoryRecord],
    key: impl Fn(&InventoryRecord) -> String,
) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for r in records {
        *map.entry(key(r)).or_insert(0) += 1;
    }
    map
}

fn count_refs(
    records: &[&InventoryRecord],
    key: impl Fn(&InventoryRecord) -> String,
) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for r in records {
        *map.entry(key(r)).or_insert(0) += 1;
    }
    map
}

fn count_multi(
    records: &[InventoryRecord],
    keys: impl Fn(&InventoryRecord) -> Vec<String>,
) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for r in records {
        for k in keys(r) {
            *map.entry(k).or_insert(0) += 1;
        }
    }
    map
}

fn write_counts(out: &mut String, title: &str, counts: BTreeMap<String, usize>) {
    let _ = writeln!(out, "## {title}\n");
    if counts.is_empty() {
        out.push_str("(none)\n\n");
        return;
    }
    // Sort by descending count, then key for determinism.
    let mut entries: Vec<(String, usize)> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (key, count) in entries {
        let _ = writeln!(out, "- {key}: {count}");
    }
    out.push('\n');
}
