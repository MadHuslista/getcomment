# Gap Analysis: Current `uncomment` vs First MVP

This document narrows the gap analysis to the requested MVP: Python, C/C++, and YAML/Hydra configuration files.

---

## 1. MVP target

The first MVP should support:

```text
Python: .py, .pyw, .pyi
C:      .c, .h
C++:    .cpp, .hpp, .cc, .cxx, .hh, .hxx
YAML:   .yml, .yaml
```

Required outputs:

```text
.agent/inventory/comments_docstrings.jsonl
.agent/inventory/comments_docstrings.md
.agent/inventory/comments_docstrings_summary.md
.agent/inventory/comments_docstrings_by_symbol.json
```

---

## 2. MVP gap summary

| Area                         | Current status              | MVP requirement                                | Gap      |
| ---------------------------- | --------------------------- | ---------------------------------------------- | -------- |
| CLI                          | Removal command + `init`    | Add `inventory` subcommand                     | Critical |
| File discovery               | Present                     | Reuse                                          | None     |
| Language detection           | Python/C/C++/YAML present   | Reuse                                          | Low      |
| Tree-sitter parser           | Present                     | Reuse                                          | None     |
| All-comment collection       | Internal only               | Public inventory collection                    | High     |
| Python docstrings            | Detection present           | Emit as records                                | Medium   |
| Python Doxygen-like comments | Not classified              | Classify `# @param`, etc.                      | Medium   |
| C/C++ comments               | Generic extraction          | Emit as records                                | Medium   |
| C/C++ Doxygen targets        | Missing                     | Heuristic macro/type/function/field attachment | High     |
| USER CODE metadata           | Missing                     | Region detection                               | Medium   |
| YAML comments                | Generic extraction possible | Emit comment records                           | Medium   |
| YAML key paths               | Missing                     | Key-path/value facts                           | Critical |
| YAML active values           | Missing                     | Config fact records                            | Critical |
| JSONL writer                 | Missing                     | Required                                       | Critical |
| Markdown writer              | Missing                     | Required                                       | Critical |
| Summary writer               | Missing                     | Required                                       | High     |
| Scoring                      | Missing                     | Deterministic MVP scoring                      | High     |
| Generated/vendor filtering   | Not inventory-aware         | Source class + scoring                         | High     |

---

## 3. MVP implementation plan

### Phase 1 — Inventory data model

Add:

```text
src/inventory/model.rs
```

Core structs:

```rust
pub enum EvidenceKind {
    Comment,
    Docstring,
    DocumentationComment,
    YamlValue,
    YamlValueWithComment,
    PreprocessorDiagnostic,
}

pub enum Priority {
    High,
    Medium,
    Low,
    Ignore,
}

pub struct InventoryRecord { ... }
```

Required schema fields are defined in `output-contract.md`.

### Phase 2 — CLI surface

Add:

```bash
uncomment inventory <paths...>
```

Minimum flags:

```text
--output-dir <path>
--format <jsonl,markdown,summary,by-symbol>
--languages <python,c,cpp,yaml>
--min-priority <ignore|low|medium|high>
--include-generated
--include-low-priority
--no-yaml-values
```

### Phase 3 — Expose comment collection

Current `CommentVisitor` should expose all comments. Keep `get_comments_to_remove()` unchanged for removal mode.

Add:

```rust
pub fn comments(&self) -> &[CommentInfo]
```

or:

```rust
pub fn into_comments(self) -> Vec<CommentInfo>
```

### Phase 4 — Generic record construction

For each comment:

```text
path
language
line_start / line_end, 1-based
byte_start / byte_end
kind
node_type
raw_text
normalized_text
source_hash
```

### Phase 5 — Python enrichment

MVP Python enrichment:

```text
is_docstring
scope_type
scope_name
qualified_symbol when simple
structured_docstring_sections when easy
python_doxygen_tags
markers
requirement_ids
claim_types
score
```

Implementation note: use existing tree-sitter nodes first. Python `ast` can be a later enhancement for fully qualified symbols.

### Phase 6 — C/C++ enrichment

MVP C/C++ enrichment:

```text
comment style: line/block/doxygen/inline-field
nearest declaration text
nearest declaration kind
macro/constant attachment heuristic
USER CODE region detection
file-origin class
markers
requirement_ids
domain terms
claim_types
score
```

Preprocessor diagnostics can be extracted with a line scan because they are not comments. This should be implemented as a separate evidence kind, not as comment detection.

### Phase 7 — YAML/Hydra enrichment

MVP YAML enrichment:

```text
comments from tree-sitter-yaml
line-oriented key-path stack
active key/value facts
leading comment attachment
inline comment attachment
# @package metadata
Hydra defaults entries
interpolation extraction
allowed-values hints from inline comments
commented-out examples
```

The MVP can use a deterministic line-oriented parser for key paths if a comment-preserving YAML parser is not adopted immediately. Every YAML record should include a `confidence` field for attachment quality.

### Phase 8 — Writers

Add writers:

```text
jsonl writer
markdown writer
summary writer
by-symbol JSON writer
```

### Phase 9 — Tests

Add fixtures:

```text
fixtures/inventory/python/docstrings.py
fixtures/inventory/python/doxygen_like.py
fixtures/inventory/c/doxygen_macros.c
fixtures/inventory/c/user_code_regions.c
fixtures/inventory/cpp/inline_field_comments.hpp
fixtures/inventory/yaml/hydra_config.yaml
fixtures/inventory/yaml/comments_and_values.yaml
```

---

## 4. Python MVP details

### Current capability

The repo already knows how to treat Python strings as docstrings only when they are first statements in modules/classes/functions/methods.

### Missing MVP behavior

The code must emit those docstrings as records instead of only using them for removal decisions.

Required additional classification:

```text
module_docstring
class_docstring
function_docstring
method_docstring
python_doxygen_comment
test_docstring
```

### MVP acceptance fixture

```python
"""Module contract."""

class Filter:
    """Not used in the default configuration."""

    def apply(self, x):
        """Return filtered sample."""
        return x

## @param level Logging level.
# TODO: validate timestamp alignment.
```

Expected:

```text
4+ records
class docstring high priority due to default/status text
TODO high priority
@param medium/high as API contract
```

---

## 5. C/C++ MVP details

### Current capability

C and C++ are registered and comments are detected generically.

### Missing MVP behavior

The MVP must add firmware-oriented context:

```text
Doxygen target attachment
USER CODE region metadata
source-origin classification
macro/constant priority boosting
inline field comments
preprocessor diagnostics
```

### MVP acceptance fixture

```c
/* USER CODE BEGIN Includes */
// TODO: validate DMA cache policy
/* USER CODE END Includes */

/** Number of LFSR clocks between consecutive rows. */
#define ROW_STRIDE_BITS (1560U)

uint32_t exact; /**< Pixels matching the model exactly. */

#warning "Possible mismatch in ll_aton library used"
```

Expected:

```text
TODO record: high priority, inside_user_code_region=true
ROW_STRIDE_BITS record: high priority, claim_type=model_parameter
field comment: attached to exact
#warning record: preprocessor_diagnostic, high priority
USER CODE markers not emitted as claims by default
```

---

## 6. YAML/Hydra MVP details

### Current capability

YAML is registered as a language with comment nodes. This supports generic comment removal.

### Missing MVP behavior

The MVP must treat YAML as configuration evidence, not comment-only source.

Required:

```text
full key paths
active values
leading comments
inline comments
allowed-values hints
Hydra defaults
# @package
interpolations
commented-out examples
```

### MVP acceptance fixture

```yaml
# @package bridge

defaults:
  - logging: default
  - _self_

# batch_end_anchored prevents synthetic clock drift.
timestamp_policy: batch_end_anchored

mode: active_send  # modbus_rtu | active_send

excluded_ports:
  # - /dev/ttyUSB0

logging:
  file: ${hydra:run.dir}/run.log
```

Expected records:

```text
Hydra package metadata
Hydra defaults[0], defaults[1]
timestamp_policy value + leading comment, high priority
mode value + allowed_values_hint
commented example under excluded_ports
logging.file value + interpolation hydra:run.dir
```

---

## 7. MVP scoring rules

Base:

```text
docstring attached to public symbol        +3
comment attached to function/class/macro   +2
inline/local comment                       +1
YAML active config fact                    +2
```

Markers:

```text
TODO/FIXME/BUG/HACK/WARNING/WORKAROUND     +5
NOTE/IMPORTANT/CAUTION/REVIEW              +3
DEPRECATED/LEGACY                          +5
```

Semantic:

```text
must/should/required/guarantee             +4
format/schema/contract/protocol            +4
default/optional/not used                  +4
units/rate/range/path/field                +3
domain term                                +2
requirement ID                             +4
```

C/C++ additions:

```text
Doxygen attached to macro/constant          +4
inside USER CODE region                     +3
mentions DMA/cache/NPU/RAW10/LFSR/etc.      +3
preprocessor #warning/#error                +5
vendor/generated boilerplate                -6
```

YAML additions:

```text
Hydra defaults entry                        +4
Hydra @package marker                       +3
comment attached to active key              +3
inline allowed-values list                  +4
stream/protocol/schema/ipc key              +5
timestamp/alignment key                     +5
quality/fit/calibration key                 +4
display-only/not acquisition truth          +5
authoritative/source of truth               +5
legacy/deprecated/no effect                 +5
```

Priority mapping:

```text
score >= 10  -> high
score 6-9    -> medium
score 3-5    -> low
score < 3    -> ignore unless requested
```

---

## 8. MVP non-goals

```text
No automatic contradiction detection
No LLM calls
No embeddings
No documentation rewriting
No perfect C++ semantic model
No full Hydra composition resolution
No replacement for Doxygen/Sphinx
```

---

## 9. Recommended first PR breakdown

### PR 1 — data model and inventory command skeleton

- Add `inventory` subcommand.
- Add output-dir/format flags.
- Add inventory record model.
- Add empty output files.

### PR 2 — generic comment JSONL export

- Expose all comments from visitor.
- Emit generic records with path, language, ranges, raw text.

### PR 3 — Python enrichment

- Docstrings as records.
- Doxygen-like Python tags.
- Basic symbol context.

### PR 4 — C/C++ enrichment

- Doxygen style detection.
- USER CODE regions.
- source-origin classification.
- `#warning`/`#error` diagnostics.

### PR 5 — YAML/Hydra enrichment

- Key-path/value facts.
- Leading/inline comment attachment.
- Hydra metadata.

### PR 6 — Markdown, summary, scoring

- Markdown retrieval output.
- Summary output.
- Score reasons.
- By-symbol JSON.

---

## 10. MVP done definition

The MVP is done when the following command works on a mixed repository:

```bash
uncomment inventory . \
  --output-dir .agent/inventory \
  --format jsonl,markdown,summary,by-symbol \
  --languages python,c,cpp,yaml
```

and produces valid, deterministic files that allow a human or agent to identify high-priority comments/docstrings/config facts without opening every source file manually.
