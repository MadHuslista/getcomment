//! `inventory_manifest.json` writer (docs/output-contract.md §9).
//!
//! `generated_at` is the only nondeterministic field; everything else is stable
//! for a given input (FR-012).

use crate::inventory::model::SCHEMA_VERSION;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub schema_version: &'static str,
    pub tool: &'static str,
    pub tool_version: String,
    pub generated_at: String,
    pub root: String,
    pub paths: Vec<String>,
    pub formats: Vec<String>,
    pub languages_requested: Vec<String>,
    pub records: usize,
    pub files_scanned: usize,
    pub files_with_records: usize,
    pub errors: Vec<String>,
}

impl Manifest {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        root: String,
        paths: Vec<String>,
        formats: Vec<String>,
        languages_requested: Vec<String>,
        records: usize,
        files_scanned: usize,
        files_with_records: usize,
        errors: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            tool: "uncomment",
            tool_version: format!("{}+inventory", env!("CARGO_PKG_VERSION")),
            generated_at: now_rfc3339(),
            root,
            paths,
            formats,
            languages_requested,
            records,
            files_scanned,
            files_with_records,
            errors,
        }
    }
}

pub fn write(manifest: &Manifest, dir: &Path) -> Result<()> {
    let path = dir.join("inventory_manifest.json");
    let json = serde_json::to_string_pretty(manifest).context("serialize manifest")?;
    std::fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Current UTC time formatted as RFC3339 (`YYYY-MM-DDTHH:MM:SSZ`).
#[must_use]
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard Hinnant's days-from-civil inverse (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_correctly() {
        // 0 seconds since epoch = 1970-01-01T00:00:00Z
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn known_date_formats() {
        // 2021-01-01 is 18628 days after epoch.
        let (y, m, d) = civil_from_days(18_628);
        assert_eq!((y, m, d), (2021, 1, 1));
    }
}
