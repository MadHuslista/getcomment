//! Per-file evidence collection.
//!
//! Parses a source file with tree-sitter (reusing the same grammar loading as
//! the removal pipeline in `src/processor.rs`) and dispatches to a
//! language-specific enricher. Comment detection is always parser-based
//! (NFR-001); line heuristics are used only for context attachment.

use crate::inventory::ids;
use crate::inventory::model::{EvidenceKind, InventoryRecord};
use crate::inventory::scoring::{self, AttachedKind, ScoreContext};
use crate::inventory::source_class::{self, FileOrigin};
use crate::inventory::text;
use crate::inventory::{c_family, python, yaml};
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

/// A comment-like node discovered in the parse tree.
pub(crate) struct RawComment {
    pub byte_start: usize,
    pub byte_end: usize,
    pub row_start: usize,
    pub row_end: usize,
    pub is_docstring: bool,
}

/// Generic inputs for building the common portion of a record.
pub(crate) struct Base<'a> {
    pub repo: &'a str,
    pub path: &'a str,
    pub language: &'a str,
    pub kind: EvidenceKind,
    pub comment_style: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub raw_text: String,
}

/// Build a record with all generic fields filled (id, hashes, markers, domain
/// terms, requirement IDs). Enrichment and scoring are applied afterward.
pub(crate) fn base_record(b: Base) -> InventoryRecord {
    let normalized_text = text::normalize_text(&b.raw_text);
    let source_hash = ids::source_hash(&b.raw_text);
    let short = ids::short_hash(
        b.path,
        b.language,
        b.kind.id_prefix(),
        b.line_start,
        &normalized_text,
    );
    let id = format!(
        "{}:{}:{}:{}:{}",
        b.kind.id_prefix(),
        b.path,
        b.line_start,
        b.line_end,
        short
    );

    InventoryRecord {
        schema_version: crate::inventory::model::SCHEMA_VERSION,
        id,
        repo: b.repo.to_string(),
        path: b.path.to_string(),
        language: b.language.to_string(),
        line_start: b.line_start,
        line_end: b.line_end,
        byte_start: b.byte_start,
        byte_end: b.byte_end,
        kind: b.kind,
        comment_style: b.comment_style,
        markers: text::markers(&normalized_text),
        requirement_ids: text::requirement_ids(&normalized_text),
        domain_terms: text::domain_terms(&normalized_text),
        normalized_text,
        raw_text: b.raw_text,
        source_hash,
        claim_types: Vec::new(),
        score: 0,
        priority: crate::inventory::model::Priority::Ignore,
        score_reasons: Vec::new(),
        scope_type: None,
        scope_name: None,
        qualified_symbol: None,
        nearby_signature: None,
        nearest_previous_symbol: None,
        nearest_following_symbol: None,
        file_origin: None,
        contradiction_targets: Vec::new(),
        confidence: None,
        python: None,
        c_family: None,
        yaml: None,
    }
}

/// Apply file origin + scoring to a record in place.
pub(crate) fn apply_score(
    record: &mut InventoryRecord,
    attached_kind: AttachedKind,
    file_origin: FileOrigin,
    is_section_separator: bool,
) {
    record.file_origin = Some(file_origin.as_str().to_string());
    record.contradiction_targets = default_contradiction_targets(record);
    let ctx = ScoreContext {
        attached_kind,
        file_origin,
        is_section_separator,
    };
    let scored = scoring::classify_and_score(record, &ctx);
    record.score = scored.score;
    record.priority = scored.priority;
    // Merge claim types discovered during enrichment with scoring output.
    for claim in scored.claim_types {
        if !record.claim_types.contains(&claim) {
            record.claim_types.push(claim);
        }
    }
    record.score_reasons = scored.score_reasons;
}

fn default_contradiction_targets(record: &InventoryRecord) -> Vec<String> {
    // Heuristic hint set: docs and tests are the usual contradiction surfaces.
    let mut targets = vec!["docs/**/*.md".to_string()];
    if record.language == "python" {
        targets.push("tests/**/*.py".to_string());
    } else {
        targets.push("tests/**/*".to_string());
    }
    targets
}

/// Collect inventory records for one already-read source file.
///
/// `include_yaml_values` controls whether active YAML facts (with no comment)
/// are emitted.
pub fn collect_file(
    repo: &str,
    rel_path: &str,
    source: &str,
    language: &str,
    tslp_name: &str,
    include_yaml_values: bool,
) -> Result<Vec<InventoryRecord>> {
    let file_origin = source_class::classify(rel_path);

    if language == "yaml" {
        // YAML uses a line-oriented walk for key paths plus tree-sitter comments.
        return yaml::collect(repo, rel_path, source, file_origin, include_yaml_values);
    }

    let ts_language = tree_sitter_language_pack::get_language(tslp_name)
        .with_context(|| format!("Failed to load grammar for '{language}'"))?;
    let mut parser = Parser::new();
    parser
        .set_language(&ts_language)
        .context("Failed to set parser language")?;
    let tree = parser
        .parse(source, None)
        .context("Failed to parse source")?;

    let raw = collect_raw_comments(&tree.root_node(), source, language);

    match language {
        "python" => python::collect(repo, rel_path, source, &tree, &raw, file_origin),
        "c" | "cpp" => c_family::collect(repo, rel_path, source, &raw, file_origin),
        _ => Ok(generic_collect(
            repo,
            rel_path,
            language,
            source,
            &raw,
            file_origin,
        )),
    }
}

/// Slice raw text for a node, clamped to source bounds.
pub(crate) fn slice(source: &str, start: usize, end: usize) -> &str {
    let end = end.min(source.len());
    if start >= end {
        return "";
    }
    source.get(start..end).unwrap_or("")
}

/// Detect the C-family comment style from raw comment text.
pub(crate) fn c_comment_style(raw: &str) -> &'static str {
    let t = raw.trim_start();
    if t.starts_with("/**<")
        || t.starts_with("/*!<")
        || t.starts_with("///<")
        || t.starts_with("//!<")
    {
        "inline_field"
    } else if t.starts_with("/**") || t.starts_with("/*!") {
        "doxygen_block"
    } else if t.starts_with("///") || t.starts_with("//!") {
        "doxygen_line"
    } else if t.starts_with("/*") {
        "block"
    } else {
        "line"
    }
}

/// Walk the tree collecting comment / docstring nodes (parser-based detection).
fn collect_raw_comments(root: &Node, source: &str, language: &str) -> Vec<RawComment> {
    let mut out = Vec::new();
    let handler = crate::languages::get_handler(language);
    walk(root, None, source, handler.as_ref(), &mut out);
    out.sort_by_key(|c| (c.byte_start, c.byte_end));
    out
}

fn walk(
    node: &Node,
    parent: Option<Node>,
    source: &str,
    handler: &dyn crate::languages::LanguageHandler,
    out: &mut Vec<RawComment>,
) {
    let kind = node.kind();
    let mut is_docstring = false;
    let mut keep = false;

    if kind == "comment" || kind.ends_with("_comment") {
        keep = true;
    } else if kind == "string" {
        // Python docstring detection reuses the removal-pipeline handler (AC-002).
        if let Some(true) = handler.is_documentation_comment(node, parent, source) {
            keep = true;
            is_docstring = true;
        }
    }

    if keep {
        out.push(RawComment {
            byte_start: node.start_byte(),
            byte_end: node.end_byte(),
            row_start: node.start_position().row,
            row_end: node.end_position().row,
            is_docstring,
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(&child, Some(*node), source, handler, out);
    }
}

/// Fallback collector for languages without dedicated enrichment.
fn generic_collect(
    repo: &str,
    rel_path: &str,
    language: &str,
    source: &str,
    raw: &[RawComment],
    file_origin: FileOrigin,
) -> Vec<InventoryRecord> {
    let mut records = Vec::with_capacity(raw.len());
    for c in raw {
        let raw_text = slice(source, c.byte_start, c.byte_end).to_string();
        let mut record = base_record(Base {
            repo,
            path: rel_path,
            language,
            kind: EvidenceKind::Comment,
            comment_style: "line".to_string(),
            byte_start: c.byte_start,
            byte_end: c.byte_end,
            line_start: c.row_start + 1,
            line_end: c.row_end + 1,
            raw_text,
        });
        apply_score(&mut record, AttachedKind::Local, file_origin, false);
        records.push(record);
    }
    records
}
