//! C/C++ inventory enrichment.
//!
//! Adds Doxygen target attachment, inline field comments, USER CODE region
//! metadata, and `#warning`/`#error` preprocessor diagnostics (FR-008). Comment
//! detection stays parser-based; declaration attachment is a line heuristic with
//! a `confidence` field (NFR-001).

use crate::inventory::collector::{self, Base, RawComment};
use crate::inventory::model::{CFamilyFields, EvidenceKind, InventoryRecord};
use crate::inventory::scoring::AttachedKind;
use crate::inventory::source_class::FileOrigin;
use anyhow::Result;

/// A `USER CODE BEGIN/END` region with its label and row span.
struct UserCodeRegion {
    label: String,
    begin_row: usize,
    end_row: usize,
}

pub(crate) fn collect(
    repo: &str,
    rel_path: &str,
    source: &str,
    raw: &[RawComment],
    file_origin: FileOrigin,
) -> Result<Vec<InventoryRecord>> {
    let regions = parse_user_code_regions(source);
    let line_starts = line_start_offsets(source);
    let language = if rel_path_is_cpp(rel_path) {
        "cpp"
    } else {
        "c"
    };

    let mut records = Vec::new();

    for c in raw {
        let raw_text = collector::slice(source, c.byte_start, c.byte_end).to_string();

        // USER CODE markers are region metadata, not claims (AC-004).
        if is_user_code_marker(&raw_text) {
            continue;
        }

        let style = collector::c_comment_style(&raw_text);
        let is_inline_field = style == "inline_field";

        let (target_kind, target_name, target_text, attached) = if is_inline_field {
            attach_inline_field(source, &line_starts, c)
        } else {
            attach_following_declaration(source, c)
        };

        let region = regions
            .iter()
            .find(|r| c.row_start > r.begin_row && c.row_end < r.end_row);

        let kind = if style.starts_with("doxygen") || is_inline_field {
            EvidenceKind::DocumentationComment
        } else {
            EvidenceKind::Comment
        };

        let mut record = collector::base_record(Base {
            repo,
            path: rel_path,
            language,
            kind,
            comment_style: style.to_string(),
            byte_start: c.byte_start,
            byte_end: c.byte_end,
            line_start: c.row_start + 1,
            line_end: c.row_end + 1,
            raw_text,
        });

        record.c_family = Some(CFamilyFields {
            target_declaration_kind: target_kind.clone(),
            target_declaration_name: target_name.clone(),
            target_declaration_text: target_text.clone(),
            inside_user_code_region: region.is_some(),
            user_code_region: region.map(|r| r.label.clone()),
            is_inline_field_comment: is_inline_field,
            is_preprocessor_diagnostic: false,
        });
        record.scope_name = target_name;
        record.nearby_signature = target_text;
        record.confidence = Some(
            if target_kind.is_some() {
                "medium"
            } else {
                "low"
            }
            .to_string(),
        );

        collector::apply_score(&mut record, attached, file_origin, false);
        records.push(record);
    }

    // Preprocessor diagnostics: line scan, separate evidence kind (not comments).
    for diag in scan_preprocessor_diagnostics(source, raw) {
        let mut record = collector::base_record(Base {
            repo,
            path: rel_path,
            language,
            kind: EvidenceKind::PreprocessorDiagnostic,
            comment_style: "preprocessor".to_string(),
            byte_start: diag.byte_start,
            byte_end: diag.byte_end,
            line_start: diag.row + 1,
            line_end: diag.row + 1,
            raw_text: diag.text,
        });
        record.c_family = Some(CFamilyFields {
            is_preprocessor_diagnostic: true,
            ..Default::default()
        });
        record.confidence = Some("high".to_string());
        collector::apply_score(&mut record, AttachedKind::Local, file_origin, false);
        records.push(record);
    }

    Ok(records)
}

fn rel_path_is_cpp(rel_path: &str) -> bool {
    let lower = rel_path.to_lowercase();
    [".cpp", ".cxx", ".cc", ".c++", ".hpp", ".hxx", ".hh", ".h++"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

fn is_user_code_marker(raw: &str) -> bool {
    raw.contains("USER CODE BEGIN") || raw.contains("USER CODE END")
}

fn parse_user_code_regions(source: &str) -> Vec<UserCodeRegion> {
    let mut regions = Vec::new();
    let mut open: Vec<(String, usize)> = Vec::new();
    for (row, line) in source.lines().enumerate() {
        if let Some(idx) = line.find("USER CODE BEGIN") {
            let rest = &line[idx + "USER CODE BEGIN".len()..];
            let label = rest.trim().trim_end_matches("*/").trim().to_string();
            open.push((label, row));
        } else if line.contains("USER CODE END")
            && let Some((label, begin_row)) = open.pop()
        {
            regions.push(UserCodeRegion {
                label,
                begin_row,
                end_row: row,
            });
        }
    }
    regions
}

/// Attach a leading comment to the next non-empty declaration line.
fn attach_following_declaration(
    source: &str,
    c: &RawComment,
) -> (Option<String>, Option<String>, Option<String>, AttachedKind) {
    let after = &source[c.byte_end.min(source.len())..];
    for line in after.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return parse_declaration(trimmed);
    }
    (None, None, None, AttachedKind::None)
}

/// Attach an inline `/**< */` field comment to the declaration on its own line.
fn attach_inline_field(
    source: &str,
    line_starts: &[usize],
    c: &RawComment,
) -> (Option<String>, Option<String>, Option<String>, AttachedKind) {
    let line_start = line_starts
        .iter()
        .rev()
        .find(|&&off| off <= c.byte_start)
        .copied()
        .unwrap_or(0);
    let before = collector::slice(source, line_start, c.byte_start).trim();
    if before.is_empty() {
        return (None, None, None, AttachedKind::Local);
    }
    let name = field_name(before);
    (
        Some("field".to_string()),
        name,
        Some(before.trim_end_matches(';').trim().to_string() + ";"),
        AttachedKind::Local,
    )
}

/// Parse a single declaration line into (kind, name, text, attachment).
fn parse_declaration(line: &str) -> (Option<String>, Option<String>, Option<String>, AttachedKind) {
    let text = Some(line.trim_end().to_string());

    if let Some(rest) = line.strip_prefix("#define") {
        let name = rest
            .trim()
            .split(|c: char| c.is_whitespace() || c == '(')
            .next();
        return (
            Some("macro".to_string()),
            name.filter(|n| !n.is_empty()).map(str::to_string),
            text,
            AttachedKind::Macro,
        );
    }
    if line.starts_with("static const") || line.starts_with("const ") || line.contains(" const ") {
        return (
            Some("constant".to_string()),
            const_name(line),
            text,
            AttachedKind::Constant,
        );
    }
    if line.starts_with("typedef") {
        return (
            Some("typedef".to_string()),
            typedef_name(line),
            text,
            AttachedKind::None,
        );
    }
    if line.starts_with("struct") || line.contains(" struct ") {
        return (Some("struct".to_string()), None, text, AttachedKind::None);
    }
    if line.starts_with("enum") || line.contains(" enum ") {
        return (Some("enum".to_string()), None, text, AttachedKind::None);
    }
    if line.contains('(') && line.contains(')') {
        return (
            Some("function".to_string()),
            function_name(line),
            text,
            AttachedKind::Function,
        );
    }
    (None, None, text, AttachedKind::None)
}

fn const_name(line: &str) -> Option<String> {
    // Take the identifier just before '=' or ';'.
    let head = line.split('=').next().unwrap_or(line);
    last_identifier(head.trim_end_matches(';'))
}

fn typedef_name(line: &str) -> Option<String> {
    last_identifier(line.trim_end_matches(';').trim_end_matches('}'))
}

fn function_name(line: &str) -> Option<String> {
    let before_paren = line.split('(').next()?;
    last_identifier(before_paren)
}

fn field_name(decl: &str) -> Option<String> {
    last_identifier(decl.trim_end_matches(';'))
}

/// Last C identifier in a string (handles `*`, `[]`, qualifiers).
fn last_identifier(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .last()
        .filter(|t| {
            t.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        })
        .map(str::to_string)
}

fn line_start_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

struct Diagnostic {
    byte_start: usize,
    byte_end: usize,
    row: usize,
    text: String,
}

fn scan_preprocessor_diagnostics(source: &str, comments: &[RawComment]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut offset = 0usize;
    for (row, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#warning") || trimmed.starts_with("#error") {
            let start = offset + (line.len() - trimmed.len());
            let end = offset + line.len();
            let in_comment = comments
                .iter()
                .any(|c| start >= c.byte_start && start < c.byte_end);
            if !in_comment {
                diags.push(Diagnostic {
                    byte_start: start,
                    byte_end: end,
                    row,
                    text: trimmed.to_string(),
                });
            }
        }
        offset += line.len() + 1; // account for '\n'
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_declaration_parsed() {
        let (kind, name, _, attached) = parse_declaration("#define ROW_STRIDE_BITS (1560U)");
        assert_eq!(kind.as_deref(), Some("macro"));
        assert_eq!(name.as_deref(), Some("ROW_STRIDE_BITS"));
        assert_eq!(attached, AttachedKind::Macro);
    }

    #[test]
    fn inline_field_name_extracted() {
        assert_eq!(field_name("uint32_t exact"), Some("exact".to_string()));
    }

    #[test]
    fn user_code_region_span() {
        let src = "/* USER CODE BEGIN Includes */\n// note\n/* USER CODE END Includes */\n";
        let regions = parse_user_code_regions(src);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].label, "Includes");
        assert!(regions[0].begin_row < 1 && 1 < regions[0].end_row);
    }

    #[test]
    fn preprocessor_warning_scanned() {
        let src = "#warning \"mismatch\"\nint x;\n";
        let diags = scan_preprocessor_diagnostics(src, &[]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].text.contains("mismatch"));
    }
}
