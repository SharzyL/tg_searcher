import logging
import traceback


def get_logger(_log_path: str, level=logging.INFO):
    log_fmt = logging.Formatter(
        f"%(asctime)s - %(levelname)s: %(message)s",
        "%Y %b %d %H:%M:%S"
    )
    _logger = logging.getLogger()
    _logger.setLevel(level)
    fh = logging.FileHandler(f'{_log_path}', encoding='utf8')
    fh.setLevel(level)
    fh.setFormatter(log_fmt)
    _logger.addHandler(fh)
    return _logger


def log_func(logger):
    def wrapper(func):
        async def wrap(*args, **kwargs):
            try:
                return await func(*args, **kwargs)
            except Exception as e:
                logger.error(traceback.format_exc())
                raise
        return wrap
    return wrapper
