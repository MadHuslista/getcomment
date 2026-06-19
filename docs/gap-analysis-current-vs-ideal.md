# Gap Analysis: Current `uncomment` vs Ideal Comment Inventory Tool

This document identifies the gap between the current implementation and the ideal comment inventory tool.

---

## 1. Executive summary

`uncomment` is a strong parser-based comment **removal** engine. The ideal target is a parser-based comment/docstring/config **inventory** engine.

The current code already contains useful primitives:

```text
file discovery
language detection
tree-sitter parsing
comment node detection
Python docstring detection
preservation rules
parallel processing
TOML config
```

The missing work is mostly product surface, data modeling, context enrichment, and output generation:

```text
inventory CLI
all-comment exposure
structured record schema
symbol/key-path attachment
classification
scoring
JSONL/Markdown writers
YAML value facts
C/C++ firmware-specific enrichment
```

---

## 2. Gap matrix

| Capability                      | Current implementation                  | Ideal target                                | Gap severity          |
| ------------------------------- | --------------------------------------- | ------------------------------------------- | --------------------- |
| Comment detection               | Tree-sitter comment/docstring detection | Same, plus inventory access                 | Low                   |
| Removal                         | In-place removal/dry-run/diff           | Keep unchanged                              | None                  |
| Inventory mode                  | Missing                                 | `uncomment inventory`                       | Critical              |
| Structured JSONL output         | Missing                                 | Stable JSONL schema                         | Critical              |
| Markdown retrieval output       | Missing                                 | Docmancer/LLM-friendly Markdown             | Critical              |
| Summary output                  | Basic removal summary only              | Priority/domain/file/symbol summary         | High                  |
| Expose all comments             | Internal only                           | Public inventory records                    | High                  |
| Raw comment text                | Available via byte slice                | Exported normalized/raw text                | Medium                |
| File path/language metadata     | Available at processor level            | Included per record                         | Medium                |
| Symbol attachment               | Missing                                 | Python/C/C++ symbol context                 | High                  |
| YAML key paths                  | Missing                                 | Full key-path facts/comments                | Critical for YAML MVP |
| YAML active values              | Missing                                 | Extract config facts                        | Critical for YAML MVP |
| Claim taxonomy                  | Missing                                 | Contract/rationale/lifecycle/risk/test/etc. | High                  |
| Scoring                         | Missing                                 | Deterministic priority scoring              | High                  |
| Domain tagging                  | Missing                                 | User/repo domain dictionaries               | Medium/High           |
| Generated/vendor classification | Partial through file patterns only      | File-origin class + score adjustment        | High for firmware     |
| USER CODE metadata              | Missing                                 | Region metadata and priority boost          | Medium/High           |
| Doxygen target attachment       | Missing                                 | Macro/function/type/field attachment        | High for firmware     |
| Preprocessor diagnostics        | Missing                                 | `#warning`/`#error` evidence                | Medium                |
| Config compatibility            | Existing TOML                           | Add inventory settings                      | Low                   |
| Tests                           | Removal-focused                         | Inventory fixtures + schema tests           | High                  |

---

## 3. Current architecture mismatch

### 3.1 Current pipeline mutates source

The current pipeline computes `processed_content` by removing byte ranges from `original_content`. This is not useful for inventory except as a source of existing parser mechanics.

### 3.2 Current comment selection is removal-oriented

`get_comments_to_remove()` filters comments by preservation. Inventory mode needs all comments plus classification.

Required change:

```rust
impl CommentVisitor<'_> {
    pub fn comments(&self) -> &[CommentInfo] { ... }
}
```

### 3.3 Current output model is file-centric, not evidence-centric

`ProcessedFile` represents one processed source file. Inventory mode needs records independent of modified source content.

Suggested model:

```rust
pub struct InventoryRecord {
    pub id: String,
    pub path: PathBuf,
    pub language: String,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub kind: EvidenceKind,
    pub raw_text: String,
    pub normalized_text: String,
    pub scope: Option<ScopeInfo>,
    pub yaml: Option<YamlInfo>,
    pub c_family: Option<CFamilyInfo>,
    pub markers: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub domain_terms: Vec<String>,
    pub claim_types: Vec<String>,
    pub score: i32,
    pub priority: Priority,
    pub score_reasons: Vec<String>,
    pub source_hash: String,
}
```

---

## 4. Ideal-only features not required for MVP

The ideal tool includes capabilities that should be postponed:

| Feature                         | Reason to postpone                                       |
| ------------------------------- | -------------------------------------------------------- |
| Full contradiction detection    | Better handled by downstream retrieval/agent layer first |
| Embeddings                      | Not necessary for first local inventory                  |
| Full Hydra defaults composition | Useful later; MVP can extract defaults entries as facts  |
| Perfect C++ symbol resolution   | High complexity; heuristics are enough initially         |
| Full Doxygen semantic parser    | Text + nearest declaration is enough for MVP             |
| Cross-repo/domain learning      | Use configurable dictionaries first                      |
| Automatic documentation rewrite | Separate product workflow                                |

---

## 5. Required architecture additions

### 5.1 CLI

Add:

```rust
Commands::Inventory { ... }
```

Recommended options:

```text
--output-dir
--format
--languages
--include-docstrings / --no-docstrings
--include-yaml-values / --no-yaml-values
--include-low-priority
--include-generated
--min-priority
--domain-config
```

### 5.2 Inventory extraction module

Add a new module independent of `Processor` removal logic:

```text
src/inventory/
  mod.rs
  model.rs
  extractor.rs
  classify.rs
  score.rs
  source_class.rs
  writers/
    jsonl.rs
    markdown.rs
    summary.rs
  languages/
    python.rs
    c_family.rs
    yaml.rs
```

### 5.3 Output writers

Add output writers that do not touch source files.

### 5.4 Config model

Extend config with inventory settings:

```toml
[inventory]
output_dir = ".agent/inventory"
formats = ["jsonl", "markdown", "summary"]
min_priority = "low"
include_docstrings = true
include_yaml_values = true
include_generated_metadata = false

[inventory.domains.embedded]
terms = ["NPU", "DMA", "cache", "RAW10"]
```

---

## 6. Major design decision

Prefer a new `inventory` subcommand over a flag on the existing removal command.

Reason:

```text
uncomment <paths>             -> removal semantics
uncomment inventory <paths>   -> evidence extraction semantics
```

This reduces accidental source mutation and makes help text clearer.

---

## 7. Risk assessment

| Risk                                                    | Mitigation                                                            |
| ------------------------------------------------------- | --------------------------------------------------------------------- |
| Inventory scope expands into full documentation auditor | Keep contradiction detection out of MVP                               |
| Generated/vendor noise overwhelms output                | Add source-class classifier before scoring                            |
| YAML comments cannot be attached perfectly              | Start with deterministic line/key-path heuristics and mark confidence |
| C/C++ symbol context is imperfect                       | Use nearest-declaration heuristics and expose confidence              |
| Existing removal behavior regresses                     | Keep inventory in separate code path; add regression tests            |
| JSON schema churn                                       | Version schema from day one                                           |

---

## 8. Bottom line

`uncomment` has the right parser foundation but the wrong product direction for the requested workflow. It should be extended, not rewritten.

The critical gap is not comment detection. The critical gap is turning comments/docstrings/configs into structured, prioritized, context-rich evidence artifacts.
