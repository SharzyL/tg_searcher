import traceback
import logging


def get_logger(level=logging.DEBUG):
    _logger = logging.getLogger(__name__)
    _logger.setLevel(level)
    return _logger
