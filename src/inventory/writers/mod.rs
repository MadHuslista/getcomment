//! Output artifact writers and dispatch.

pub mod by_symbol;
pub mod jsonl;
pub mod manifest;
pub mod markdown;
pub mod summary;

pub use manifest::Manifest;

use crate::inventory::InventoryFormat;
use crate::inventory::model::InventoryRecord;
use anyhow::{Context, Result};
use std::path::Path;

/// Write the requested artifacts plus the always-present manifest.
pub fn write_outputs(
    records: &[InventoryRecord],
    formats: &[InventoryFormat],
    suppressed: usize,
    dir: &Path,
    manifest: &Manifest,
) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("create output dir {}", dir.display()))?;

    if formats.contains(&InventoryFormat::Jsonl) {
        jsonl::write(records, dir)?;
    }
    if formats.contains(&InventoryFormat::Markdown) {
        markdown::write(records, dir)?;
    }
    if formats.contains(&InventoryFormat::Summary) {
        summary::write(records, suppressed, dir)?;
    }
    if formats.contains(&InventoryFormat::BySymbol) {
        by_symbol::write(records, dir)?;
    }

    manifest::write(manifest, dir)?;
    Ok(())
}
