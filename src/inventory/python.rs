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

    let sections = parse_sections(&record.normalized_text);
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
fn parse_sections(normalized: &str) -> std::collections::BTreeMap<String, String> {
    let mut sections = std::collections::BTreeMap::new();
    // reST directive form: ".. note:: text"
    if let Some(idx) = normalized.find(".. note::") {
        let text = normalized[idx + ".. note::".len()..].trim();
        if !text.is_empty() {
            sections.insert("Notes".to_string(), first_sentence(text));
        }
    }
    if let Some(idx) = normalized.find(".. warning::") {
        let text = normalized[idx + ".. warning::".len()..].trim();
        if !text.is_empty() {
            sections.insert("Warnings".to_string(), first_sentence(text));
        }
    }
    // Header form: "Returns: text" / "Args: text".
    for header in SECTION_HEADERS {
        let needle = format!("{header}:");
        if let Some(idx) = normalized.find(&needle) {
            let text = normalized[idx + needle.len()..].trim();
            if !text.is_empty() {
                sections
                    .entry(canonical_section(header))
                    .or_insert_with(|| first_sentence(text));
            }
        }
    }
    sections
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
    fn test_file_detection() {
        assert!(is_test_file("tests/test_x.py"));
        assert!(!is_test_file("pkg/util.py"));
    }
}
