# Output Contract

This document defines the JSONL and Markdown output contracts for inventory mode.

---

## 1. Artifact layout

Default output directory:

```text
.agent/inventory/
```

Default files:

```text
comments_docstrings.jsonl
comments_docstrings.md
comments_docstrings_summary.md
comments_docstrings_by_symbol.json
inventory_manifest.json
```

---

## 2. JSONL schema

Each line of `comments_docstrings.jsonl` is one record.

### 2.1 Required fields

```json
{
  "schema_version": "comment-inventory.v1",
  "id": "comment:src/main.c:120:126:sha256-12",
  "repo": "uncomment",
  "path": "src/main.c",
  "language": "c",
  "line_start": 120,
  "line_end": 126,
  "byte_start": 4096,
  "byte_end": 4301,
  "kind": "documentation_comment",
  "comment_style": "doxygen_block",
  "raw_text": "/** ... */",
  "normalized_text": "Expected RAW10 value for a fully dark test-pattern sample.",
  "source_hash": "sha256:...",
  "markers": [],
  "requirement_ids": [],
  "domain_terms": ["RAW10", "frame"],
  "claim_types": ["constant_contract", "sensor_model_claim"],
  "score": 12,
  "priority": "high",
  "score_reasons": ["Doxygen comment", "attached to macro", "domain term: RAW10"]
}
```

### 2.2 Optional common fields

```json
{
  "scope_type": "function",
  "scope_name": "_frame_validate_noise_pattern",
  "qualified_symbol": "frame_validate::_frame_validate_noise_pattern",
  "nearby_signature": "static bool _frame_validate_noise_pattern(...)",
  "nearest_previous_symbol": "...",
  "nearest_following_symbol": "...",
  "file_origin": "local_validation_model",
  "contradiction_targets": [
    "docs/**/*.md",
    "tests/**/*.py"
  ],
  "confidence": "medium"
}
```

---

## 3. Python-specific fields

```json
{
  "python": {
    "is_docstring": true,
    "docstring_scope": "class",
    "docstring_sections": {
      "Parameters": "...",
      "Returns": "...",
      "Warnings": "..."
    },
    "doxygen_tags": [
      {"tag": "@param", "name": "level", "text": "Logging level string."}
    ],
    "is_test_file": false
  }
}
```

---

## 4. C/C++-specific fields

```json
{
  "c_family": {
    "target_declaration_kind": "macro",
    "target_declaration_name": "FRAME_VALIDATE_PRBS_LFSR_ROW_STRIDE_BITS",
    "target_declaration_text": "#define FRAME_VALIDATE_PRBS_LFSR_ROW_STRIDE_BITS (1560U)",
    "inside_user_code_region": true,
    "user_code_region": "Includes",
    "is_inline_field_comment": false,
    "is_preprocessor_diagnostic": false
  }
}
```

Preprocessor diagnostic example:

```json
{
  "kind": "preprocessor_diagnostic",
  "raw_text": "#warning \"Possible mismatch in ll_aton library used\"",
  "claim_types": ["build_assumption", "version_mismatch_risk"],
  "priority": "high"
}
```

---

## 5. YAML-specific fields

```json
{
  "yaml": {
    "config_system": "hydra",
    "key_path": "streams.reference.nominal_srate_hz",
    "key": "nominal_srate_hz",
    "value": 500,
    "value_type": "int",
    "leading_comment": "Reference stream nominal rate.",
    "inline_comment": "Hz",
    "allowed_values_hint": [],
    "interpolations": [],
    "is_hydra_config": true,
    "hydra_package": "bridge",
    "is_commented_example": false,
    "attachment_confidence": "high"
  }
}
```

Allowed values example:

```json
{
  "yaml": {
    "key_path": "device.mode",
    "key": "mode",
    "value": "active_send",
    "inline_comment": "modbus_rtu | active_send",
    "allowed_values_hint": ["modbus_rtu", "active_send"]
  },
  "claim_types": ["runtime_default", "allowed_values"]
}
```

---

## 6. Record ID rules

IDs must be stable across runs unless the record changes.

Recommended format:

```text
<kind>:<path>:<line_start>:<line_end>:<short_hash>
```

Examples:

```text
comment:src/frame_validate.c:120:126:a4c3e911
docstring:src/pkg/filter.py:42:51:b913f222
yaml:conf/config.yaml:streams.reference.nominal_srate_hz:3dd91f0a
preprocessor:src/network.c:88:88:af100fe2
```

Hash input:

```text
path + language + kind + line_start + normalized_text
```

---

## 7. Markdown output format

`comments_docstrings.md` should be optimized for retrieval.

Example:

```markdown
# Comments and Docstrings Inventory

## src/frame_validate.c:120-126 — FRAME_VALIDATE_PRBS_LFSR_ROW_STRIDE_BITS

Type: documentation_comment
Priority: high
Score: 12
Claim types: constant_contract, sensor_model_claim
Domain terms: RAW10, frame, LFSR
Language: c
File origin: local_validation_model
Symbol: `FRAME_VALIDATE_PRBS_LFSR_ROW_STRIDE_BITS`

Extracted text:

> Number of LFSR clocks between consecutive image rows.

Score reasons:

- Doxygen comment
- Attached to macro
- Domain term: LFSR

Potential contradiction targets:

- `docs/**/*.md`
- `tests/**/*`
```

---

## 8. Summary output format

`comments_docstrings_summary.md` should include:

```text
total records
records by language
records by priority
records by claim type
records by marker
records by file-origin class
top high-priority files
top high-priority symbols/key paths
generated/vendor records suppressed/down-ranked
warnings/errors during parsing
```

---

## 9. Manifest

`inventory_manifest.json` should include:

```json
{
  "schema_version": "comment-inventory.v1",
  "tool": "uncomment",
  "tool_version": "3.0.3+inventory",
  "generated_at": "2026-06-18T00:00:00Z",
  "root": ".",
  "paths": ["."],
  "formats": ["jsonl", "markdown", "summary"],
  "languages_requested": ["python", "c", "cpp", "yaml"],
  "records": 1234,
  "files_scanned": 200,
  "files_with_records": 80,
  "errors": []
}
```
