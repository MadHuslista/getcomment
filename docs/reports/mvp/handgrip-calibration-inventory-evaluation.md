# MVP Evaluation: `uncomment inventory` on Handgrip_Calibration

---
- **Creation Date**: 2026-06-19
- **Last Updated** : 2026-06-19
- **Branch@Commit**: local@49e0242
- **Scope**: Evaluation of `uncomment inventory` MVP run against `Handgrip_Calibration` (representative library inside `madhus.project.handgrip`), generated via project `.uncommentrc.toml` `[inventory]` config rather than CLI flags.
- **Purpose**: Validate the MVP against the intended-usage workflow (evidence generation → agent ingest → priority triage → downstream contradiction audit) and produce concrete, reproducible gap findings for follow-up issues.
- **Claude Plan Path**: /home/levi/.claude/plans/wondrous-sparking-hamming.md
- **Version**: 1.0
---

## 1. What was measured and why

The intended usage of inventory mode (per `PRD.md` §3.2 and prior discussion) is:

1. Generate a non-destructive evidence layer (`uncomment inventory`).
2. Ingest the JSONL/Markdown artifacts into agent/retrieval tooling.
3. Triage by `priority`/`score` to surface high-value claims first.
4. Use the evidence (path + line range + extracted text) to cross-check documentation/tests for drift — a downstream agent task, explicitly out of scope for the tool itself.

This evaluation checks whether the artifacts actually generated against a real, representative library (`Handgrip_Calibration`: pure Python + Hydra YAML config, no C/C++ in-repo) support that workflow, and measures recall/precision against ground truth pulled independently from the source (Python `ast` module, `grep`).

## 2. How it was measured

- **Run**: `uncomment inventory ./Handgrip_Calibration` executed from `/home/levi/Repositories/madhus.project.handgrip`, configured via that repo's `.uncommentrc.toml` `[inventory]` table (`output_dir = ".agent/inventory"`, `formats = ["jsonl","markdown","summary","by-symbol"]`, `languages = ["python","c","cpp","yaml"]`, `min_priority = "low"`, `include_generated = false`).
- **Config/CLI parity check**: read `src/cli.rs` `InventoryArgs` and confirmed the TOML `[inventory]` keys map 1:1 to the same fields the CLI flags set, following the same precedence chain (CLI > local config > global > defaults) as every other `uncomment` command. No separate code path for config-driven inventory runs.
- **Artifact inspection**: read `inventory_manifest.json`, `comments_docstrings_summary.md`, and parsed all 1914 lines of `comments_docstrings.jsonl` with a Python script to census field population (`markers`, `domain_terms`, `claim_types`, nested `python`/`yaml` objects, `docstring_sections`, etc.).
- **Ground truth — Python docstrings**: independent AST walk (`ast.get_docstring` over `ast.Module`/`ClassDef`/`FunctionDef`/`AsyncFunctionDef`) across every `.py` file in the library (excluding `.venv`), diffed per-file against the tool's `kind: docstring` record counts.
- **Ground truth — markers**: `grep -rIn -E "TODO|FIXME|HACK|XXX|WARNING|DEPRECATED"` across the library, cross-checked against the tool's `markers` field population.
- **Ground truth — YAML comments**: `grep -c '#'` across all 13 files in `conf/`.
- **C/C++ scope check**: `find` for `.c/.h/.cpp/.hpp` outside `.venv` in the library, confirming none exist.

## 3. Results

### 3.1 Manifest / scale

```
files_scanned: 48
files_with_records: 40
records: 1914
errors: []
```

48 ≈ 35 `.py` + 13 `conf/*.yaml` files in the library (no C/C++ subject matter present, correctly reflected as zero C/C++ records despite being requested).

### 3.2 Summary breakdown

```
by language:  yaml 1633, python 281
by priority:  medium 1614, low 234, high 66
by claim type: configuration_fact 1633, runtime_default 1633,
               protocol_schema_contract 768, contract_claim 114,
               test_intent 4, risk_workaround_marker 1
by marker:     WARNING 1
by file origin: local 1904, test_harness 10
generated/vendor suppressed: 0
```

### 3.3 Output-contract compliance

All five files required by `docs/output-contract.md` §1 and §9 are present and well-formed: `comments_docstrings.jsonl` (1914 valid JSON lines), `comments_docstrings.md`, `comments_docstrings_summary.md`, `comments_docstrings_by_symbol.json`, `inventory_manifest.json`. Markdown record shape matches the contract's example (path/line range, Type, Priority, Score, Claim types, Symbol/Key path, extracted text, score reasons, contradiction targets).

### 3.4 Config-vs-CLI parity

Confirmed: `.uncommentrc.toml`'s `[inventory]` table sets the same fields the CLI flags do, through the same config-merge precedence used elsewhere in the tool. The user's assumption — config-driven run produces identical results to the equivalent CLI invocation — holds structurally; no separate/divergent code path exists for config-sourced inventory settings.

### 3.5 Recall — Python docstrings

AST ground truth: **130** docstrings across the library. Tool emitted: **120** `kind: docstring` records. **Recall: 92.3%** (120/130).

### 3.6 Confirmed-working features

- YAML key-path extraction (`streams.reference.nominal_srate_hz`, `defaults[0]`, etc.) — correct and present on every active-value record.
- Hydra defaults-entry detection (`Hydra defaults entry` score reason, contributing to the 768 `protocol_schema_contract` records).
- Scoring/priority pipeline runs deterministically end-to-end and produces non-trivial differentiation (66 high / 1614 medium / 234 low).
- File-origin classification (`local` vs `test_harness`) works and correctly identifies test fixtures.
- Python scope/qualified-symbol attachment works for nested scopes (e.g. `SessionManager.manifest_dict`).
- Python Doxygen-like `# @...` comment classification works (102 records carry `doxygen_tags`).

## 4. Gaps

Each gap below is written to be directly usable as a tracked issue.

---

### GAP-1 (High) — Single-line module docstrings are never extracted

**Evidence**: AST ground truth = 130 docstrings; tool = 120. Diffing per-file counts, every missing docstring is a **single-line, module-level** docstring (`line_start == line_end`, first statement in the file). Confirmed missing entirely in:

```
src/handgrip_calibration/__main__.py     (1 missing)
src/handgrip_calibration/cli.py          (1 missing)
src/handgrip_calibration/session.py      (1 missing)
src/handgrip_calibration/synthetic.py    (1 missing)
src/handgrip_calibration/_utils.py       (1 missing)
src/handgrip_calibration/report.py       (1 missing)
src/handgrip_calibration/segmentation.py (1 missing)
src/handgrip_calibration/quality.py      (1 missing)
src/handgrip_calibration/lsl_io.py       (1 missing)
tests/conftest.py                        (1 missing)
```

Example: `src/handgrip_calibration/cli.py:1` — `"""Command-line interface for the Handgrip_Calibration package."""` — does not appear anywhere in `comments_docstrings.jsonl` for that file; only the function-level docstring at line 173 is present.

Multi-line module docstrings (e.g. `scripts/analyze_step_relaxation.py:2-22`) extract correctly, as do all class/function docstrings regardless of line count. The failure is specific to **module-scope + single physical line**.

**Root cause hypothesis**: the module-docstring detection path in `src/inventory/python.rs` likely special-cases module scope separately from class/function scope (since module has no enclosing `def`/`class` node to anchor on), and that special case probably assumes or requires a multi-line span, or otherwise diverges from the generic single-line string-literal-as-docstring path used for function/class docstrings.

**Suggested fix location**: `src/inventory/python.rs`, module docstring detection logic. Add a fixture: a `.py` file whose *only* statement is a single-line module docstring, assert it appears in the inventory.

**Impact**: 7.7% of all docstrings in this real corpus silently dropped — these are often the most useful records (one-line "what this file is for" statements), and they are the exact kind of `contract_claim` evidence PRD intends the tool to surface.

---

### GAP-2 (Medium) — Marker detection has no context/word-boundary guard, producing false positives

**Evidence**: record `docstring:./Handgrip_Calibration/src/handgrip_calibration/logging_setup.py:42:53:0c8caab9` is flagged `markers: ["WARNING"]`, `claim_types: ["risk_workaround_marker"]`, `priority: high`, `score: 15`. The actual text is:

```
Console verbosity.  One of ``DEBUG``, ``INFO``, ``WARNING``,
``ERROR``, ``CRITICAL``.  Case-insensitive.
```

This is a parameter docstring enumerating valid log-level strings, not a warning/risk marker. It is the **only** marker hit across all 1914 records in this corpus (per the summary's "by marker" section), and it's a false positive — meaning marker-based detection contributed zero true positives in this run and one false positive.

**Root cause hypothesis**: marker matching is a substring/keyword scan over `normalized_text` without word-boundary checks or positional constraints (e.g. requiring the marker to appear comment-initial, or excluding occurrences inside backtick/code-quoted spans).

**Suggested fix location**: wherever `markers` extraction runs (likely `src/inventory/scoring.rs` or a shared text-classification helper called from `python.rs`/`c_family.rs`/`yaml.rs`). Add word-boundary regex/token matching, and consider excluding matches inside inline-code spans (`` `...` `` / ``` ``...`` ```) since those are typically enumerating values, not flagging risk.

**Impact**: inflates an unrelated record to `high` priority/score 15 — actively misleading for the priority-triage workflow this tool exists to support.

---

### GAP-3 (Low) — `docstring_sections` (structured NumPy/Google docstring parsing) is unimplemented

**Evidence**: 0/120 docstring records populate the optional `python.docstring_sections` field, even where the raw text clearly contains parseable `Parameters` / `Returns` sections (e.g. `configure_logging`'s docstring in `logging_setup.py`).

**Status**: this matches `docs/gap-analysis-current-vs-mvp.md` Phase 5's "when easy" framing — it's explicitly optional for MVP, not a contract violation. Listed here as a ready next-iteration enhancement since real source data already exists to validate against.

---

### GAP-4 (Low) — `by_symbol` fallback naming for module docstrings is weak

**Evidence**: `comments_docstrings_by_symbol.json` entries for module-level docstrings use `path:line` as the `symbol` key (e.g. `./Handgrip_Calibration/scripts/analyze_step_relaxation.py:2`) rather than a module-name-derived symbol (e.g. `handgrip_calibration.analyze_step_relaxation` or similar).

**Impact**: minor — reduces the by-symbol lookup's usefulness specifically for module-level claims; doesn't affect JSONL/Markdown correctness.

---

### GAP-5 (Info, non-blocking) — YAML comment-attachment features have zero real-world coverage in this corpus

**Evidence**: all 13 files under `Handgrip_Calibration/conf/` contain **zero** `#` characters (verified by grep), so `leading_comment`, `inline_comment`, `allowed_values_hint`, and Hydra interpolation extraction are 0/1633 populated in this run. This is a property of the source data, not evidence the features are broken.

**Recommendation**: don't treat this run as validation (positive or negative) for AC-006 (allowed-values hint) or interpolation extraction. Those should be checked against the existing `fixtures/inventory/yaml/` fixtures (added in `ce90b17`/`01d170f`) instead. If those fixture tests pass, the feature can be considered verified independent of this report.

---

### GAP-6 (Observation, not a code bug) — Priority distribution is heavily skewed to "medium"

**Evidence**: 1614/1914 records (84%) are `medium` priority, driven by the flat `+2 configuration_fact` baseline that the MVP scoring rules (per `docs/gap-analysis-current-vs-mvp.md` §7) apply to essentially every active YAML value. Only 66 records (3.4%) reach `high`.

**Implication for the intended workflow**: PRD's triage step ("Step 4 — triage by priority") expects priority to meaningfully thin the review set. On a YAML-config-heavy repo like this one, an agent doing `--min-priority medium` still gets ~1680 records to wade through — not materially better than no filtering. This is consistent with the MVP's explicit non-goal of contradiction detection (the scoring is intentionally coarse), but it's worth flagging as a known limitation when sizing realistic agent workflows against YAML-dominant repos.

## 5. Conclusion

The MVP substantially matches the PRD and `output-contract.md` for a real mixed Python+YAML repository: the full pipeline (discovery → tree-sitter extraction → scoring → multi-format export) runs end-to-end, deterministically, without touching source files, and config-driven runs are structurally equivalent to CLI-flag runs.

Two concrete, fixable extraction bugs were found:

- **GAP-1** (high severity): single-line module docstrings are silently dropped — a precise, reproducible parser gap, not a fuzzy heuristic problem.
- **GAP-2** (medium severity): marker detection lacks context guards, producing a false-positive high-priority record.

The remaining items (GAP-3, GAP-4, GAP-6) are enhancement opportunities or known MVP-scope limitations, and GAP-5 is a note that this particular corpus cannot validate YAML comment-attachment features one way or the other — that needs the existing fixture suite, not this real-world run.
