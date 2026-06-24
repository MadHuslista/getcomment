//! JSONL writer: one JSON object per line (AC-007), in deterministic order.

use crate::inventory::model::InventoryRecord;
use anyhow::{Context, Result};
use std::path::Path;

pub fn write(records: &[InventoryRecord], dir: &Path) -> Result<()> {
    let path = dir.join("comments_docstrings.jsonl");
    let mut out = String::new();
    for record in records {
        let line = serde_json::to_string(record).context("serialize record")?;
        out.push_str(&line);
        out.push('\n');
    }
    std::fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
