//! Deterministic scoring and claim-type classification.
//!
//! Tables are transcribed from docs/gap-analysis-current-vs-mvp.md §7 and
//! docs/language-extraction-rules.md §3.9. Scoring is pure and local so output
//! is reproducible (FR-012) and scales linearly (NFR-003).

use crate::inventory::model::{EvidenceKind, InventoryRecord, Priority};
use crate::inventory::source_class::FileOrigin;

/// What declaration / structure a comment is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachedKind {
    None,
    /// Inline or local comment with no symbol attachment.
    Local,
    Function,
    Class,
    Macro,
    Constant,
    /// An active YAML key/value fact.
    YamlFact,
}

/// Extra context the record itself does not carry.
#[derive(Debug, Clone, Copy)]
pub struct ScoreContext {
    pub attached_kind: AttachedKind,
    pub file_origin: FileOrigin,
    pub is_section_separator: bool,
}

/// Result of scoring: numeric score, priority bucket, claims, and reasons.
#[derive(Debug, Clone, Default)]
pub struct Scored {
    pub score: i64,
    pub priority: Priority,
    pub claim_types: Vec<String>,
    pub score_reasons: Vec<String>,
}

const MARKER_HIGH: &[&str] = &["TODO", "FIXME", "BUG", "HACK", "WARNING", "WORKAROUND"];
const MARKER_MED: &[&str] = &["NOTE", "IMPORTANT", "CAUTION", "REVIEW"];
const MARKER_LIFECYCLE: &[&str] = &["DEPRECATED", "LEGACY"];

/// Compute score, priority, claim types, and reasons for a record.
#[must_use]
pub fn classify_and_score(record: &InventoryRecord, ctx: &ScoreContext) -> Scored {
    let mut score: i64 = 0;
    let mut reasons: Vec<String> = Vec::new();
    let mut claims: Vec<String> = Vec::new();
    let text = record.normalized_text.to_lowercase();

    // --- Base attachment ---------------------------------------------------
    match ctx.attached_kind {
        AttachedKind::Function | AttachedKind::Class if record.kind == EvidenceKind::Docstring => {
            score += 3;
            reasons.push("docstring attached to public symbol".into());
        }
        AttachedKind::Function
        | AttachedKind::Class
        | AttachedKind::Macro
        | AttachedKind::Constant => {
            score += 2;
            reasons.push("comment attached to function/class/macro".into());
        }
        AttachedKind::YamlFact => {
            score += 2;
            reasons.push("YAML active config fact".into());
            claims.push("configuration_fact".into());
        }
        AttachedKind::Local => {
            score += 1;
            reasons.push("inline/local comment".into());
        }
        AttachedKind::None => {}
    }

    // --- Markers -----------------------------------------------------------
    for marker in &record.markers {
        let m = marker.as_str();
        if MARKER_HIGH.contains(&m) {
            score += 5;
            reasons.push(format!("marker: {m}"));
            claims.push("risk_workaround_marker".into());
        } else if MARKER_LIFECYCLE.contains(&m) {
            score += 5;
            reasons.push(format!("marker: {m}"));
            claims.push("lifecycle_status".into());
        } else if MARKER_MED.contains(&m) {
            score += 3;
            reasons.push(format!("marker: {m}"));
        }
    }

    // --- Semantic ----------------------------------------------------------
    if any_of(&text, &["must", "should", "required", "guarantee"]) {
        score += 4;
        reasons.push("contract language (must/should/required)".into());
        claims.push("contract_claim".into());
    }
    if any_of(&text, &["format", "schema", "contract", "protocol"]) {
        score += 4;
        reasons.push("schema/protocol language".into());
        claims.push("protocol_schema_contract".into());
    }
    if any_of(&text, &["default", "optional", "not used"]) {
        score += 4;
        reasons.push("default/optional/not-used language".into());
    }
    if any_of(&text, &["unit", "rate", "range", "path", "field"]) {
        score += 3;
        reasons.push("units/rate/range/path/field".into());
    }
    if !record.domain_terms.is_empty() {
        score += 2;
        reasons.push(format!("domain term: {}", record.domain_terms.join(", ")));
    }
    if !record.requirement_ids.is_empty() {
        score += 4;
        reasons.push(format!(
            "requirement id: {}",
            record.requirement_ids.join(", ")
        ));
        claims.push("requirement_traceability".into());
    }

    // --- C/C++ additions ---------------------------------------------------
    if let Some(c) = &record.c_family {
        if c.is_preprocessor_diagnostic {
            score += 5;
            reasons.push("preprocessor #warning/#error".into());
            claims.push("build_assumption".into());
            claims.push("version_mismatch_risk".into());
        }
        let doxygen = record.comment_style.starts_with("doxygen") || c.is_inline_field_comment;
        if doxygen
            && matches!(
                c.target_declaration_kind.as_deref(),
                Some("macro") | Some("constant")
            )
        {
            score += 4;
            reasons.push("Doxygen attached to macro/constant".into());
            claims.push("constant_contract".into());
            claims.push("model_parameter".into());
        }
        if c.inside_user_code_region {
            score += 3;
            reasons.push("inside USER CODE region".into());
        }
        if mentions_hw(&text) {
            score += 3;
            reasons.push("firmware domain term".into());
        }
    }

    // --- YAML additions ----------------------------------------------------
    if let Some(y) = &record.yaml {
        if y.is_hydra_config
            && y.key_path
                .as_deref()
                .is_some_and(|k| k.starts_with("defaults["))
        {
            score += 4;
            reasons.push("Hydra defaults entry".into());
        }
        if y.hydra_package.is_some() {
            score += 3;
            reasons.push("Hydra @package marker".into());
        }
        if y.leading_comment.is_some() && ctx.attached_kind == AttachedKind::YamlFact {
            score += 3;
            reasons.push("comment attached to active key".into());
        }
        if !y.allowed_values_hint.is_empty() {
            score += 4;
            reasons.push("inline allowed-values list".into());
            claims.push("allowed_values".into());
        }
        if let Some(kp) = y.key_path.as_deref() {
            let kp_l = kp.to_lowercase();
            if any_of(&kp_l, &["stream", "protocol", "schema", "ipc"]) {
                score += 5;
                reasons.push("stream/protocol/schema/ipc key".into());
                claims.push("protocol_schema_contract".into());
            }
            if any_of(&kp_l, &["timestamp", "alignment"]) {
                score += 5;
                reasons.push("timestamp/alignment key".into());
                claims.push("timing_synchronization_policy".into());
            }
            if any_of(&kp_l, &["quality", "fit", "calibration"]) {
                score += 4;
                reasons.push("quality/fit/calibration key".into());
            }
        }
        if any_of(
            &text,
            &["display-only", "not acquisition truth", "display only"],
        ) {
            score += 5;
            reasons.push("display-only / not acquisition truth".into());
        }
        if any_of(&text, &["authoritative", "source of truth"]) {
            score += 5;
            reasons.push("authoritative / source of truth".into());
            claims.push("source_of_truth".into());
        }
        if any_of(&text, &["legacy", "deprecated", "no effect"]) {
            score += 5;
            reasons.push("legacy / deprecated / no effect".into());
            claims.push("lifecycle_status".into());
        }
        if !y.interpolations.is_empty() {
            claims.push("runtime_interpolation".into());
        }
        if y.is_commented_example {
            score += 2;
            reasons.push("commented-out operator example".into());
            claims.push("operator_guidance".into());
        }
        if ctx.attached_kind == AttachedKind::YamlFact && y.value.is_some() {
            claims.push("runtime_default".into());
        }
    }

    // --- Penalties ---------------------------------------------------------
    if ctx.file_origin.is_generated_or_vendor() {
        score -= 6;
        reasons.push("vendor/generated boilerplate".into());
        claims.push("generated_noise".into());
    }
    if ctx.is_section_separator {
        score -= 4;
        reasons.push("section separator only".into());
    }

    // Python doxygen / docstrings carry contract intent.
    if let Some(p) = &record.python {
        if !p.doxygen_tags.is_empty() {
            claims.push("contract_claim".into());
        }
        if p.is_test_file && p.is_docstring {
            claims.push("test_intent".into());
        }
        if !p.docstring_sections.is_empty() {
            claims.push("implementation_rationale".into());
        }
    }

    // Preprocessor diagnostics are build-critical and always warrant review.
    if record
        .c_family
        .as_ref()
        .is_some_and(|c| c.is_preprocessor_diagnostic)
        && score < 10
    {
        score = 10;
        reasons.push("preprocessor diagnostic elevated to high".into());
    }

    dedup(&mut claims);
    dedup(&mut reasons);

    let priority = Priority::from_score(score);
    Scored {
        score,
        priority,
        claim_types: claims,
        score_reasons: reasons,
    }
}

fn any_of(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn mentions_hw(text: &str) -> bool {
    any_of(
        text,
        &[
            "dma", "cache", "npu", "raw10", "lfsr", "risaf", "xspi", "threadx", "prbs", "bayer",
        ],
    )
}

fn dedup(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert(i.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::model::CFamilyFields;

    fn base() -> InventoryRecord {
        InventoryRecord {
            schema_version: crate::inventory::model::SCHEMA_VERSION,
            id: "x".into(),
            repo: "r".into(),
            path: "p".into(),
            language: "c".into(),
            line_start: 1,
            line_end: 1,
            byte_start: 0,
            byte_end: 1,
            kind: EvidenceKind::Comment,
            comment_style: "line".into(),
            raw_text: String::new(),
            normalized_text: String::new(),
            source_hash: String::new(),
            markers: vec![],
            requirement_ids: vec![],
            domain_terms: vec![],
            claim_types: vec![],
            score: 0,
            priority: Priority::Ignore,
            score_reasons: vec![],
            scope_type: None,
            scope_name: None,
            qualified_symbol: None,
            nearby_signature: None,
            nearest_previous_symbol: None,
            nearest_following_symbol: None,
            file_origin: None,
            contradiction_targets: vec![],
            confidence: None,
            python: None,
            c_family: None,
            yaml: None,
        }
    }

    fn ctx(attached: AttachedKind) -> ScoreContext {
        ScoreContext {
            attached_kind: attached,
            file_origin: FileOrigin::Local,
            is_section_separator: false,
        }
    }

    #[test]
    fn todo_is_high_priority() {
        let mut r = base();
        r.markers = vec!["TODO".into()];
        r.normalized_text = "validate DMA cache policy".into();
        r.domain_terms = vec!["DMA".into(), "cache".into()];
        r.c_family = Some(CFamilyFields {
            inside_user_code_region: true,
            ..Default::default()
        });
        let s = classify_and_score(&r, &ctx(AttachedKind::Local));
        assert!(s.score >= 10, "score was {}", s.score);
        assert_eq!(s.priority, Priority::High);
        assert!(
            s.claim_types
                .contains(&"risk_workaround_marker".to_string())
        );
    }

    #[test]
    fn doxygen_macro_is_model_parameter() {
        let mut r = base();
        r.comment_style = "doxygen_block".into();
        r.normalized_text = "Number of LFSR clocks between rows.".into();
        r.domain_terms = vec!["LFSR".into()];
        r.c_family = Some(CFamilyFields {
            target_declaration_kind: Some("macro".into()),
            target_declaration_name: Some("ROW_STRIDE_BITS".into()),
            ..Default::default()
        });
        let s = classify_and_score(&r, &ctx(AttachedKind::Macro));
        assert!(s.claim_types.contains(&"model_parameter".to_string()));
        assert_eq!(s.priority, Priority::High);
    }

    #[test]
    fn vendor_is_downranked() {
        let mut r = base();
        r.normalized_text = "Copyright STMicroelectronics".into();
        let mut c = ctx(AttachedKind::Local);
        c.file_origin = FileOrigin::VendorHalCmsis;
        let s = classify_and_score(&r, &c);
        assert!(s.score < 3);
        assert_eq!(s.priority, Priority::Ignore);
    }
}
