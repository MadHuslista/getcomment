# Implementation Roadmap

This document translates the PRD into an implementation sequence suitable for incremental PRs.

---

## 1. Guiding principle

Do not disturb the existing removal pipeline. Add inventory mode as a separate path that reuses parser, registry, config, and file traversal primitives.

---

## 2. Proposed module layout

```text
src/inventory/
  mod.rs
  model.rs
  extractor.rs
  classify.rs
  score.rs
  source_class.rs
  manifest.rs
  writers/
    mod.rs
    jsonl.rs
    markdown.rs
    summary.rs
    by_symbol.rs
  languages/
    mod.rs
    python.rs
    c_family.rs
    yaml.rs
```

---

## 3. PR sequence

### PR 1 — CLI and model skeleton

Scope:

- Add `inventory` subcommand.
- Add `InventoryArgs`.
- Add `InventoryRecord` model.
- Add `InventoryOptions`.
- Add manifest writer.
- Add empty artifact generation.

Acceptance:

```bash
uncomment inventory fixtures/languages/test.py --output-dir /tmp/inventory
```

creates:

```text
/tmp/inventory/inventory_manifest.json
```

without modifying source files.

---

### PR 2 — Generic comment extraction

Scope:

- Expose all comments from `CommentVisitor`.
- Add generic inventory extractor using existing tree-sitter parser and language registry.
- Emit JSONL with generic records.

Acceptance:

- JSONL records include path, language, line ranges, byte ranges, node type, raw text.
- Existing removal tests still pass.

---

### PR 3 — Markdown and summary writers

Scope:

- Add Markdown writer.
- Add summary writer.
- Add deterministic ordering.
- Add schema version field.

Acceptance:

- JSONL is valid.
- Markdown contains one section per high/medium record.
- Summary contains counts by language and priority.

---

### PR 4 — Scoring and classification core

Scope:

- Marker extraction.
- Requirement ID extraction.
- Domain term extraction.
- Claim type classification.
- Score and priority calculation.

Acceptance:

- TODO/FIXME/WARNING/DEPRECATED comments become high priority.
- License/header comments are down-ranked.
- Score reasons are emitted.

---

### PR 5 — Python enrichment

Scope:

- Emit Python docstrings as `docstring` records.
- Attach simple scope names.
- Parse common docstring sections.
- Recognize Python Doxygen-like comments.
- Promote test docstrings.

Acceptance:

- Fixtures distinguish docstrings from non-docstring strings.
- `# @param` comments become `python_doxygen_comment` records.

---

### PR 6 — C/C++ enrichment

Scope:

- Doxygen style detection.
- Nearest declaration heuristics.
- Macro/constant/function/type/field target attachment.
- USER CODE region detection.
- Source-origin classification.
- `#warning`/`#error` diagnostic extraction.

Acceptance:

- Macro Doxygen comments are high priority.
- USER CODE TODOs include region metadata.
- Generated/vendor noise is down-ranked.

---

### PR 7 — YAML/Hydra enrichment

Scope:

- YAML key path extraction.
- Active value fact records.
- Leading/inline comment attachment.
- Allowed-values hint parsing.
- Hydra `defaults` and `# @package` extraction.
- Interpolation extraction.
- Commented-out example extraction.

Acceptance:

- `streams.reference.nominal_srate_hz` value facts are emitted.
- `mode: active_send  # modbus_rtu | active_send` emits allowed values.
- `${hydra:run.dir}` interpolation is captured.

---

### PR 8 — Configuration and documentation

Scope:

- Add `[inventory]` config section.
- Add README documentation.
- Add examples.
- Add migration notes.

Acceptance:

- Existing configs remain valid.
- Inventory config is optional.

---

## 4. Suggested CLI design

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    Init { ... },
    Inventory {
        #[arg(help = "Files, directories, or glob patterns to inventory")]
        paths: Vec<String>,

        #[arg(long, default_value = ".agent/inventory")]
        output_dir: PathBuf,

        #[arg(long, value_delimiter = ',', default_value = "jsonl,markdown,summary")]
        format: Vec<InventoryFormat>,

        #[arg(long, value_delimiter = ',')]
        languages: Vec<String>,

        #[arg(long, default_value_t = true)]
        include_docstrings: bool,

        #[arg(long, default_value_t = true)]
        include_yaml_values: bool,

        #[arg(long, default_value_t = false)]
        include_generated: bool,

        #[arg(long, default_value = "low")]
        min_priority: Priority,
    },
}
```

---

## 5. Internal API sketch

```rust
pub fn inventory_paths(
    paths: &[String],
    options: &InventoryOptions,
    config_manager: &ConfigManager,
) -> anyhow::Result<InventoryResult>;

pub struct InventoryResult {
    pub records: Vec<InventoryRecord>,
    pub files_scanned: usize,
    pub files_with_records: usize,
    pub errors: Vec<InventoryError>,
}
```

---

## 6. Testing strategy

### 6.1 Unit tests

```text
record ID stability
marker extraction
requirement ID extraction
domain term extraction
priority scoring
YAML key path stack
allowed values parsing
interpolation parsing
USER CODE region tracking
source-origin classification
```

### 6.2 Integration tests

```text
uncomment inventory fixtures/inventory/python
uncomment inventory fixtures/inventory/c
uncomment inventory fixtures/inventory/yaml
uncomment inventory fixtures/inventory/mixed
```

Assertions:

```text
source files unchanged
JSONL valid
expected record count
expected high-priority IDs
expected Markdown headings
summary totals match JSONL
```

### 6.3 Regression tests

Run existing removal tests unchanged.

---

## 7. Implementation guardrails

- Keep inventory and removal output paths separate.
- Do not add LLM dependencies.
- Do not require network access.
- Do not parse comments with regex as the primary mechanism.
- Use regex only after parser extraction for marker/classification logic.
- Keep schema versioned.
- Include confidence when context attachment is heuristic.

---

## 8. Recommended first implementation cut

The fastest useful cut is:

```text
inventory subcommand
all-comment JSONL export
Python docstring records
C/C++ generic records
YAML key/value fact records
basic scoring
Markdown summary
```

Do C/C++ Doxygen target attachment and YAML comment attachment in the next cut if needed.
