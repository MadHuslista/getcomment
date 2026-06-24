# PRD: Comment Inventory Mode for `uncomment`

Status: Draft for implementation planning
Target repository: `goldziher/uncomment`
Generated from: uploaded `uncomment-main(2).zip` and `comment-rules_repo-infer(2).md`
Primary goal: evolve `uncomment` from a comment **removal** tool into a reusable comment/docstring/config-claim **inventory** tool without breaking its current removal workflow.

---

## 1. Problem statement

`uncomment` currently solves a narrow and valuable problem: it removes comments from source files using tree-sitter so comment-like text inside strings is not incorrectly modified. That behavior is useful for code cleanup and repo packing.

The missing product capability is the inverse workflow:

> Given a source tree, emit a structured, non-destructive inventory of comments, docstrings, Doxygen blocks, and YAML/Hydra configuration comments/facts so that humans and LLM agents can audit claims, compare them against documentation, and detect drift.

The desired tool is not merely “comments only text.” It must produce an evidence layer that links each comment/docstring/config claim to file path, line range, language, scope/symbol, semantic role, priority, and likely contradiction targets.

---

## 2. Product objectives

### 2.1 Ideal product objective

Build a claim-oriented comment inventory tool that can answer:

1. What comments, docstrings, and YAML/Hydra config values contain meaningful claims?
2. Which file, symbol, macro, key path, or generated/vendor region does each claim belong to?
3. Is the claim behavioral, configurational, lifecycle-related, risk-related, test-intent, traceability-related, or generated noise?
4. Which Markdown docs, tests, source symbols, and config files should be checked for contradiction or duplication?
5. Which claims should be prioritized for human review or LLM retrieval?

### 2.2 MVP objective

Add a first implementation slice that supports:

- Python: `.py`, `.pyw`, `.pyi`
- C/C++: `.c`, `.h`, `.cpp`, `.hpp`, `.cc`, `.cxx`, `.hh`, `.hxx`
- YAML/Hydra config: `.yml`, `.yaml`

The MVP should emit structured artifacts:

```text
.agent/inventory/comments_docstrings.jsonl
.agent/inventory/comments_docstrings.md
.agent/inventory/comments_docstrings_summary.md
.agent/inventory/comments_docstrings_by_symbol.json
```

The MVP does **not** need to perform automated contradiction detection. It must produce a high-quality evidence inventory that downstream agents or retrieval tools can use.

---

## 3. Users and use cases

### 3.1 Primary user

A technical lead or AI-assisted engineer auditing mixed repositories with Python packages, embedded C/C++ firmware, generated/vendor code, tests, and Hydra/YAML configs.

### 3.2 Primary workflows

#### Workflow A — repo documentation drift audit

```bash
uncomment inventory . \
  --format jsonl,markdown,summary \
  --output-dir .agent/inventory
```

Then ingest the Markdown inventory into retrieval tooling and ask:

```text
Find comments/docstrings that contradict current Markdown documentation about timestamping, calibration outputs, stream contracts, or firmware frame validation behavior.
```

#### Workflow B — firmware comment triage

```bash
uncomment inventory firmware/ \
  --languages c,cpp \
  --exclude 'Drivers/CMSIS/**' \
  --exclude 'Middlewares/**' \
  --include-generated-metadata
```

Expected result: local comments, Doxygen constants, USER CODE region comments, TODO/FIXME/HACK/WARNING comments, memory/cache/DMA/RTOS/sensor/NPU notes, and selected generated AI network metadata.

#### Workflow C — Hydra/YAML config truth audit

```bash
uncomment inventory . \
  --languages yaml \
  --include-values \
  --config-system hydra
```

Expected result: both YAML comments and active config facts, including full key paths, defaults composition, `# @package`, allowed values, interpolations, runtime defaults, stream schemas, timestamping policies, and source-of-truth comments.

---

## 4. Current-state baseline from `uncomment`

The reviewed repository already contains strong foundations:

- Rust CLI with `clap`.
- Tree-sitter parsing through `tree-sitter` and `tree-sitter-language-pack`.
- Built-in language registry, including Python, C, C++, and YAML.
- AST visitor that records comment byte ranges, row ranges, node type, documentation status, and preservation status.
- Language handlers for Python docstrings and C-family preprocessor comment preservation.
- TOML-based config with global/language/pattern settings.
- File discovery with glob, gitignore-aware traversal, and parallel processing.
- Dry-run and diff support.
- Tests covering Python docstring detection versus regular strings.

The current implementation is still centered on removal:

```text
source file -> parse tree -> collect comments -> decide preserve/remove -> remove byte ranges -> write modified source or dry-run diff
```

The inventory product requires a parallel pipeline:

```text
source file -> parse tree -> collect all comments/docstrings/config facts -> attach context -> classify -> score -> export JSONL/Markdown/summary
```

---

## 5. Ideal feature set

### 5.1 Extraction capabilities

The ideal tool extracts:

- Ordinary comments.
- Documentation comments.
- Python module/class/function/method docstrings.
- Python Doxygen-like `# @param`, `# @return`, `# @note`, `# @warning` comments.
- C/C++ Doxygen blocks attached to functions, macros, typedefs, enums, structs, struct fields, and constants.
- C/C++ inline Doxygen field comments such as `/**< ... */` and `///< ...`.
- C/C++ preprocessor diagnostics such as `#warning` and `#error` as a sibling evidence type.
- YAML/Hydra comments attached to key paths.
- YAML/Hydra active value facts, even when no comments exist.
- YAML commented-out examples and option lists.
- Generated AI metadata, but isolated from normal local-source claims.

### 5.2 Context attachment

Each extracted item should include as much useful context as practical:

- Repository-relative path.
- Language.
- Line and byte ranges.
- Comment style/kind.
- Enclosing scope.
- Nearest following symbol.
- Nearest previous symbol.
- Qualified Python symbol when available.
- C/C++ target declaration kind: function, macro, enum, struct, field, typedef, static const.
- USER CODE region metadata.
- File-origin class: local, generated, vendor, test, fixture, generated AI network, third-party middleware.
- YAML full key path and active value.

### 5.3 Classification

The tool should classify comments/docstrings/config facts into:

- `contract_claim`
- `implementation_rationale`
- `lifecycle_status`
- `risk_workaround_marker`
- `test_intent`
- `requirement_traceability`
- `configuration_fact`
- `runtime_default`
- `allowed_values`
- `source_of_truth`
- `timing_synchronization_policy`
- `protocol_schema_contract`
- `generated_noise`
- `operator_guidance`

### 5.4 Scoring

Every record should have:

```json
{
  "score": 12,
  "priority": "high",
  "score_reasons": [
    "TODO marker",
    "domain term: DMA",
    "attached to macro",
    "inside USER CODE region"
  ]
}
```

Priority mapping:

```text
score >= 10  -> high
score 6-9    -> medium
score 3-5    -> low
score < 3    -> ignore unless requested
```

### 5.5 Output artifacts

Minimum outputs:

```text
comments_docstrings.jsonl
comments_docstrings.md
comments_docstrings_summary.md
comments_docstrings_by_symbol.json
```

Optional ideal outputs:

```text
comments_docstrings_by_file.json
comments_docstrings_by_domain.md
comments_docstrings_by_priority.md
yaml_config_claims.jsonl
inventory_manifest.json
```

---

## 6. MVP feature set

The first MVP should add a new subcommand:

```bash
uncomment inventory <paths...>
```

Recommended flags:

```bash
uncomment inventory . \
  --output-dir .agent/inventory \
  --format jsonl,markdown,summary \
  --languages python,c,cpp,yaml \
  --include-docstrings \
  --include-yaml-values \
  --respect-gitignore \
  --threads 0
```

### 6.1 MVP includes

- Reuse current file discovery, config resolution, language registry, and tree-sitter parser.
- Expose all comment records, not only comments selected for removal.
- Export JSONL and Markdown without modifying source files.
- Python docstring inventory using current docstring detection as the baseline.
- Python leading/inline comments and Doxygen-like `# @...` comments.
- C/C++ comments using existing tree-sitter comment nodes.
- C/C++ Doxygen classification using comment text and nearby declaration heuristics.
- USER CODE region metadata.
- Generated/vendor path classification with conservative defaults.
- YAML comments using tree-sitter comment nodes.
- YAML key-path/value facts using a YAML parser or a line-oriented MVP fallback.
- Scoring and priority classification using deterministic rules.

### 6.2 MVP excludes

- Full semantic contradiction detection.
- Embedding generation.
- LLM inference inside the tool.
- Perfect C/C++ symbol resolution.
- Full Doxygen AST parsing.
- Full Hydra composition resolution across defaults.
- Cross-file ownership inference beyond heuristic contradiction target hints.

---

## 7. Functional requirements

### FR-001 — inventory subcommand

The CLI must support:

```bash
uncomment inventory <files|directories|glob-patterns...>
```

The command must not modify source files.

### FR-002 — output directory

The user must be able to set:

```bash
--output-dir .agent/inventory
```

Default:

```text
.agent/inventory
```

### FR-003 — output formats

The user must be able to request any subset of:

```text
jsonl, markdown, summary, by-symbol
```

Default:

```text
jsonl, markdown, summary
```

### FR-004 — stable JSONL schema

Every extracted evidence item must be emitted as one JSON object per line with a stable schema. The schema is defined in `docs/output-contract.md`.

### FR-005 — Markdown optimized for retrieval

The Markdown artifact must be optimized for retrieval and agent consumption, not presentation. Each record should include path, line range, priority, claim types, domain terms, symbol/key path, and extracted text.

### FR-006 — current removal workflow compatibility

Existing `uncomment <paths...>` behavior must remain unchanged. Inventory mode must be additive.

### FR-007 — Python extraction

Python inventory must include:

- Module docstrings.
- Class docstrings.
- Function/method docstrings.
- Regular `#` comments.
- Doxygen-like `# @param`, `# @return`, `# @brief`, `# @note`, `# @warning`, `# @deprecated` comments.
- Test docstrings promoted when they contain `regression`, `must`, `should`, `bug`, `FR-`, `US`, or `schema`.

### FR-008 — C/C++ extraction

C/C++ inventory must include:

- `//` comments.
- `/* ... */` comments.
- `/** ... */`, `/*! ... */`, `///`, `//!` Doxygen comments.
- Doxygen attached to macros, constants, functions, structs, enum values, fields, and typedefs when heuristically detectable.
- Inline field comments `/**< ... */` and `///< ...`.
- Comments inside `USER CODE` regions, with region metadata.
- `#warning` and `#error` as preprocessor diagnostic evidence.

### FR-009 — YAML/Hydra extraction

YAML inventory must include:

- File-level comments.
- Leading comments attached to keys.
- Inline comments after values.
- Comments inside lists.
- `# @package` markers.
- Commented-out examples and option lists.
- Active key/value facts.
- Full key paths.
- Hydra defaults entries.
- Hydra/OmegaConf interpolations such as `${hydra:run.dir}`.

### FR-010 — generated/vendor noise filtering

The tool must classify file origin and suppress or down-rank generated/vendor noise by default while preserving high-value comments.

### FR-011 — scoring and priority

Every inventory record must include score, priority, and score reasons.

### FR-012 — deterministic output

Given the same inputs and config, the output order and record IDs must be stable.

---

## 8. Non-functional requirements

### NFR-001 — accuracy first

Comment detection must remain parser-based. Regex may be used only for classification of already-extracted comment text or for path/line heuristics. Regex must not be the primary comment detector.

### NFR-002 — non-destructive by design

Inventory mode must never write to source files.

### NFR-003 — performance

Inventory mode should preserve current parallel processing behavior. For the MVP, classification should be local and deterministic so it scales linearly with file count.

### NFR-004 — reviewability

The Markdown summary must be readable enough for manual triage before any automated contradiction pipeline is introduced.

### NFR-005 — configuration compatibility

Existing `.uncommentrc.toml` semantics should keep working. Inventory-specific settings should be additive.

---

## 9. Acceptance criteria

### AC-001 — no source mutation

Running:

```bash
uncomment inventory repo/
```

must not change any file under `repo/`.

### AC-002 — Python docstring precision

The current Python behavior that distinguishes real docstrings from ordinary strings must remain intact and be reused for inventory records.

### AC-003 — C macro/Doxygen example

Given:

```c
/** Number of LFSR clocks between consecutive image rows. */
#define ROW_STRIDE_BITS (1560U)
```

The inventory must emit a high-priority record attached to `ROW_STRIDE_BITS` with claim type `model_parameter` or `constant_contract`.

### AC-004 — USER CODE region metadata

Given:

```c
/* USER CODE BEGIN Includes */
// TODO: validate DMA cache policy
/* USER CODE END Includes */
```

The TODO record must include:

```json
{
  "inside_user_code_region": true,
  "user_code_region": "Includes"
}
```

The USER CODE markers themselves should not be emitted as claims unless explicitly requested.

### AC-005 — YAML key-path fact

Given:

```yaml
streams:
  reference:
    nominal_srate_hz: 500
```

The inventory must emit a configuration fact with key path:

```text
streams.reference.nominal_srate_hz
```

### AC-006 — YAML inline allowed values

Given:

```yaml
mode: active_send  # modbus_rtu | active_send
```

The inventory must include:

```json
{
  "key_path": "mode",
  "value": "active_send",
  "allowed_values_hint": ["modbus_rtu", "active_send"]
}
```

### AC-007 — JSONL parseability

Every line of `comments_docstrings.jsonl` must be valid JSON.

### AC-008 — Markdown retrieval quality

For each high-priority record, the Markdown artifact must include enough context to retrieve it by path, symbol/key path, domain term, marker, and extracted text.

---

## 10. Recommended implementation sequence

1. Add inventory data model and JSONL writer.
2. Add `inventory` subcommand with output directory and format flags.
3. Expose all collected comments from `CommentVisitor`.
4. Add deterministic record IDs and source hashing.
5. Add Python inventory enrichment.
6. Add C/C++ inventory enrichment and file-origin classification.
7. Add YAML key-path/value extraction.
8. Add scoring and Markdown writer.
9. Add summary/by-symbol output.
10. Add integration tests and fixtures for Python, C/C++, and YAML.

---

## 11. Success metrics

The MVP is successful when:

- It produces stable JSONL and Markdown for representative Python, C/C++, and YAML repositories.
- High-value TODO/FIXME/WARNING/DEPRECATED/default/schema/protocol/timestamping comments are easy to find.
- Generated/vendor noise is materially reduced compared with a raw comment dump.
- The output can be ingested by retrieval tools and used by agents to audit docs versus implementation.
- Current removal behavior remains unchanged.
