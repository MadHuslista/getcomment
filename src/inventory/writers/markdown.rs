//! Markdown writer optimized for retrieval, not presentation (FR-005, §7).

use crate::inventory::model::{InventoryRecord, Priority};
use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::path::Path;

pub fn write(records: &[InventoryRecord], dir: &Path) -> Result<()> {
    let path = dir.join("comments_docstrings.md");
    let mut out = String::from("# Comments and Docstrings Inventory\n\n");

    for record in records {
        if record.priority == Priority::Ignore {
            continue;
        }
        render_record(&mut out, record);
    }

    std::fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn render_record(out: &mut String, r: &InventoryRecord) {
    let symbol = heading_symbol(r);
    let heading = match &symbol {
        Some(s) => format!("## {}:{}-{} — {}\n\n", r.path, r.line_start, r.line_end, s),
        None => format!("## {}:{}-{}\n\n", r.path, r.line_start, r.line_end),
    };
    out.push_str(&heading);

    let _ = writeln!(out, "Type: {}", kind_str(r));
    let _ = writeln!(out, "Priority: {}", r.priority.as_str());
    let _ = writeln!(out, "Score: {}", r.score);
    if !r.claim_types.is_empty() {
        let _ = writeln!(out, "Claim types: {}", r.claim_types.join(", "));
    }
    if !r.domain_terms.is_empty() {
        let _ = writeln!(out, "Domain terms: {}", r.domain_terms.join(", "));
    }
    if !r.markers.is_empty() {
        let _ = writeln!(out, "Markers: {}", r.markers.join(", "));
    }
    let _ = writeln!(out, "Language: {}", r.language);
    if let Some(origin) = &r.file_origin {
        let _ = writeln!(out, "File origin: {origin}");
    }
    if let Some(s) = &symbol {
        let _ = writeln!(out, "Symbol: `{s}`");
    }
    if let Some(y) = &r.yaml {
        if let Some(kp) = &y.key_path {
            let _ = writeln!(out, "Key path: `{kp}`");
        }
        if !y.allowed_values_hint.is_empty() {
            let _ = writeln!(out, "Allowed values: {}", y.allowed_values_hint.join(", "));
        }
    }

    let _ = write!(
        out,
        "\nExtracted text:\n\n> {}\n\n",
        r.normalized_text.replace('\n', " ")
    );

    if !r.score_reasons.is_empty() {
        out.push_str("Score reasons:\n\n");
        for reason in &r.score_reasons {
            let _ = writeln!(out, "- {reason}");
        }
        out.push('\n');
    }

    if !r.contradiction_targets.is_empty() {
        out.push_str("Potential contradiction targets:\n\n");
        for target in &r.contradiction_targets {
            let _ = writeln!(out, "- `{target}`");
        }
        out.push('\n');
    }
}

fn heading_symbol(r: &InventoryRecord) -> Option<String> {
    if let Some(q) = &r.qualified_symbol {
        return Some(q.clone());
    }
    if let Some(c) = &r.c_family
        && let Some(name) = &c.target_declaration_name
    {
        return Some(name.clone());
    }
    if let Some(y) = &r.yaml
        && let Some(kp) = &y.key_path
    {
        return Some(kp.clone());
    }
    r.scope_name.clone()
}

fn kind_str(r: &InventoryRecord) -> &'static str {
    use crate::inventory::model::EvidenceKind::*;
    match r.kind {
        Comment => "comment",
        Docstring => "docstring",
        DocumentationComment => "documentation_comment",
        YamlValue => "yaml_value",
        YamlValueWithComment => "yaml_value_with_comment",
        PreprocessorDiagnostic => "preprocessor_diagnostic",
    }
}
