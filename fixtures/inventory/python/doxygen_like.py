"""Doxygen-like Python comment fixture."""


def configure(level, timeout):
    # @brief Configure the logging subsystem.
    # @param level Logging level string.
    # @param timeout Seconds before the operation aborts.
    # @return True when configuration succeeded.
    # @warning Must be called before any worker thread starts.
    # @deprecated Use configure_v2 instead.
    return True
