# Language Extraction Rules

This document defines language-specific extraction behavior for the ideal tool and the first MVP.

---

## 1. Python extraction

### 1.1 Parser strategy

Use tree-sitter for comment/docstring node discovery. Reuse the existing Python handler logic that distinguishes real docstrings from ordinary string literals.

For richer symbol context, optionally supplement with Python AST parsing in a later phase.

### 1.2 Extract

```text
module docstrings
class docstrings
function docstrings
method docstrings
inline and block `#` comments
leading comments attached to symbols
comments inside functions when they contain high-value markers
Doxygen-like Python comments: # @param, # @return, # @brief, # @note, # @warning, # @deprecated
test docstrings
```

### 1.3 Do not extract by default

```text
raise/assert strings
logging messages
ordinary string literals
comment-like text inside strings
obvious section separators
low-value inline comments unless requested
```

### 1.4 Structured docstring parsing

Parse docstring sections when possible:

```text
Args
Arguments
Parameters
Returns
Yields
Raises
Notes
Warnings
Examples
Deprecated
See Also
Dependency chain
Test Strategy
Regression
```

The record may include:

```json
{
  "docstring_sections": {
    "Parameters": "...",
    "Returns": "...",
    "Warnings": "..."
  }
}
```

### 1.5 Scope attachment

Attach each record to:

```text
module
class
function
method
nearest enclosing function
nearest following symbol
nearest previous symbol
```

### 1.6 Classification priorities

Promote to high priority when text mentions:

```text
default
optional
not used
schema
contract
CLI
stream
timestamp
alignment
calibration
output path
protocol
regression
bug
must
should
```

### 1.7 Test docstrings

Do not ignore test docstrings. In the target repository style, tests often encode high-authority expected behavior.

High-priority markers:

```text
REGRESSION TEST
Test Strategy
System MUST
FR-
US
Bug #
schema
contract
```

---

## 2. C/C++ extraction

### 2.1 Parser strategy

Use tree-sitter C/C++ comment nodes as the primary extraction source.

### 2.2 File-origin classification

Before scoring comments, classify file origin:

| File class               | Default behavior                             |
| ------------------------ | -------------------------------------------- |
| `local_application`      | full extraction                              |
| `local_driver`           | full extraction                              |
| `local_validation_model` | full extraction, high priority               |
| `test_harness`           | full extraction                              |
| `generated_st_cube`      | extract USER CODE and high markers only      |
| `generated_ai_network`   | extract selected model/build/memory metadata |
| `vendor_hal_cmsis`       | ignore unless explicitly requested           |
| `third_party_middleware` | ignore unless high marker or local patch     |

Suggested path heuristics:

```text
Drivers/CMSIS/**                 -> vendor_hal_cmsis
Middlewares/**                   -> third_party_middleware
**/Core/Src/**, **/Core/Inc/**   -> local_application
**/Drivers/** local project dirs -> local_driver
**/network.c, **/network_data.*  -> generated_ai_network
**/frame_validate.*              -> local_validation_model
```

### 2.3 Extract

```text
// line comments
/* ordinary block comments */
/** Doxygen block comments */
/*! Doxygen block comments */
/// Doxygen line comments
//! Doxygen line comments
inline field comments: /**< ... */ and ///< ...
comments around macros
comments around static consts
comments around typedefs
comments around structs/enums
comments around functions
comments inside USER CODE regions
```

### 2.4 Doxygen target attachment

Attach Doxygen comments to the nearest target declaration:

```text
#define
static const
typedef enum
typedef struct
struct field
enum value
function declaration
function definition
```

Promote comments on constants/macros when they mention:

```text
RAW10
Bayer
BGGR
PN9
PRBS
LFSR
seed
frame
pixel
DMA
cache
NPU
RISAF
IAC
XSPI
ThreadX
timestamp
buffer
address
stride
width
height
```

Claim types:

```text
constant_contract
model_parameter
hardware_assumption
protocol_constant
memory_layout_claim
sensor_model_claim
validation_assumption
```

### 2.5 USER CODE regions

Treat STM32/Cube markers as region metadata, not claims by default:

```c
/* USER CODE BEGIN Includes */
/* USER CODE END Includes */
```

A comment inside that region should include:

```json
{
  "inside_user_code_region": true,
  "user_code_region": "Includes"
}
```

Increase priority for comments inside USER CODE regions.

### 2.6 Preprocessor diagnostics

Extract `#warning` and `#error` as `preprocessor_diagnostic` evidence.

Claim types:

```text
build_assumption
version_mismatch_risk
platform_constraint
configuration_error
```

### 2.7 Commented-out code

Default behavior:

```text
commented-out code -> ignore
commented-out code with adjacent rationale -> extract
commented-out include/define touching NPU/cache/DMA/peripheral -> medium priority
commented-out code inside USER CODE -> medium priority
```

### 2.8 Suppress

```text
license banners
generic HAL/CMSIS boilerplate
USER CODE markers alone
vendor middleware comments
generated network.c bulk comments except selected metadata
commented-out code without rationale
```

---

## 3. YAML / Hydra extraction

### 3.1 Scope

Extract from:

```text
*.yaml
*.yml
conf/**/*.yaml
config/**/*.yaml
*/conf/**/*.yaml
*/config/**/*.yaml
```

Hydra-style layouts:

```text
conf/config.yaml
conf/<group>/<name>.yaml
config/config.yaml
```

Recognize:

```yaml
defaults:
  - group: option
  - _self_

# @package <package>

hydra:
  run:
    dir: ...
  output_subdir: null
  job:
    chdir: false
```

### 3.2 Parser requirement

For the ideal tool, use a comment-preserving YAML strategy. In Rust, this may require either:

1. tree-sitter for comment positions plus a YAML parser for values, or
2. a comment-preserving YAML parser if adopted later.

For the MVP, a hybrid approach is acceptable:

```text
tree-sitter-yaml comments -> comment records
YAML parser or deterministic line walk -> active key/value facts and key paths
line proximity rules -> attach comments to keys
```

### 3.3 Extract

```text
file-level comments
leading comments above keys
inline comments after values
comments inside lists
Hydra # @package markers
commented-out examples
commented-out selectable options
active key/value facts
Hydra defaults entries
Hydra/OmegaConf interpolations
```

### 3.4 Key-path attachment

Each YAML record must include a key path:

```text
active_send.timestamp_policy
active_send.delivery_window_s
viewer.xy.alignment.mode
streams.reference.nominal_srate_hz
fit.selection.primary_metric
logging.file
```

List notation:

```text
defaults[0]
filters[3].cutoff_hz
protocol.holds.levels_N[4]
session.copy_component_configs[1]
```

Identity-aware list notation when possible:

```text
filters[name=butter_lowpass_8hz].cutoff_hz
dynamic.ramps[label=slow].speed_N_per_s
fit.candidate_models[]
```

### 3.5 Comment attachment rules

1. Inline comment after a value attaches to that exact key.
2. Consecutive leading comments directly above a key attach to that key.
3. A leading comment block followed by a mapping attaches to the mapping root.
4. A section banner followed by a mapping attaches to the section root unless immediately followed by a specific key.
5. `# @package ...` attaches to file-level Hydra metadata.
6. Commented-out list examples attach to the nearest key or list parent.
7. Empty comment lines inside a contiguous block preserve paragraph boundaries.
8. Comments separated from a key by more than one blank line become file/section-level comments.

### 3.6 YAML claim types

```text
runtime_default
allowed_values
operational_safety
non_interference
timing_synchronization_policy
source_of_truth
lifecycle_status
performance_backpressure_buffering
protocol_schema_contract
calibration_analysis_model_policy
operator_guidance
runtime_interpolation
output_path
```

### 3.7 Value extraction as claims

Extract active values even without comments. Many configuration files contain critical truth with no adjacent comment.

High-value key patterns:

```text
stream
schema
topic
timestamp
alignment
ref_shift
sample_rate
nominal_srate
frequency
protocol
quality
fit
filter
candidate
output
log
CSV
XDF
IPC
ZMQ
parser
register
active_send
modbus
calibration
```

### 3.8 Interpolation extraction

Given:

```yaml
file: ${hydra:run.dir}/run.log
```

Emit:

```json
{
  "key_path": "logging.file",
  "value": "${hydra:run.dir}/run.log",
  "interpolations": ["hydra:run.dir"],
  "claim_types": ["output_path", "runtime_interpolation"]
}
```

### 3.9 YAML scoring additions

```text
Hydra defaults entry                              +4
Hydra @package marker                             +3
comment attached to active key                    +3
inline allowed-values list                        +4
config value under stream/protocol/schema/ipc     +5
config value under timestamp/alignment section    +5
config value under quality/fit/calibration        +4
comment says display-only / not acquisition truth +5
comment says authoritative / source of truth      +5
comment says legacy / deprecated / no effect      +5
comment mentions measured session/date            +4
comment mentions 500 Hz / 2 ms / latency/drift    +4
commented-out operator example                    +2
section separator only                            -4
Hydra boilerplate with no comment                 +1
```
