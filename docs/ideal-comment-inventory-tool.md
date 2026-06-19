# Ideal Comment Inventory Tool Specification

This document defines the target behavior for the ideal comment inventory tool.

---

## 1. Product definition

The ideal tool is a non-destructive source evidence extractor.

It inventories:

- source comments,
- documentation comments,
- Python docstrings,
- C/C++ Doxygen claims,
- YAML/Hydra comments,
- YAML/Hydra active config values,
- selected non-comment implementation evidence such as `#warning`/`#error`.

It does **not** remove source code and does **not** attempt to automatically rewrite documentation. Its job is to make hidden or scattered claims explicit and retrievable.

---

## 2. Conceptual model

### 2.1 Evidence record

An evidence record is one extracted item that may contain a claim.

Examples:

```text
Python function docstring
C macro Doxygen block
C inline field comment
YAML key with inline allowed-values comment
YAML active value fact
C #warning diagnostic
TODO inside USER CODE region
```

### 2.2 Claim

A claim is a statement or value that may need to be checked against docs, tests, configs, or implementation.

Examples:

```text
"Not used in the default configuration."
"Expected RAW10 value for a fully dark test-pattern sample."
"timestamp_policy: batch_end_anchored"
"Display-only alignment; raw LSL/XDF timestamps are not modified."
```

### 2.3 Inventory artifact

An inventory artifact is a generated file that organizes evidence records for machines and humans.

Required artifacts:

```text
comments_docstrings.jsonl
comments_docstrings.md
comments_docstrings_summary.md
comments_docstrings_by_symbol.json
```

---

## 3. Processing pipeline

```text
source files
  -> language-aware extraction
  -> generated/vendor/noise classification
  -> symbol/key-path attachment
  -> text normalization
  -> marker and requirement-ID extraction
  -> domain tagging
  -> claim classification
  -> scoring
  -> contradiction-target hinting
  -> JSONL/Markdown/summary emission
```

---

## 4. Core extraction rules

### 4.1 General comments

Extract parser-identified comments only. Regex may classify comment text after extraction, but must not be used to identify comments directly when a parser is available.

### 4.2 Documentation comments

Documentation comments must be preserved as evidence and marked as documentation.

Examples:

```text
/// Rust/C++ style doc comment
/** Doxygen/Javadoc style block */
//! module-level doc comment
## Python Doxygen-like comment
```

### 4.3 Marker comments

High-value markers:

```text
TODO
FIXME
HACK
BUG
WARNING
CAUTION
WORKAROUND
XXX
TBD
IMPORTANT
NOTE
REVIEW
DEPRECATED
LEGACY
```

These should increase priority, not simply be preserved or removed.

### 4.4 Requirement identifiers

Extract IDs such as:

```text
US08
US09
FR-023
FR-101a
SP-005
MFW-234
Bug #2
T045
```

Requirement IDs should be stored as searchable fields.

---

## 5. Language rules

See `language-extraction-rules.md` for detailed Python, C/C++, and YAML/Hydra rules.

---

## 6. Classification taxonomy

### 6.1 Contract claim

Statements defining behavior, schema, format, range, unit, default, or API behavior.

Triggers:

```text
must
should
required
guarantees
expected
returns
raises
format
schema
contract
sample rate
units
range
```

### 6.2 Implementation rationale

Explains why code exists.

Triggers:

```text
because
workaround
avoid
prevents
needed due to
kept for compatibility
retained for optional use
```

### 6.3 Lifecycle/status claim

Signals deprecated, legacy, optional, temporary, experimental, or removed behavior.

### 6.4 Risk/workaround marker

Captures TODO/FIXME/HACK/BUG/WARNING/WORKAROUND/etc.

### 6.5 Test-intent claim

Captures test docstrings and comments that define behavior, regression purpose, or bug-fix guard intent.

### 6.6 Requirement traceability claim

Captures requirement IDs, ticket IDs, bug IDs, and feature references.

### 6.7 Configuration fact

Captures active YAML/Hydra values, defaults, interpolations, config schemas, allowed values, and source-of-truth statements.

### 6.8 Generated/noise comment

Classifies generated/vendor comments so they can be ignored or down-ranked unless they contain high-value markers.

---

## 7. Domain dictionary

The ideal tool should ship a generic vocabulary and allow user-defined domain dictionaries. For the target repo family, start with:

```yaml
domains:
  acquisition:
    - sample
    - sampling
    - ADC
    - HX711
    - force
    - fixture
    - target
    - reference

  streams:
    - LSL
    - stream
    - outlet
    - inlet
    - channel
    - timestamp
    - synchronization
    - ref-shift
    - latency
    - drift

  serial:
    - UART
    - serial
    - RS485
    - Modbus
    - active-send
    - frame
    - packet
    - parser
    - codec

  outputs:
    - CSV
    - XDF
    - report
    - manifest
    - artifact
    - log
    - calibration_report

  calibration:
    - calibration
    - affine
    - fit
    - residual
    - quality
    - segmentation
    - protocol

  analysis:
    - stage
    - warmup
    - noise
    - drift
    - dynamics
    - interference
    - filter
    - Butterworth
    - biquad

  embedded:
    - NPU
    - DCMI
    - DCMIPP
    - DMA
    - HPDMA
    - cache
    - RISAF
    - IAC
    - XSPI
    - ThreadX
    - UART
    - sensor
    - Bayer
    - RAW10
    - frame
```

---

## 8. Noise filtering

Suppress or down-rank by default:

```text
license headers
copyright banners
autogenerated boilerplate
HAL/CMSIS boilerplate
USER CODE markers alone
pure section separators
trivial comments under 20 chars
commented-out code without explanation
generic YAML logging/UI values without domain relevance
```

Never suppress when text contains:

```text
TODO
FIXME
BUG
HACK
WARNING
WORKAROUND
DEPRECATED
must
contract
schema
protocol
not used
default
legacy
regression
source of truth
display-only
500 Hz
2 ms
latency
drift
```

---

## 9. Output philosophy

JSONL is the source of truth for tools. Markdown is the retrieval artifact for LLMs. Summary Markdown is the human review artifact.

The tool should prefer transparency over hidden intelligence. Every score should include score reasons so humans can tune the model.
