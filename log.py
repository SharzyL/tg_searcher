import traceback
import logging


def get_logger(_log_path=None, level=logging.DEBUG):
    log_fmt = logging.Formatter(
        "%(asctime)s - %(levelname)s - %(filename)s | %(funcName)s: %(message)s",
        "%Y %b %d %H:%M:%S"
    )
    _logger = logging.getLogger(__name__)
    fh = logging.FileHandler(f'{_log_path}', encoding='utf8') if _log_path else logging.StreamHandler()
    fh.setLevel(level)
    fh.setFormatter(log_fmt)
    _logger.addHandler(fh)
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
