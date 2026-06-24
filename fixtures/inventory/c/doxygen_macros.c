#include <stdint.h>

/** Number of LFSR clocks between consecutive image rows. */
#define FRAME_VALIDATE_ROW_STRIDE_BITS (1560U)

/** Expected RAW10 value for a fully dark test-pattern sample. */
static const uint32_t FRAME_VALIDATE_DARK_RAW10 = 64U;

/// Validate one noise pattern frame against the model.
bool frame_validate_noise_pattern(const uint8_t *frame, uint32_t length);
