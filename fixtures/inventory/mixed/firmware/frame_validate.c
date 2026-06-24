#include <stdint.h>

/** Number of LFSR clocks between consecutive image rows. */
#define FRAME_VALIDATE_ROW_STRIDE_BITS (1560U)

/* USER CODE BEGIN Includes */
// TODO: validate DMA cache policy
/* USER CODE END Includes */

uint32_t exact; /**< Pixels matching the model exactly. */

#warning "Possible mismatch in ll_aton library used"
