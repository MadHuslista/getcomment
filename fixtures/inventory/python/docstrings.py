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


def configure_logging(level="INFO"):
    """Configure the package-root logger.

    Parameters
    ----------
    level:
        Console verbosity.  One of ``DEBUG``, ``INFO``, ``WARNING``,
        ``ERROR``, ``CRITICAL``.  Case-insensitive.
    """
    return level
