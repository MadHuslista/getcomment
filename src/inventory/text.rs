//! Pure text helpers shared by inventory enrichers.
//!
//! Per NFR-001 these operate only on already-extracted comment/docstring text:
//! markers, requirement IDs, and domain terms are *classification* signals, not
//! comment detectors.

/// Markers that signal lifecycle / risk / attention.
///
/// Ordered most-specific first; matching is case-sensitive on the upper form so
/// that prose like "todo list" does not trigger a marker.
const MARKERS: &[&str] = &[
    "TODO",
    "FIXME",
    "BUG",
    "HACK",
    "WARNING",
    "WORKAROUND",
    "DEPRECATED",
    "LEGACY",
    "NOTE",
    "IMPORTANT",
    "CAUTION",
    "REVIEW",
    "XXX",
];

/// Firmware / streaming / config domain terms used for priority boosting and
/// retrieval. Drawn from docs/language-extraction-rules.md §2.4 and §3.7.
const DOMAIN_TERMS: &[&str] = &[
    // sensor / imaging
    "RAW10",
    "Bayer",
    "BGGR",
    "PN9",
    "PRBS",
    "LFSR",
    "seed",
    "frame",
    "pixel",
    "stride",
    "width",
    "height", // memory / hardware / rtos
    "DMA",
    "cache",
    "NPU",
    "RISAF",
    "IAC",
    "XSPI",
    "ThreadX",
    "buffer",
    "address",
    "register",
    // streaming / protocol / timing
    "stream",
    "schema",
    "topic",
    "timestamp",
    "alignment",
    "ref_shift",
    "sample_rate",
    "nominal_srate",
    "frequency",
    "protocol",
    "quality",
    "calibration",
    "modbus",
    "ZMQ",
    "IPC",
    "XDF",
    "CSV",
    "latency",
    "drift",
];

/// Extract distinct attention markers present in `text`, in canonical order.
#[must_use]
pub fn markers(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for marker in MARKERS {
        if contains_word(text, marker) && !found.iter().any(|m| m == marker) {
            found.push((*marker).to_string());
        }
    }
    found
}

/// Extract requirement-traceability identifiers (`FR-001`, `US-42`, `Bug #7`).
#[must_use]
pub fn requirement_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let bytes = text.as_bytes();

    // FR-<digits> and US-<digits> / US<digits>
    for prefix in ["FR-", "US-", "US"] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            let after = start + prefix.len();
            let digits: String = text[after..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if !digits.is_empty() {
                let id = format!("{prefix}{digits}");
                // Avoid double-counting "US-1" as both "US-" and "US".
                if !ids
                    .iter()
                    .any(|existing: &String| existing.contains(&digits))
                {
                    ids.push(id);
                }
            }
            search_from = after.max(start + 1);
        }
    }

    // Bug #<digits>
    let mut idx = 0;
    while let Some(rel) = text[idx..].find("Bug #") {
        let start = idx + rel + "Bug #".len();
        let digits: String = bytes[start..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .map(|&b| b as char)
            .collect();
        if !digits.is_empty() {
            ids.push(format!("Bug #{digits}"));
        }
        idx = start.max(idx + 1);
    }

    ids
}

/// Extract distinct domain terms present in `text`, preserving dictionary order.
#[must_use]
pub fn domain_terms(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut found = Vec::new();
    for term in DOMAIN_TERMS {
        if lower.contains(&term.to_lowercase()) && !found.iter().any(|t| t == term) {
            found.push((*term).to_string());
        }
    }
    found
}

/// Strip comment delimiters and collapse interior whitespace to single spaces.
///
/// Handles `//`, `/* */`, `/** */`, `///`, `//!`, `/**< */`, `#`, and leading
/// `*` continuation lines used in block comments.
#[must_use]
pub fn normalize_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        let cleaned = strip_comment_markers(line);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
    }
    // Collapse any remaining runs of whitespace.
    collapse_whitespace(&out)
}

/// Remove leading/trailing comment punctuation from a single line.
#[must_use]
pub fn strip_comment_markers(line: &str) -> String {
    let mut s = line.trim();

    // Block comment terminator.
    if let Some(stripped) = s.strip_suffix("*/") {
        s = stripped.trim_end();
    }

    // Leading openers, most specific first.
    for opener in ["/**<", "/*!", "/**", "/*", "///<", "///", "//!", "//", "#"] {
        if let Some(stripped) = s.strip_prefix(opener) {
            s = stripped;
            break;
        }
    }

    // Block-comment continuation lines beginning with `*`.
    let s = s.trim_start();
    let s = s
        .strip_prefix("* ")
        .or_else(|| s.strip_prefix('*'))
        .unwrap_or(s);

    s.trim().to_string()
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_ws && !out.is_empty() {
                out.push(' ');
            }
            last_ws = true;
        } else {
            out.push(ch);
            last_ws = false;
        }
    }
    out.trim_end().to_string()
}

/// True when `word` appears in `text` bounded by non-alphanumeric characters.
fn contains_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let wbytes = word.as_bytes();
    if wbytes.is_empty() {
        return false;
    }
    let mut i = 0;
    while let Some(rel) = text[i..].find(word) {
        let start = i + rel;
        let end = start + word.len();
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        i = start + 1;
        if i >= text.len() {
            break;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_detects_word_bounded() {
        assert_eq!(markers("TODO: fix"), vec!["TODO"]);
        assert!(markers("mastodon migration").is_empty());
        // Markers are uppercase tokens to avoid prose false positives; lowercase
        // lifecycle words are handled by scoring's semantic pass instead.
        assert_eq!(
            markers("DEPRECATED LEGACY path"),
            vec!["DEPRECATED", "LEGACY"]
        );
    }

    #[test]
    fn requirement_ids_extracts_known_forms() {
        assert_eq!(requirement_ids("see FR-001 here"), vec!["FR-001"]);
        assert!(requirement_ids("relates to Bug #42").contains(&"Bug #42".to_string()));
    }

    #[test]
    fn domain_terms_are_case_insensitive() {
        let terms = domain_terms("validate dma cache policy");
        assert!(terms.contains(&"DMA".to_string()));
        assert!(terms.contains(&"cache".to_string()));
    }

    #[test]
    fn normalize_strips_block_comment() {
        let raw = "/**\n * Number of LFSR clocks.\n */";
        assert_eq!(normalize_text(raw), "Number of LFSR clocks.");
    }

    #[test]
    fn strip_handles_inline_field() {
        assert_eq!(
            strip_comment_markers("/**< Pixels exact. */"),
            "Pixels exact."
        );
        assert_eq!(strip_comment_markers("# leading hash"), "leading hash");
    }
}
