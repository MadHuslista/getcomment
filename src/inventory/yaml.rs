//! YAML / Hydra inventory enrichment.
//!
//! Comment detection uses a quote-aware line scanner (not regex), and a
//! deterministic indent stack reconstructs full key paths and active value
//! facts (FR-009). Every record carries an `attachment_confidence` because the
//! attachment is heuristic.

use crate::inventory::collector::{self, Base};
use crate::inventory::model::{EvidenceKind, InventoryRecord, YamlFields};
use crate::inventory::scoring::AttachedKind;
use crate::inventory::source_class::FileOrigin;
use anyhow::Result;
use serde_json::Value;

pub fn collect(
    repo: &str,
    rel_path: &str,
    source: &str,
    file_origin: FileOrigin,
    include_values: bool,
) -> Result<Vec<InventoryRecord>> {
    let is_hydra = detect_hydra(rel_path, source);
    let mut walker = Walker::new(
        repo,
        rel_path,
        source,
        file_origin,
        include_values,
        is_hydra,
    );
    walker.run();
    Ok(walker.records)
}

fn detect_hydra(rel_path: &str, source: &str) -> bool {
    let lower = rel_path.to_lowercase();
    source.contains("# @package")
        || source.lines().any(|l| l.trim_start() == "defaults:")
        || lower.contains("/conf/")
        || lower.contains("/config/")
}

struct Walker<'a> {
    repo: &'a str,
    path: &'a str,
    source: &'a str,
    file_origin: FileOrigin,
    include_values: bool,
    is_hydra: bool,
    hydra_package: Option<String>,
    records: Vec<InventoryRecord>,

    stack: Vec<(usize, String)>,
    pending_leading: Vec<String>,
    list_path: Option<String>,
    list_indent: usize,
    list_counter: usize,
    line_offset: usize,
}

impl<'a> Walker<'a> {
    fn new(
        repo: &'a str,
        path: &'a str,
        source: &'a str,
        file_origin: FileOrigin,
        include_values: bool,
        is_hydra: bool,
    ) -> Self {
        Self {
            repo,
            path,
            source,
            file_origin,
            include_values,
            is_hydra,
            hydra_package: None,
            records: Vec::new(),
            stack: Vec::new(),
            pending_leading: Vec::new(),
            list_path: None,
            list_indent: 0,
            list_counter: 0,
            line_offset: 0,
        }
    }

    fn run(&mut self) {
        // Two passes are not needed; clone lines to avoid borrow conflicts.
        let lines: Vec<(usize, String)> = self
            .source
            .lines()
            .enumerate()
            .map(|(r, l)| (r, l.to_string()))
            .collect();
        let mut offset = 0usize;
        for (row, line) in lines {
            self.line_offset = offset;
            offset += line.len() + 1;
            self.process_line(row, &line);
        }
    }

    fn process_line(&mut self, row: usize, line: &str) {
        let indent = line.len() - line.trim_start().len();
        let content = line.trim();

        if content.is_empty() {
            self.pending_leading.clear();
            return;
        }

        // Pure comment line.
        if content.starts_with('#') {
            self.handle_comment_line(row, line, indent, content);
            return;
        }

        // Split off any inline comment (quote-aware).
        let (code, inline_comment) = split_inline_comment(line);
        let code_trim = code.trim();

        if let Some(item) = code_trim
            .strip_prefix("- ")
            .or_else(|| if code_trim == "-" { Some("") } else { None })
        {
            self.handle_list_item(row, line, indent, item, inline_comment);
            return;
        }

        if let Some(colon) = find_key_colon(code_trim) {
            let key = code_trim[..colon].trim().to_string();
            let value = code_trim[colon + 1..].trim().to_string();
            self.handle_key(row, line, indent, &key, &value, inline_comment);
        }
    }

    fn handle_comment_line(&mut self, row: usize, line: &str, indent: usize, content: &str) {
        let body = content.trim_start_matches('#').trim();

        // Hydra package marker.
        if let Some(pkg) = body.strip_prefix("@package") {
            let pkg = pkg.trim().to_string();
            self.hydra_package = Some(pkg.clone());
            let yaml = YamlFields {
                config_system: Some("hydra".to_string()),
                is_hydra_config: true,
                hydra_package: Some(pkg),
                attachment_confidence: Some("high".to_string()),
                ..Default::default()
            };
            self.push_record(
                row,
                line,
                EvidenceKind::Comment,
                AttachedKind::None,
                yaml,
                None,
            );
            return;
        }

        // Commented-out example/option under an active key.
        let looks_like_yaml = body.starts_with('-') || find_key_colon(body).is_some();
        if looks_like_yaml && !self.stack.is_empty() && indent > 0 {
            let parent = self.current_path();
            let yaml = YamlFields {
                config_system: self.config_system(),
                key_path: Some(parent),
                is_hydra_config: self.is_hydra,
                is_commented_example: true,
                hydra_package: self.hydra_package.clone(),
                attachment_confidence: Some("medium".to_string()),
                ..Default::default()
            };
            self.push_record(
                row,
                line,
                EvidenceKind::Comment,
                AttachedKind::Local,
                yaml,
                None,
            );
            return;
        }

        // Ordinary leading comment: attach to the next key.
        self.pending_leading.push(body.to_string());
    }

    fn handle_key(
        &mut self,
        row: usize,
        line: &str,
        indent: usize,
        key: &str,
        value: &str,
        inline_comment: Option<String>,
    ) {
        self.pop_to(indent);
        self.stack.push((indent, key.to_string()));
        let key_path = self.current_path();

        if value.is_empty() {
            // Mapping / list parent: remember for list indexing, attach leading.
            self.list_path = Some(key_path.clone());
            self.list_indent = indent;
            self.list_counter = 0;
            // A leading block above a mapping attaches to the mapping root only
            // when it carries signal; for the MVP we keep it on value facts.
            self.pending_leading.clear();
            return;
        }

        self.list_path = None;
        let leading = self.take_leading();
        let (json_value, value_type) = parse_scalar(value);
        let interpolations = extract_interpolations(value);
        let allowed = inline_comment
            .as_deref()
            .map(allowed_values_hint)
            .unwrap_or_default();

        let has_comment = leading.is_some() || inline_comment.is_some();
        if !self.include_values && !has_comment && allowed.is_empty() {
            return;
        }

        let kind = if has_comment {
            EvidenceKind::YamlValueWithComment
        } else {
            EvidenceKind::YamlValue
        };

        let yaml = YamlFields {
            config_system: self.config_system(),
            key_path: Some(key_path),
            key: Some(key.to_string()),
            value: Some(json_value),
            value_type: Some(value_type.to_string()),
            leading_comment: leading,
            inline_comment,
            allowed_values_hint: allowed,
            interpolations,
            is_hydra_config: self.is_hydra,
            hydra_package: self.hydra_package.clone(),
            is_commented_example: false,
            attachment_confidence: Some("high".to_string()),
        };
        self.push_record(row, line, kind, AttachedKind::YamlFact, yaml, None);
    }

    fn handle_list_item(
        &mut self,
        row: usize,
        line: &str,
        indent: usize,
        item: &str,
        inline_comment: Option<String>,
    ) {
        let Some(parent) = self.list_path.clone() else {
            return;
        };
        if indent < self.list_indent {
            self.list_path = None;
            return;
        }
        let index = self.list_counter;
        self.list_counter += 1;
        let key_path = format!("{parent}[{index}]");

        let leading = self.take_leading();
        let (json_value, value_type) = parse_scalar(item);
        let interpolations = extract_interpolations(item);

        if !self.include_values && leading.is_none() && inline_comment.is_none() {
            // Hydra defaults entries are always meaningful even without comments.
            if !(self.is_hydra && parent == "defaults") {
                return;
            }
        }

        let kind = if leading.is_some() || inline_comment.is_some() {
            EvidenceKind::YamlValueWithComment
        } else {
            EvidenceKind::YamlValue
        };

        let yaml = YamlFields {
            config_system: self.config_system(),
            key_path: Some(key_path),
            key: None,
            value: Some(json_value),
            value_type: Some(value_type.to_string()),
            leading_comment: leading,
            inline_comment,
            allowed_values_hint: Vec::new(),
            interpolations,
            is_hydra_config: self.is_hydra,
            hydra_package: self.hydra_package.clone(),
            is_commented_example: false,
            attachment_confidence: Some("high".to_string()),
        };
        self.push_record(row, line, kind, AttachedKind::YamlFact, yaml, None);
    }

    fn push_record(
        &mut self,
        row: usize,
        line: &str,
        kind: EvidenceKind,
        attached: AttachedKind,
        yaml: YamlFields,
        _extra: Option<()>,
    ) {
        let byte_start = self.line_offset;
        let byte_end = self.line_offset + line.len();
        let mut record = collector::base_record(Base {
            repo: self.repo,
            path: self.path,
            language: "yaml",
            kind,
            comment_style: "yaml".to_string(),
            byte_start,
            byte_end,
            line_start: row + 1,
            line_end: row + 1,
            raw_text: line.to_string(),
        });
        record.confidence = yaml.attachment_confidence.clone();
        record.yaml = Some(yaml);
        collector::apply_score(&mut record, attached, self.file_origin, false);
        self.records.push(record);
    }

    fn pop_to(&mut self, indent: usize) {
        while let Some((i, _)) = self.stack.last() {
            if *i >= indent {
                self.stack.pop();
            } else {
                break;
            }
        }
    }

    fn current_path(&self) -> String {
        self.stack
            .iter()
            .map(|(_, k)| k.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }

    fn take_leading(&mut self) -> Option<String> {
        if self.pending_leading.is_empty() {
            None
        } else {
            let joined = self.pending_leading.join(" ");
            self.pending_leading.clear();
            Some(joined)
        }
    }

    fn config_system(&self) -> Option<String> {
        Some(if self.is_hydra { "hydra" } else { "yaml" }.to_string())
    }
}

/// Find the position of a `key:` colon, ignoring `:` inside quotes / `${}`.
fn find_key_colon(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut brace_depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'{' if !in_single && !in_double => brace_depth += 1,
            b'}' if !in_single && !in_double => brace_depth -= 1,
            // Require a space or end after ':' to count as a mapping colon.
            b':' if !in_single
                && !in_double
                && brace_depth == 0
                && (i + 1 >= bytes.len() || bytes[i + 1] == b' ') =>
            {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Split a line into (code, optional inline comment) respecting quotes.
fn split_inline_comment(line: &str) -> (String, Option<String>) {
    let bytes = line.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            // Must be preceded by whitespace or start-of-line.
            b'#' if !in_single
                && !in_double
                && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') =>
            {
                let code = line[..i].to_string();
                let comment = line[i + 1..].trim().to_string();
                return (
                    code,
                    if comment.is_empty() {
                        None
                    } else {
                        Some(comment)
                    },
                );
            }
            _ => {}
        }
    }
    (line.to_string(), None)
}

/// Parse `# a | b | c` style allowed-values hints.
fn allowed_values_hint(comment: &str) -> Vec<String> {
    if !comment.contains('|') {
        return Vec::new();
    }
    comment
        .split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract `${...}` interpolation inner expressions.
fn extract_interpolations(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            out.push(after[..end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// Parse a YAML scalar into a typed JSON value.
fn parse_scalar(raw: &str) -> (Value, &'static str) {
    let v = raw.trim();
    if v.is_empty() || v == "null" || v == "~" {
        return (Value::Null, "null");
    }
    if v == "true" || v == "false" {
        return (Value::Bool(v == "true"), "bool");
    }
    if let Ok(i) = v.parse::<i64>() {
        return (Value::from(i), "int");
    }
    if let Ok(f) = v.parse::<f64>() {
        return (Value::from(f), "float");
    }
    let unquoted = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(v);
    (Value::String(unquoted.to_string()), "str")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Vec<InventoryRecord> {
        collect("repo", "conf/config.yaml", src, FileOrigin::Local, true).unwrap()
    }

    #[test]
    fn nested_key_path_is_built() {
        let src = "streams:\n  reference:\n    nominal_srate_hz: 500\n";
        let recs = run(src);
        let kp: Vec<_> = recs
            .iter()
            .filter_map(|r| r.yaml.as_ref().and_then(|y| y.key_path.clone()))
            .collect();
        assert!(kp.contains(&"streams.reference.nominal_srate_hz".to_string()));
    }

    #[test]
    fn inline_allowed_values_parsed() {
        let src = "mode: active_send  # modbus_rtu | active_send\n";
        let recs = run(src);
        let r = &recs[0];
        let y = r.yaml.as_ref().unwrap();
        assert_eq!(y.value, Some(Value::String("active_send".into())));
        assert_eq!(y.allowed_values_hint, vec!["modbus_rtu", "active_send"]);
    }

    #[test]
    fn interpolation_extracted() {
        let src = "logging:\n  file: ${hydra:run.dir}/run.log\n";
        let recs = run(src);
        let y = recs
            .iter()
            .find_map(|r| r.yaml.as_ref().filter(|y| y.key == Some("file".into())))
            .unwrap();
        assert_eq!(y.interpolations, vec!["hydra:run.dir"]);
    }

    #[test]
    fn hydra_defaults_and_package() {
        let src = "# @package bridge\n\ndefaults:\n  - logging: default\n  - _self_\n";
        let recs = run(src);
        assert!(recs.iter().any(
            |r| r.yaml.as_ref().and_then(|y| y.hydra_package.clone()) == Some("bridge".into())
        ));
        let paths: Vec<_> = recs
            .iter()
            .filter_map(|r| r.yaml.as_ref().and_then(|y| y.key_path.clone()))
            .collect();
        assert!(paths.contains(&"defaults[0]".to_string()));
        assert!(paths.contains(&"defaults[1]".to_string()));
    }

    #[test]
    fn commented_example_attached() {
        let src = "excluded_ports:\n  # - /dev/ttyUSB0\n";
        let recs = run(src);
        assert!(recs.iter().any(|r| r.yaml.as_ref().is_some_and(
            |y| y.is_commented_example && y.key_path == Some("excluded_ports".into())
        )));
    }

    #[test]
    fn leading_comment_attaches_to_key() {
        let src = "# batch_end_anchored prevents synthetic clock drift.\ntimestamp_policy: batch_end_anchored\n";
        let recs = run(src);
        let y = recs[0].yaml.as_ref().unwrap();
        assert!(
            y.leading_comment
                .as_deref()
                .unwrap()
                .contains("batch_end_anchored")
        );
    }
}
