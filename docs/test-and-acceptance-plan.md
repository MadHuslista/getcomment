# Test and Acceptance Plan

This document defines test fixtures and acceptance checks for the comment inventory MVP.

---

## 1. Test fixture layout

```text
fixtures/inventory/
  python/
    docstrings.py
    doxygen_like.py
    test_intent.py
  c/
    doxygen_macros.c
    user_code_regions.c
    preprocessor_diagnostics.c
  cpp/
    inline_field_comments.hpp
  yaml/
    hydra_config.yaml
    comments_and_values.yaml
  mixed/
    src/example.py
    firmware/frame_validate.c
    conf/config.yaml
```

---

## 2. Generic CLI tests

### Test: inventory does not mutate source

1. Copy fixture directory to temp dir.
2. Hash all source files.
3. Run:

```bash
uncomment inventory tempdir --output-dir tempdir/.agent/inventory
```

4. Hash source files again.
5. Assert hashes match.

### Test: JSONL validity

1. Run inventory.
2. Parse every JSONL line.
3. Assert required fields exist.

### Test: deterministic output

1. Run inventory twice.
2. Compare JSONL byte-for-byte after normalizing manifest timestamp if necessary.

---

## 3. Python tests

### Fixture

```python
"""Module contract."""

class DriftCorrector:
    """Adaptive baseline drift corrector.

    .. note:: Not used in the default configuration.
    """

    def apply(self, sample):
        """Return corrected sample."""
        text = """not a docstring"""
        return sample

## @param level Logging level string.
# TODO: validate timestamp alignment.
```

### Assertions

```text
module docstring emitted
class docstring emitted, high priority
method docstring emitted
assigned triple-quoted string not emitted as docstring
@param emitted as python_doxygen_comment
TODO emitted as high priority
```

---

## 4. C tests

### Fixture

```c
/* USER CODE BEGIN Includes */
// TODO: validate DMA cache policy
/* USER CODE END Includes */

/** Number of LFSR clocks between consecutive image rows. */
#define FRAME_VALIDATE_ROW_STRIDE_BITS (1560U)

uint32_t exact; /**< Pixels matching the model exactly. */

#warning "Possible mismatch in ll_aton library used"
```

### Assertions

```text
TODO record has inside_user_code_region=true
TODO record user_code_region=Includes
USER CODE markers not emitted as claim records by default
Doxygen macro comment attached to FRAME_VALIDATE_ROW_STRIDE_BITS
Doxygen macro comment priority=high
inline field comment attached to exact
#warning emitted as preprocessor_diagnostic priority=high
```

---

## 5. YAML/Hydra tests

### Fixture

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

### Assertions

```text
hydra package emitted: bridge
defaults[0] fact emitted
defaults[1] fact emitted
timestamp_policy fact emitted
timestamp_policy leading comment attached
mode fact emitted
mode allowed_values_hint = [modbus_rtu, active_send]
commented example /dev/ttyUSB0 emitted under excluded_ports
logging.file interpolation hydra:run.dir extracted
```

---

## 6. Summary tests

Assert summary contains:

```text
total records
records by language
records by priority
records by claim type
top high-priority files
suppressed/down-ranked generated records count
```

---

## 7. Existing removal regression

All existing tests must still pass:

```bash
cargo test
```

Inventory mode must not alter:

```text
uncomment <paths>
uncomment --dry-run <paths>
uncomment --diff --dry-run <paths>
uncomment init
```

---

## 8. Manual review checklist

Before declaring MVP complete, inspect the Markdown output manually and verify:

```text
high-priority records are actually useful
generated/vendor noise is not dominant
YAML key paths are readable
C/C++ macro comments are attached to useful names
Python docstrings are attached to useful symbols
score reasons are explainable
```
