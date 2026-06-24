//! Python inventory enrichment.
//!
//! Reuses the removal pipeline's docstring detection (`PythonHandler`, AC-002)
//! and adds scope attachment, docstring section parsing, and Doxygen-like
//! `# @tag` extraction (FR-007).

use crate::inventory::collector::{self, Base, RawComment};
use crate::inventory::model::{DoxygenTag, EvidenceKind, InventoryRecord, PythonFields};
use crate::inventory::scoring::AttachedKind;
use crate::inventory::source_class::FileOrigin;
use anyhow::Result;
use tree_sitter::{Node, Tree};

/// Known docstring section headers (Google / NumPy / reST flavored).
const SECTION_HEADERS: &[&str] = &[
    "Args",
    "Arguments",
    "Parameters",
    "Returns",
    "Yields",
    "Raises",
    "Notes",
    "Note",
    "Warnings",
    "Warning",
    "Examples",
    "Example",
    "Deprecated",
    "See Also",
    "Test Strategy",
    "Regression",
];

pub(crate) fn collect(
    repo: &str,
    rel_path: &str,
    source: &str,
    tree: &Tree,
    raw: &[RawComment],
    file_origin: FileOrigin,
) -> Result<Vec<InventoryRecord>> {
    let is_test = is_test_file(rel_path);
    let root = tree.root_node();
    let mut records = Vec::with_capacity(raw.len());

    for c in raw {
        let raw_text = collector::slice(source, c.byte_start, c.byte_end).to_string();
        let node = root.descendant_for_byte_range(c.byte_start, c.byte_end);

        if c.is_docstring {
            records.push(build_docstring(
                repo,
                rel_path,
                source,
                &raw_text,
                c,
                node,
                is_test,
                file_origin,
            ));
        } else {
            records.push(build_comment(
                repo,
                rel_path,
                &raw_text,
                c,
                node,
                is_test,
                file_origin,
            ));
        }
    }

    Ok(records)
}

#[allow(clippy::too_many_arguments)]
fn build_docstring(
    repo: &str,
    rel_path: &str,
    source: &str,
    raw_text: &str,
    c: &RawComment,
    node: Option<Node>,
    is_test: bool,
    file_origin: FileOrigin,
) -> InventoryRecord {
    let (scope_type, scope_name, qualified) =
        node.map(|n| python_scope(n, source))
            .unwrap_or(("module".to_string(), None, None));

    let mut record = collector::base_record(Base {
        repo,
        path: rel_path,
        language: "python",
        kind: EvidenceKind::Docstring,
        comment_style: "docstring".to_string(),
        byte_start: c.byte_start,
        byte_end: c.byte_end,
        line_start: c.row_start + 1,
        line_end: c.row_end + 1,
        raw_text: raw_text.to_string(),
    });

    let sections = parse_sections(raw_text);
    record.scope_type = Some(scope_type.clone());
    record.scope_name = scope_name.clone();
    record.qualified_symbol = qualified;
    record.python = Some(PythonFields {
        is_docstring: true,
        docstring_scope: Some(scope_type.clone()),
        docstring_sections: sections,
        doxygen_tags: Vec::new(),
        is_test_file: is_test,
    });
    record.confidence = Some("high".to_string());

    let attached = match scope_type.as_str() {
        "class" => AttachedKind::Class,
        "function" | "method" => AttachedKind::Function,
        "module" => AttachedKind::Module,
        _ => AttachedKind::None,
    };
    collector::apply_score(&mut record, attached, file_origin, false);
    record
}

fn build_comment(
    repo: &str,
    rel_path: &str,
    raw_text: &str,
    c: &RawComment,
    node: Option<Node>,
    is_test: bool,
    file_origin: FileOrigin,
) -> InventoryRecord {
    let tags = parse_doxygen_tags(raw_text);
    let kind = if tags.is_empty() {
        EvidenceKind::Comment
    } else {
        EvidenceKind::DocumentationComment
    };

    let mut record = collector::base_record(Base {
        repo,
        path: rel_path,
        language: "python",
        kind,
        comment_style: if tags.is_empty() {
            "line"
        } else {
            "doxygen_line"
        }
        .to_string(),
        byte_start: c.byte_start,
        byte_end: c.byte_end,
        line_start: c.row_start + 1,
        line_end: c.row_end + 1,
        raw_text: raw_text.to_string(),
    });

    if let Some(n) = node {
        let (scope_type, scope_name) = enclosing_scope(n);
        record.scope_type = scope_type;
        record.scope_name = scope_name;
    }

    record.python = Some(PythonFields {
        is_docstring: false,
        docstring_scope: None,
        docstring_sections: Default::default(),
        doxygen_tags: tags,
        is_test_file: is_test,
    });

    collector::apply_score(&mut record, AttachedKind::Local, file_origin, false);
    record
}

/// Determine the docstring's owning scope by climbing the AST.
fn python_scope(node: Node, source: &str) -> (String, Option<String>, Option<String>) {
    // The docstring's owner is the nearest enclosing definition.
    let mut owner = None;
    let mut ancestors: Vec<(String, String)> = Vec::new();
    let mut cur = node.parent();
    while let Some(n) = cur {
        match n.kind() {
            "class_definition" | "function_definition" | "async_function_definition" => {
                let name = node_name(&n, source).unwrap_or_default();
                if owner.is_none() {
                    owner = Some(n);
                }
                ancestors.push((n.kind().to_string(), name));
            }
            _ => {}
        }
        cur = n.parent();
    }

    let Some(owner) = owner else {
        return ("module".to_string(), None, None);
    };

    let has_class_ancestor = ancestors
        .iter()
        .skip(1)
        .any(|(k, _)| k == "class_definition");
    let scope_type = match owner.kind() {
        "class_definition" => "class",
        _ if has_class_ancestor => "method",
        _ => "function",
    };

    let scope_name = node_name(&owner, source);
    // Build qualified name from outermost to innermost.
    let mut parts: Vec<String> = ancestors
        .iter()
        .rev()
        .map(|(_, name)| name.clone())
        .filter(|n| !n.is_empty())
        .collect();
    parts.dedup();
    let qualified = if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    };

    (scope_type.to_string(), scope_name, qualified)
}

/// Nearest enclosing class/function for an ordinary comment.
fn enclosing_scope(node: Node) -> (Option<String>, Option<String>) {
    let mut cur = node.parent();
    while let Some(n) = cur {
        match n.kind() {
            "class_definition" => return (Some("class".to_string()), None),
            "function_definition" | "async_function_definition" => {
                return (Some("function".to_string()), None);
            }
            _ => {}
        }
        cur = n.parent();
    }
    (None, None)
}

fn node_name(node: &Node, source: &str) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    name.utf8_text(source.as_bytes()).ok().map(str::to_string)
}

/// Parse `# @param name text`, `# @return text`, etc. from raw comment text.
fn parse_doxygen_tags(raw: &str) -> Vec<DoxygenTag> {
    let mut tags = Vec::new();
    for line in raw.lines() {
        let stripped = line.trim_start().trim_start_matches('#').trim();
        if !stripped.starts_with('@') {
            continue;
        }
        let mut parts = stripped.splitn(2, char::is_whitespace);
        let tag = parts.next().unwrap_or("").to_string();
        let rest = parts.next().unwrap_or("").trim();
        match tag.as_str() {
            "@param" | "@arg" => {
                let mut rp = rest.splitn(2, char::is_whitespace);
                let name = rp.next().unwrap_or("").to_string();
                let text = rp.next().unwrap_or("").trim().to_string();
                tags.push(DoxygenTag {
                    tag,
                    name: if name.is_empty() { None } else { Some(name) },
                    text,
                });
            }
            "@return" | "@returns" | "@brief" | "@note" | "@warning" | "@deprecated" => {
                tags.push(DoxygenTag {
                    tag,
                    name: None,
                    text: rest.to_string(),
                });
            }
            _ => {}
        }
    }
    tags
}

/// Parse docstring sections into a header -> body map.
///
/// Handles three flavors: reST directives (`.. note::`), inline `Header: text`
/// (Google one-liners collapsed onto a single line), and block sections — both
/// NumPy underline (`Parameters` over a `----` rule) and Google block (`Args:`
/// on its own line followed by an indented body).
fn parse_sections(raw: &str) -> std::collections::BTreeMap<String, String> {
    let mut sections = std::collections::BTreeMap::new();
    let cleaned = strip_docstring_quotes(raw);
    parse_inline_sections(cleaned, &mut sections);
    parse_block_sections(cleaned, &mut sections);
    sections
}

/// reST directives and single-line `Header: text` forms.
fn parse_inline_sections(text: &str, sections: &mut std::collections::BTreeMap<String, String>) {
    if let Some(idx) = text.find(".. note::") {
        let body = first_line(text[idx + ".. note::".len()..].trim());
        if !body.is_empty() {
            sections
                .entry("Notes".to_string())
                .or_insert_with(|| first_sentence(&body));
        }
    }
    if let Some(idx) = text.find(".. warning::") {
        let body = first_line(text[idx + ".. warning::".len()..].trim());
        if !body.is_empty() {
            sections
                .entry("Warnings".to_string())
                .or_insert_with(|| first_sentence(&body));
        }
    }
    // Single-line "Header: text" (skip a bare "Header:" — that is a block form).
    for header in SECTION_HEADERS {
        let needle = format!("{header}:");
        if let Some(idx) = text.find(&needle) {
            let rest = first_line(text[idx + needle.len()..].trim());
            if !rest.is_empty() {
                sections
                    .entry(canonical_section(header))
                    .or_insert_with(|| first_sentence(&rest));
            }
        }
    }
}

/// NumPy underline (`Parameters` / `-----`) and Google block (`Args:`) sections.
fn parse_block_sections(text: &str, sections: &mut std::collections::BTreeMap<String, String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let Some(header) = section_header(lines[i]) else {
            i += 1;
            continue;
        };
        let numpy = i + 1 < lines.len() && is_underline(lines[i + 1]);
        let google = lines[i].trim().ends_with(':');
        if !numpy && !google {
            i += 1;
            continue;
        }
        let mut j = if numpy { i + 2 } else { i + 1 };
        let mut body = Vec::new();
        while j < lines.len() {
            let line = lines[j].trim();
            if line.is_empty() || is_block_header(&lines, j) {
                break;
            }
            body.push(line);
            j += 1;
        }
        if !body.is_empty() {
            sections
                .entry(canonical_section(header))
                .or_insert_with(|| first_sentence(&body.join(" ")));
        }
        i = j.max(i + 1);
    }
}

/// The matched section header for a line, if the line is exactly a header
/// (bare or with a trailing colon), case-insensitively.
fn section_header(line: &str) -> Option<&'static str> {
    let bare = line.trim().strip_suffix(':').unwrap_or(line.trim()).trim();
    SECTION_HEADERS
        .iter()
        .copied()
        .find(|h| bare.eq_ignore_ascii_case(h))
}

/// True when line `idx` begins a new block section (NumPy underline or Google).
fn is_block_header(lines: &[&str], idx: usize) -> bool {
    section_header(lines[idx]).is_some()
        && (lines[idx].trim().ends_with(':')
            || (idx + 1 < lines.len() && is_underline(lines[idx + 1])))
}

fn is_underline(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.chars().all(|c| c == '-')
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().to_string()
}

/// Strip surrounding triple/single quote delimiters, preserving interior lines.
fn strip_docstring_quotes(raw: &str) -> &str {
    let s = raw.trim();
    let s = s
        .strip_prefix("r")
        .or_else(|| s.strip_prefix("R"))
        .unwrap_or(s);
    let s = s
        .strip_prefix("\"\"\"")
        .or_else(|| s.strip_prefix("'''"))
        .unwrap_or(s);
    s.strip_suffix("\"\"\"")
        .or_else(|| s.strip_suffix("'''"))
        .unwrap_or(s)
}

fn canonical_section(header: &str) -> String {
    match header {
        "Arguments" | "Args" | "Parameters" => "Parameters",
        "Note" | "Notes" => "Notes",
        "Warning" | "Warnings" => "Warnings",
        "Example" | "Examples" => "Examples",
        other => other,
    }
    .to_string()
}

fn first_sentence(text: &str) -> String {
    match text.find(". ") {
        Some(i) => text[..=i].trim().to_string(),
        None => text.trim().to_string(),
    }
}

fn is_test_file(rel_path: &str) -> bool {
    let lower = rel_path.replace('\\', "/").to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    lower.contains("/tests/") || file.starts_with("test_") || file.ends_with("_test.py")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doxygen_param_parsed() {
        let tags = parse_doxygen_tags("## @param level Logging level string.");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag, "@param");
        assert_eq!(tags[0].name.as_deref(), Some("level"));
        assert!(tags[0].text.contains("Logging level"));
    }

    #[test]
    fn rest_note_section_parsed() {
        let s = parse_sections(
            "Adaptive drift corrector. .. note:: Not used in the default configuration.",
        );
        assert!(s.contains_key("Notes"));
    }

    #[test]
    fn numpy_underline_sections_parsed() {
        // GAP-3: NumPy underline headers must populate docstring_sections.
        let raw = "\"\"\"Configure the package-root logger.\n\n    Parameters\n    ----------\n    level:\n        Console verbosity. One of DEBUG, INFO.\n    \"\"\"";
        let s = parse_sections(raw);
        assert!(s.contains_key("Parameters"), "sections: {s:?}");
        assert!(s["Parameters"].contains("level"));
    }

    #[test]
    fn google_block_sections_parsed() {
        let raw = "\"\"\"Do a thing.\n\n    Args:\n        level: the verbosity.\n\n    Returns:\n        nothing useful.\n    \"\"\"";
        let s = parse_sections(raw);
        assert!(
            s.contains_key("Parameters"),
            "Args canonicalizes to Parameters"
        );
        assert!(s.contains_key("Returns"));
    }

    #[test]
    fn test_file_detection() {
        assert!(is_test_file("tests/test_x.py"));
        assert!(!is_test_file("pkg/util.py"));
    }
}
