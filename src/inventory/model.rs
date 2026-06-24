//! Data model for inventory mode records.
//!
//! These structs define the stable JSONL schema documented in
//! `docs/output-contract.md` (`schema-version` = `comment-inventory.v1`).

use serde::Serialize;

pub const SCHEMA_VERSION: &str = "comment-inventory.v1";

/// The kind of evidence a record represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Comment,
    Docstring,
    DocumentationComment,
    YamlValue,
    YamlValueWithComment,
    PreprocessorDiagnostic,
}

impl EvidenceKind {
    /// Short token used as the `id` prefix.
    #[must_use]
    pub const fn id_prefix(self) -> &'static str {
        match self {
            EvidenceKind::Comment => "comment",
            EvidenceKind::Docstring => "docstring",
            EvidenceKind::DocumentationComment => "doc",
            EvidenceKind::YamlValue | EvidenceKind::YamlValueWithComment => "yaml",
            EvidenceKind::PreprocessorDiagnostic => "preprocessor",
        }
    }
}

/// Review priority derived from the deterministic score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
    #[default]
    Ignore,
}

impl Priority {
    /// Map a numeric score to a priority bucket using the MVP thresholds.
    #[must_use]
    pub const fn from_score(score: i64) -> Self {
        if score >= 10 {
            Priority::High
        } else if score >= 6 {
            Priority::Medium
        } else if score >= 3 {
            Priority::Low
        } else {
            Priority::Ignore
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
            Priority::Ignore => "ignore",
        }
    }

    /// Ordering rank used for `--min-priority` filtering (higher is stronger).
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Priority::Ignore => 0,
            Priority::Low => 1,
            Priority::Medium => 2,
            Priority::High => 3,
        }
    }
}

/// Attachment / extraction confidence for heuristic enrichment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

/// A single Doxygen-style tag extracted from a comment or docstring.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoxygenTag {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub text: String,
}

/// Python-specific enrichment payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct PythonFields {
    pub is_docstring: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring_scope: Option<String>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub docstring_sections: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub doxygen_tags: Vec<DoxygenTag>,
    pub is_test_file: bool,
}

/// C/C++-specific enrichment payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct CFamilyFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_declaration_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_declaration_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_declaration_text: Option<String>,
    pub inside_user_code_region: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code_region: Option<String>,
    pub is_inline_field_comment: bool,
    pub is_preprocessor_diagnostic: bool,
}

/// YAML/Hydra-specific enrichment payload.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct YamlFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leading_comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_comment: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allowed_values_hint: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interpolations: Vec<String>,
    pub is_hydra_config: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hydra_package: Option<String>,
    pub is_commented_example: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_confidence: Option<String>,
}

/// One inventory evidence record.
///
/// Serializes to a single JSON line in `comments_docstrings.jsonl`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InventoryRecord {
    pub schema_version: &'static str,
    pub id: String,
    pub repo: String,
    pub path: String,
    pub language: String,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub kind: EvidenceKind,
    pub comment_style: String,
    pub raw_text: String,
    pub normalized_text: String,
    pub source_hash: String,
    pub markers: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub domain_terms: Vec<String>,
    pub claim_types: Vec<String>,
    pub score: i64,
    pub priority: Priority,
    pub score_reasons: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearby_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearest_previous_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nearest_following_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_origin: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contradiction_targets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<PythonFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c_family: Option<CFamilyFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaml: Option<YamlFields>,
}

impl InventoryRecord {
    /// Deterministic sort key: path, then position, then kind prefix.
    #[must_use]
    pub fn sort_key(&self) -> (String, usize, usize, String, String) {
        (
            self.path.clone(),
            self.line_start,
            self.line_end,
            self.kind.id_prefix().to_string(),
            self.id.clone(),
        )
    }
}
