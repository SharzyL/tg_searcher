import traceback
import logging


def get_logger(level=logging.DEBUG):
    _logger = logging.getLogger(__name__)
    _logger.setLevel(level)
    return _logger


def log_exception(logger):
    def wrapper(func):
        async def wrap(*args, **kwargs):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                logger.error(traceback.format_exc())
                raise
        return wrap
    return wrapper
