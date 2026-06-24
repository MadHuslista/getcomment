# MVP Evaluation: `uncomment inventory` on Handgrip_Calibration

---
- **Creation Date**: 2026-06-19
- **Last Updated** : 2026-06-24
- **Branch@Commit**: local@051d851
- **Scope**: Evaluation of `uncomment inventory` MVP run against two representative libraries from the `madhus.project.handgrip` corpus — `Handgrip_Calibration` (pure Python + bare Hydra YAML) and `LSL_Viewer` (Python + heavily-commented Hydra YAML) — cloned read-only at `tests/integration_test/repos_cache/handgrip-calibration/` (branch `tmain`) per `scripts/fetch-inventory-corpus.sh`.
- **Purpose**: Validate the MVP against the intended-usage workflow (evidence generation → agent ingest → priority triage → downstream contradiction audit) and produce concrete, reproducible gap findings for follow-up issues.
- **Claude Plan Path**: /home/levi/.claude/plans/purrfect-conjuring-quiche.md
- **Version**: 1.3
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
- **Second run (2026-06-20)**: `uncomment inventory ./LSL_Viewer` from the corpus root, same `.uncommentrc.toml`-equivalent defaults. Used to re-validate GAP-5/GAP-6 against a corpus with real YAML comments, and to re-investigate GAP-1 with `--min-priority ignore --include-low-priority` (outputs `/tmp/inv3` for `Handgrip_Calibration`, `/tmp/inv4` for `LSL_Viewer`) after the default-settings diff produced a misleading root cause the first time — see corrected GAP-1 below.

## 3. Results

### 3.1 Manifest / scale

```
files_scanned: 48
files_with_records: 41
records: 1924
errors: []
```

48 ≈ 35 `.py` + 13 `conf/*.yaml` files in the library (no C/C++ subject matter present, correctly reflected as zero C/C++ records despite being requested). (Numbers are the v1.3 current-source run, `local@051d851`; the original v1.0 baseline run reported 40 files-with-records / 1914 records before the GAP fixes landed — see the resolution banner in §4 for the before→after delta.)

### 3.2 Summary breakdown

```
by language:  yaml 1633, python 291
by priority:  medium 1613, low 241, high 70
by claim type: configuration_fact 1633, runtime_default 1633,
               protocol_schema_contract 768, contract_claim 114,
               test_intent 5, implementation_rationale 2
by marker:     (none)
by file origin: local 1913, test_harness 11
generated/vendor suppressed: 0
```

(v1.3 current-source run. Deltas vs the v1.0 pre-fix baseline: python 281 → 291 — the
ten module-scope docstrings GAP-1 used to filter now surface; `by marker: WARNING 1` →
`(none)` — the GAP-2 code-span false positive is gone; `risk_workaround_marker 1` drops
out with it; `implementation_rationale 2` is newly populated from GAP-3 docstring-section
parsing.)

### 3.3 Output-contract compliance

All five files required by `docs/output-contract.md` §1 and §9 are present and well-formed: `comments_docstrings.jsonl` (1914 valid JSON lines), `comments_docstrings.md`, `comments_docstrings_summary.md`, `comments_docstrings_by_symbol.json`, `inventory_manifest.json`. Markdown record shape matches the contract's example (path/line range, Type, Priority, Score, Claim types, Symbol/Key path, extracted text, score reasons, contradiction targets).

### 3.4 Config-vs-CLI parity

Confirmed: `.uncommentrc.toml`'s `[inventory]` table sets the same fields the CLI flags do, through the same config-merge precedence used elsewhere in the tool. The user's assumption — config-driven run produces identical results to the equivalent CLI invocation — holds structurally; no separate/divergent code path exists for config-sourced inventory settings.

### 3.5 Recall — Python docstrings

AST ground truth: **130** docstrings across the library. v1.0 baseline emitted **120** `kind: docstring` records at default settings (recall 92.3%, 120/130) — the 10 missing were module-scope docstrings filtered by `min_priority=low` (GAP-1, a scoring gap, not extraction). On the v1.3 current-source run the GAP-1 module-scope base bonus lifts all of them above the filter: tool emits **130** `kind: docstring` records, **recall 100% (130/130)** at default settings.

### 3.6 Confirmed-working features

- YAML key-path extraction (`streams.reference.nominal_srate_hz`, `defaults[0]`, etc.) — correct and present on every active-value record.
- Hydra defaults-entry detection (`Hydra defaults entry` score reason, contributing to the 768 `protocol_schema_contract` records).
- Scoring/priority pipeline runs deterministically end-to-end and produces non-trivial differentiation (66 high / 1614 medium / 234 low).
- File-origin classification (`local` vs `test_harness`) works and correctly identifies test fixtures.
- Python scope/qualified-symbol attachment works for nested scopes (e.g. `SessionManager.manifest_dict`).
- Python Doxygen-like `# @...` comment classification works (102 records carry `doxygen_tags`).

### 3.7 Second corpus run — `LSL_Viewer`

`LSL_Viewer` differs from `Handgrip_Calibration` in one key way: its `conf/config.yaml` has real leading/inline comments (the first library's config tree had none). Used to fill the gap left by GAP-5 below.

```
files_scanned: 33  (32 .py + 1 conf/config.yaml)
files_with_records: 26
records: 280
by language:  python 243, yaml 37
by priority:  low 225, medium 48, high 7
by claim type: contract_claim 166, configuration_fact 37, runtime_default 37,
               test_intent 25, protocol_schema_contract 21,
               timing_synchronization_policy 9, allowed_values 1
by marker:     (none)
by file origin: local 242, test_harness 38
```

(v1.3 current-source run. Deltas vs the v1.1 pre-fix run: `operator_guidance 1` is gone —
that record was the GAP-7 stray, mis-split `lock_max_span` comment line, now folded back
into its parent's single `leading_comment`; python record count rises 240 → 243 as the
module-scope docstrings GAP-1 surfaces.)

Python docstring recall here: AST ground truth = 25; v1.1 emitted 22 at default settings
(88%, the same GAP-1 module-docstring shortfall as the first corpus). On the v1.3
current-source run the tool emits **25** `kind: docstring` records, **recall 100% (25/25)**
at default settings.

## 4. Gaps

Each gap below is written to be directly usable as a tracked issue.

> **Resolution status (v1.3, `local@051d851`)**: GAP-1, GAP-2, GAP-3, GAP-4, GAP-7, and
> GAP-8 are **fixed** in the source and covered by unit + acceptance tests; the per-gap
> "Resolution" notes below record what changed and how it was verified on this corpus.
> GAP-5 was already resolved in v1.1; GAP-6 remains an observation (not a bug).
>
> **v1.3 — clean milestone re-run on current source.** The v1.2 resolution numbers were
> captured mid-fix; v1.3 re-ran the full validation against `local@051d851` (release
> binary rebuilt first) to confirm every fix reproduces on `HEAD`. Each fixed gap was
> re-verified directly on this corpus, default settings unless noted:
> - **GAP-1**: all 26 module-scope docstrings now carry the `+3 docstring attached to
>   module` reason and clear the `low` filter; Python docstring recall is now **100%**
>   (`Handgrip_Calibration` 130/130, `LSL_Viewer` 25/25), up from 92.3% / 88%.
> - **GAP-2**: `by marker` is `(none)` on both corpora — the `logging_setup.py` WARNING
>   false positive (and its `risk_workaround_marker` claim) is eliminated.
> - **GAP-3**: `implementation_rationale` populated (2 records, `Handgrip_Calibration`)
>   from parsed docstring sections.
> - **GAP-4**: `LSL_Viewer` by-symbol output carries 8 dotted module keys
>   (e.g. `LSL_Viewer.tests.unit.test_alignment`).
> - **GAP-7**: `viewer.xy_correlation.lock_max_span` is a single record with both prose
>   lines in one `leading_comment`; zero stray commented-example records remain under the
>   parent key (the v1.1 `operator_guidance` record is gone).
> - **GAP-8**: zero records carry the "public symbol" reason; 104 carry the scope-neutral
>   "docstring attached to function/class/method".
>
> Re-run deltas (this corpus, default settings): `Handgrip_Calibration` records
> 1914 → 1924 (+10 module docstrings now visible, +1 file-with-records), by-marker
> `WARNING 1 → (none)`. `LSL_Viewer` records 278 → 280 with the `operator_guidance` stray
> folded back into its parent's `leading_comment`. No errors in any manifest; all five
> output-contract files present and well-formed on both runs.

---

### GAP-1 (Medium — corrected from v1.0's "High/parser bug") — Module-scope docstrings get no base score, so most are silently filtered by default settings

**v1.0 of this report claimed** this was a single-line-docstring parsing bug. **That diagnosis was wrong.** Re-investigated using the second corpus run: re-ran both libraries with `--min-priority ignore --include-low-priority` (`/tmp/inv3` for `Handgrip_Calibration`, `/tmp/inv4` for `LSL_Viewer`) to see every record regardless of priority.

**Result: every "missing" docstring is present in the JSONL.** None were dropped by the parser. Example — `src/handgrip_calibration/cli.py:1`:
```json
{"line_start": 1, "python": {"docstring_scope": "module"}, "score": 2, "priority": "ignore", "score_reasons": ["domain term: calibration"]}
```
It's filtered by the default `min_priority = "low"` (set in `.uncommentrc.toml`) because its score (2) is below the low-tier threshold (3) — not because extraction failed.

**Root cause, conclusively isolated**: across 26 module-scope docstring records inspected (10 in `Handgrip_Calibration`, plus the rest), **zero** carry the `"docstring attached to public symbol"` score reason. That +3 base bonus (`docs/gap-analysis-current-vs-mvp.md` §7) is applied to function/class/method docstrings but never to module-scope ones — confirmed by checking every function/method docstring sampled, which all do carry that reason. A module docstring's score is therefore built *entirely* from optional semantic/domain-term bonuses; a plain one-line "what this file does" description with no matching trigger word scores 0–2 and disappears under default settings, **regardless of line count** — this holds for both single-line and multi-line module docstrings (`_utils.py` and `lsl_io.py`, both multi-line, score 2 each).

Evidence table (`Handgrip_Calibration`, via `--min-priority ignore`):

```
cli.py            score 2   reasons: [domain term: calibration]            -> ignore
session.py        score 0   reasons: []                                    -> ignore
synthetic.py      score 2   reasons: [domain term: calibration]            -> ignore
_utils.py         score 2   reasons: [domain term: calibration]            -> ignore   (multi-line)
report.py         score 2   reasons: [domain term: calibration]            -> ignore
segmentation.py   score 2   reasons: [domain term: calibration]            -> ignore
quality.py        score 2   reasons: [domain term: quality]                -> ignore
lsl_io.py         score 2   reasons: [domain term: stream, CSV]            -> ignore   (multi-line)
__main__.py       score 2   reasons: [domain term: calibration]            -> ignore
conftest.py       score 2   reasons: [domain term: calibration]            -> ignore   (multi-line)
analyze_step_relaxation.py  score 9  reasons: [...default/optional..., units/rate/range/path/field, domain terms]  -> medium  (survives — has trigger words)
```

Same pattern in `LSL_Viewer` (`--min-priority ignore`): `tests/e2e/test_cli.py` score 2, `tests/integration/test_charts.py` score 0, `tests/integration/test_csv_replay.py` score 2 — all `ignore`, all the docstrings the first corpus run's recall measurement (92.3%) and this corpus's (88%, 22/25) silently lost.

**This reframes the fix target completely**: `src/inventory/python.rs` docstring detection is fine — the gap is in the scoring rules (likely `src/inventory/scoring.rs` or wherever the public-symbol-attachment bonus is computed), which has no module-scope case. Practical impact is unchanged (~77% of module docstrings invisible by default in this corpus) but the correct fix is adding a module-scope base bonus (e.g. "+3 docstring attached to module"), not touching the parser.

**Suggested fix location**: wherever `"docstring attached to public symbol"` is computed (scoring logic, not `python.rs`'s extraction path) — add an equivalent base bonus for `docstring_scope == "module"`. Add a regression test asserting a plain one-line module docstring with no domain/semantic trigger words still scores ≥3 (low tier) by default.

**Severity downgraded from High to Medium**: the user-visible symptom (docstrings missing from default output) is real and still worth fixing, but it's a tunable scoring-weight gap, not data loss from a broken parser — `--include-low-priority` already provides a complete workaround today.

**Resolution (v1.2)**: Added `AttachedKind::Module` (`src/inventory/scoring.rs`) with a +3 base bonus and reason `"docstring attached to module"`; `src/inventory/python.rs` maps module-scope docstrings to it. `docs/gap-analysis-current-vs-mvp.md` §7 now lists the module bonus. Verified: `Handgrip_Calibration` python records 281 → 291 at default settings (the previously-filtered module docstrings now surface). Unit test `module_docstring_gets_base_bonus`; acceptance test `module_docstring_scored_above_ignore_by_default`.

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

**Resolution (v1.2)**: `src/inventory/text.rs` `markers()` now strips backtick-delimited inline-code spans (Markdown rule: an opening run of N backticks closes on the next run of exactly N, covering reST `` ``…`` `` and single `` `…` ``) before scanning. Verified: the `Handgrip_Calibration` by-marker count went from `WARNING 1` to `(none)` — the `logging_setup.py` false positive is gone. Unit test `markers_ignores_code_span_enumerations`; acceptance test `marker_not_flagged_inside_code_span`.

---

### GAP-3 (Low) — `docstring_sections` (structured NumPy/Google docstring parsing) is unimplemented

**Evidence**: 0/120 docstring records populate the optional `python.docstring_sections` field, even where the raw text clearly contains parseable `Parameters` / `Returns` sections (e.g. `configure_logging`'s docstring in `logging_setup.py`).

**Status**: this matches `docs/gap-analysis-current-vs-mvp.md` Phase 5's "when easy" framing — it's explicitly optional for MVP, not a contract violation. Listed here as a ready next-iteration enhancement since real source data already exists to validate against.

**Resolution (v1.2)**: `parse_sections` (`src/inventory/python.rs`) now parses the line-structured raw docstring instead of the whitespace-collapsed normalized text, and recognizes NumPy underline headers (`Parameters` over `----`) and Google block headers (`Args:` on its own line) in addition to the previous reST/inline forms. `implementation_rationale` (driven by populated sections) now appears in the `Handgrip_Calibration` claim census. Unit tests `numpy_underline_sections_parsed` / `google_block_sections_parsed`; acceptance test `numpy_docstring_sections_populated`.

---

### GAP-4 (Low) — `by_symbol` fallback naming for module docstrings is weak

**Evidence**: `comments_docstrings_by_symbol.json` entries for module-level docstrings use `path:line` as the `symbol` key (e.g. `./Handgrip_Calibration/scripts/analyze_step_relaxation.py:2`) rather than a module-name-derived symbol (e.g. `handgrip_calibration.analyze_step_relaxation` or similar).

**Impact**: minor — reduces the by-symbol lookup's usefulness specifically for module-level claims; doesn't affect JSONL/Markdown correctness.

**Resolution (v1.2)**: `symbol_key` (`src/inventory/writers/by_symbol.rs`) now derives a dotted module symbol from the path (drop leading `./` and `src/`, strip `.py`) for module-scope Python docstrings before falling back to `path:line`. Verified on `LSL_Viewer`: module docstrings now key under e.g. `LSL_Viewer.tests.unit.test_alignment`. Unit tests `module_docstring_uses_dotted_path_symbol` / `module_symbol_drops_src_segment`; acceptance test `by_symbol_uses_module_derived_symbol`.

---

### GAP-5 (Resolved — confirmed working, was "untested" in v1.0) — YAML comment-attachment features validated against AC-006

**v1.0 finding**: all 13 files under `Handgrip_Calibration/conf/` have zero `#` comments, so this feature was untestable there.

**v1.1 update**: `LSL_Viewer/conf/config.yaml` has real comments, and the feature works as specified. Exact AC-006 match:
```yaml
mode: manual                  # raw_lsl | tail_aligned_lsl | manual
```
produces:
```json
{
  "yaml": {
    "key_path": "viewer.xy_correlation.time_alignment.mode",
    "value": "manual",
    "inline_comment": "raw_lsl | tail_aligned_lsl | manual",
    "allowed_values_hint": ["raw_lsl", "tail_aligned_lsl", "manual"],
    "attachment_confidence": "high"
  },
  "priority": "high",
  "score": 11,
  "claim_types": ["configuration_fact", "allowed_values", "timing_synchronization_policy", "runtime_default"]
}
```
Multi-line leading-comment blocks also join correctly in the general case: an 11-line block (lines 76–86) attaches verbatim as one `leading_comment` string to `manual_reference_shift_s` (line 87); a 2-line block (lines 63–64) joins correctly for `target_signal` (line 65).

**Status**: AC-006 and basic multi-line leading-comment attachment confirmed working on real data. (A related but distinct failure mode in multi-line block handling was found alongside this — see GAP-7.)

---

### GAP-6 (Observation, not a code bug — narrowed in v1.1) — Priority distribution skew is YAML-heavy-corpus-specific, not universal

**v1.0 finding**: 1614/1914 records (84%) in `Handgrip_Calibration` are `medium` priority, driven by the flat `+2 configuration_fact` baseline applied to essentially every active YAML value. Only 66 records (3.4%) reach `high`.

**v1.1 update — contrasting corpus**: `LSL_Viewer` is python-heavy (240/278 = 86% python records, vs. `Handgrip_Calibration`'s 85% yaml) and shows the **opposite** skew: low 228 (82%), medium 43 (15%), high 7 (2.5%). Python's ordinary-inline-comment baseline (`+1`) lands most records in `low`; YAML's active-value baseline (`+2`) lands most records in `medium`. Python breakdown: 218 low / 22 medium, claim types `contract_claim` 166, `test_intent` 22, `protocol_schema_contract` 8.

**Narrowed conclusion**: the "84% medium glut" is a property of **YAML-heavy repos specifically** (where nearly every active value gets the same flat baseline), not a general MVP scoring flaw. Python-dominant repos triage usefully out of the box — `--min-priority medium` already thins 82% of records away in `LSL_Viewer`. The original concern about triage workflows still applies, but only to YAML-config-dominant repositories — worth keeping in mind when sizing agent workflows, not treating as a blanket MVP limitation.

---

### GAP-7 (Medium, new) — Multi-line YAML leading-comment block gets split when an interior line looks like a commented-out option

**Evidence**: in `LSL_Viewer/conf/config.yaml`:
```yaml
  xy_correlation:
    # false = adaptive autoscale on every refresh.        (line 54)
    # true  = only zoom out: preserve the largest ...      (line 55)
    lock_max_span: false                                   (line 56)
```
Line 54 correctly becomes `leading_comment` for `lock_max_span` (`"false = adaptive autoscale on every refresh."`). Line 55 does **not** join it — it's instead emitted as a *separate*, misattached record:
```json
{
  "id": "comment:./LSL_Viewer/conf/config.yaml:55:55:db3403a4",
  "kind": "comment",
  "raw_text": "    # true  = only zoom out: preserve the largest observed XY axis span.",
  "claim_types": ["operator_guidance"],
  "yaml": {"key_path": "viewer.xy_correlation", "is_commented_example": true, "attachment_confidence": "medium"}
}
```
attached to the **parent** key path `viewer.xy_correlation` instead of `lock_max_span` — both lines describe the same key, but the second is split off and reclassified as an unrelated "commented-out example."

By contrast, other multi-line blocks in the same file join correctly (see GAP-5: an 11-line block and a separate 2-line block both attach as single `leading_comment` strings without being split).

**Root cause hypothesis**: a heuristic that detects "looks like a commented-out true/false or enumerated-option line" pulls matching lines out of a contiguous leading-comment block and reclassifies them as standalone commented-out examples — this fires correctly when the *entire* block is a list of alternatives, but incorrectly here where the block is prose describing one key's two possible states.

**Suggested fix location**: the YAML leading-comment block-joining logic (`src/inventory/yaml.rs`) — the commented-out-example detector should not split an interior line out of an otherwise-contiguous comment block attached to the same key; it should either classify the whole block as one unit or only apply the split when the block boundary (blank line, different indentation, or a key/value line) actually separates them.

**Impact**: data loss/misattachment — the `lock_max_span` claim is split across two records, one of which is filed under the wrong key path, reducing retrieval precision for exactly this kind of two-state documentation comment (a common pattern in this codebase's config).

**Resolution (v1.2)**: `handle_comment_line` (`src/inventory/yaml.rs`) now skips the commented-out-example branch while a leading-comment block is already accumulating (`pending_leading` non-empty), so a contiguous block joins as one unit. Verified on `LSL_Viewer`: `viewer.xy_correlation.lock_max_span` now carries both prose lines in a single `leading_comment` with zero stray commented-example records. Unit test `multiline_leading_block_not_split_by_interior_colon`.

---

### GAP-8 (Low, cosmetic, new) — Score reason says "public symbol" even for private (`_`-prefixed) functions

**Evidence**: private function/method docstrings receive the same `"docstring attached to public symbol"` score reason and +3 bonus as genuinely public ones, e.g.:
```json
{"scope_name": "_add_common_flags", "score": 3, "score_reasons": ["docstring attached to public symbol"]}
```
Not score-incorrect under the MVP's documented scoring rules (`docs/gap-analysis-current-vs-mvp.md` §7 doesn't distinguish public/private in its base-bonus rule), but the reason string is misleading since `_add_common_flags` is conventionally private.

**Suggested fix location**: wherever the reason string is generated alongside the +3 base bonus — reword to a scope-neutral phrase (e.g. `"docstring attached to function/class/method"`), or implement an actual public/private distinction if that's intended scoring behavior. Severity is low — purely a labeling clarity issue, no score/priority impact.

**Resolution (v1.2)**: the function/class/method docstring reason in `src/inventory/scoring.rs` is now `"docstring attached to function/class/method"` (no score change). Unit test `function_docstring_reason_is_scope_neutral` asserts the string no longer claims "public symbol".

## 5. Conclusion

The MVP substantially matches the PRD and `output-contract.md` for real mixed Python+YAML repositories: the full pipeline (discovery → tree-sitter extraction → scoring → multi-format export) runs end-to-end, deterministically, without touching source files, and config-driven runs are structurally equivalent to CLI-flag runs. This holds across two corpora with materially different shapes (`Handgrip_Calibration`: YAML-heavy, no YAML comments; `LSL_Viewer`: Python-heavy, richly-commented YAML).

The second corpus run **corrected** the first report's most serious finding: GAP-1 is not a parser bug (extraction is sound for module docstrings of any line count) but a **scoring** gap — module-scope docstrings never receive the base "attached to symbol" bonus that function/class/method docstrings get, so most fall to `ignore` tier and are filtered by default `min_priority=low`. It also **confirmed** GAP-5 as working (AC-006 allowed-values-hint extraction validated exactly) and surfaced two new findings:

- **GAP-1** (medium severity, corrected from "high/parser bug"): module docstrings systematically under-scored, mostly invisible by default — **fixed** (module-scope +3 base bonus in scoring rules).
- **GAP-2** (medium severity): marker detection lacks context guards, producing a false-positive high-priority record — **fixed** (code-span stripping before marker scan).
- **GAP-7** (medium severity, new): multi-line YAML leading-comment blocks can be incorrectly split/misattached when an interior line resembles a commented-out option — **fixed** (contiguous block kept whole).
- **GAP-8** (low severity, new): cosmetic score-reason mislabeling for private functions — **fixed** (scope-neutral reason string).

All six actionable gaps (GAP-1/2/3/4/7/8) are **fixed** and covered by unit + acceptance tests; GAP-3 (NumPy/Google docstring section parsing) and GAP-4 (by-symbol module naming) were closed alongside the medium-severity fixes. GAP-5 was confirmed working in v1.1; GAP-6 remains a narrowed observation (priority skew is YAML-heavy-corpus-specific, not universal) rather than a general limitation. **v1.3 re-ran the full validation against `local@051d851` (release binary rebuilt first) and confirms every fix reproduces cleanly on `HEAD`** — Python docstring recall is now 100% on both corpora, marker false positives are gone, and no manifest reports errors.
