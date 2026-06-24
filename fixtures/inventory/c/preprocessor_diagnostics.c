#include <stdint.h>

#warning "Possible mismatch in ll_aton library used"

#if !defined(NPU_CACHE_ENABLED)
#error "NPU cache configuration is required for this build"
#endif

uint32_t exact; /**< Pixels matching the model exactly. */
