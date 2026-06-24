#pragma once
#include <cstdint>

/// Pixel match statistics for one validated frame.
struct MatchStats {
    uint32_t exact;       /**< Pixels matching the model exactly. */
    uint32_t close;       ///< Pixels within tolerance of the model.
    uint32_t mismatched;  /**< Pixels failing the validation contract. */
};
