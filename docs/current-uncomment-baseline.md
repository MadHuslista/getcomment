# Current `uncomment` Baseline Review

This document summarizes the current implementation baseline used for the PRD and gap analysis.

---

## 1. Repository shape

The uploaded repository is a Rust CLI project. Important files reviewed:

```text
Cargo.toml
README.md
src/main.rs
src/cli.rs
src/processor.rs
src/ast/visitor.rs
src/languages/config.rs
src/languages/registry.rs
src/languages/handlers.rs
src/rules/preservation.rs
src/config.rs
tests/python_docstring_test.rs
tests/integration_tests.rs
.ai-rulez/context/architecture.md
.ai-rulez/rules/tree-sitter-ast-parsing.md
.ai-rulez/rules/preservation-rules.md
```

The codebase is structured around comment removal, not comment inventory.

---

## 2. Existing strengths

### 2.1 Parser-based comment detection

The project uses tree-sitter to parse source files and find comment/docstring nodes. This is the correct foundation for inventory mode because it avoids false positives from comment-like content inside strings.

### 2.2 Language registry

`src/languages/config.rs` and `src/languages/registry.rs` define built-in languages and extensions. Python, C, C++, and YAML are already present:

```text
python -> py, pyw, pyi, pyx, pxd
c      -> c, h
cpp    -> cpp, cxx, cc, c++, hpp, hxx, hh, h++
yaml   -> yaml, yml
```

The registry is directly reusable for inventory mode.

### 2.3 Comment metadata already captured

`CommentInfo` currently records:

```text
start_byte
end_byte
start_row
end_row
node_type
should_preserve
is_documentation
```

This is a useful low-level base for inventory records.

### 2.4 Python docstring detection

`PythonHandler` already distinguishes real module/class/function/method docstrings from ordinary string literals by checking the AST parent and first-statement position.

The tests in `tests/python_docstring_test.rs` validate that:

- module docstrings are removed when configured,
- function/class/method docstrings are removed when configured,
- assigned or post-assignment strings are preserved as normal strings.

This behavior should be reused for inventory mode.

### 2.5 Preservation engine

`src/rules/preservation.rs` contains default and comprehensive preservation rules. These already encode useful concepts:

```text
TODO
FIXME
HACK
NOTE
WARNING
BUG
REVIEW
OPTIMIZE
PERFORMANCE
SECURITY
DEPRECATED
linting directives
file headers
shebangs
documentation comments
```

Inventory mode should reuse this pattern knowledge, but invert the purpose: these markers should become classification and priority signals, not just preservation/removal decisions.

### 2.6 C-family special handling

`CFamilyHandler` preserves comments that are trailing preprocessor comments. This is a useful starting point, but far from enough for firmware-oriented inventory.

### 2.7 File discovery and parallel execution

`main.rs` already implements:

- files/directories/glob pattern input,
- gitignore-aware traversal,
- unsupported file reporting,
- optional nested Git repo traversal,
- parallel file processing with Rayon.

Inventory mode should reuse this infrastructure.

### 2.8 TOML configuration system

`src/config.rs` supports hierarchical configuration with global, language, and pattern-level overrides. Inventory settings can be added without breaking removal behavior.

---

## 3. Current processing flow

Current removal flow:

```text
CLI args
  -> collect files
  -> resolve config
  -> detect language by extension
  -> parse source with tree-sitter
  -> visit AST comments/docstrings
  -> apply preservation rules
  -> select comments to remove
  -> remove byte ranges from source
  -> write modified source or dry-run/diff
```

Inventory flow needed:

```text
CLI args
  -> collect files
  -> resolve config
  -> detect language by extension
  -> parse source with tree-sitter
  -> visit AST comments/docstrings
  -> collect all evidence records
  -> attach context/symbol/key path
  -> classify and score
  -> emit JSONL/Markdown/summary
```

---

## 4. Current gaps relevant to inventory mode

### 4.1 No inventory command

The CLI has only the default processing mode and `init`. There is no `inventory`, `extract`, `comments-only`, or structured export mode.

### 4.2 `CommentVisitor` does not expose all comments publicly

`CommentVisitor` stores all comments internally, but only exposes `get_comments_to_remove()`. Inventory mode needs a public method such as:

```rust
pub fn comments(&self) -> &[CommentInfo]
```

or an owned collection method.

### 4.3 Records lack context beyond byte/row ranges

Current `CommentInfo` does not include:

```text
path
language
line_start as 1-based line
line_end as 1-based line
raw_text
normalized_text
parent node kind
nearest symbol
qualified symbol
YAML key path
C/C++ target declaration
USER CODE region
file-origin class
classification tags
priority score
```

### 4.4 Output is modified source, not evidence artifacts

`ProcessedFile` contains original and processed source, comment count, and important removal samples. There are no JSONL, Markdown, summary, or by-symbol outputs.

### 4.5 YAML support is comment-node-only

YAML is registered as a language with `comment` nodes, so comments can be removed. However, the ideal YAML/Hydra inventory requires active value facts and key-path attachment. That requires a YAML value traversal pass in addition to tree-sitter comment extraction.

### 4.6 Preservation is not classification

Current rules answer: “Should this comment be preserved?”

Inventory mode needs to answer: “What does this comment claim, how important is it, and what should it be checked against?”

### 4.7 C/C++ semantics are under-modeled

Current C/C++ behavior sees comments as generic comment nodes. The target workflow needs special handling for:

```text
Doxygen comments on macros/constants/functions/types/fields
inline field comments
firmware USER CODE regions
generated/vendor source classification
preprocessor #warning/#error
generated AI network metadata
commented-out code with rationale
```

### 4.8 No domain dictionary

The current tool has generic preserve patterns but no domain dictionaries for firmware, streams, timestamping, calibration, protocol, YAML/Hydra, or embedded systems.

### 4.9 No contradiction-target hints

The current tool has no concept of mapping a comment claim to likely docs/tests/config/source files that should be checked.

---

## 5. Architectural leverage points

The inventory feature can be added without rewriting the tool.

Recommended additions:

```text
src/inventory/mod.rs
src/inventory/model.rs
src/inventory/extract.rs
src/inventory/classify.rs
src/inventory/score.rs
src/inventory/writers/jsonl.rs
src/inventory/writers/markdown.rs
src/inventory/writers/summary.rs
src/inventory/languages/python.rs
src/inventory/languages/c_family.rs
src/inventory/languages/yaml.rs
```

Recommended existing modules to reuse:

```text
src/main.rs                file collection and threading pattern
src/cli.rs                 clap command integration
src/config.rs              config discovery and merge model
src/languages/registry.rs  language detection
src/ast/visitor.rs         low-level comment discovery
src/rules/preservation.rs  marker/pattern vocabulary
src/processor.rs           parser setup and tree traversal pattern
```

---

## 6. Main implementation risk

The biggest risk is trying to make inventory mode semantically perfect too early. The first implementation should focus on deterministic extraction, context attachment, and transparent scoring. Full contradiction detection should remain downstream.
