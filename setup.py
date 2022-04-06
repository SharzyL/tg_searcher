from setuptools import setup, find_packages

import pathlib
here = pathlib.Path(__file__).parent.resolve()
long_description = (here / 'README.md').read_text(encoding='utf-8')

__version__ = '0.3.1'

setup(
    name='tg-searcher',
    version=__version__,
    packages=find_packages(),
    description='Telegram searcher bot for Chinese',
    long_description=long_description,
    long_description_content_type="text/markdown",
    include_package_data=True,
    author='Sharzy L',
    author_email='me@sharzy.in',
    url='https://github.com/SharzyL/tg_searcher',
    license='MIT',
    python_requires='>=3.8',
    install_requires=[
        'telethon~=1.24.0',
        'cryptg',
        'whoosh~=2.7.4',
        'python-socks[asyncio]',
        'jieba',
        'pyyaml',
        'redis',
    ],
    classifiers=[
        "Development Status :: 3 - Alpha",
        "License :: OSI Approved :: MIT License",
        "Intended Audience :: Developers",
        "Intended Audience :: End Users/Desktop",
        "Programming Language :: Python :: 3 :: Only",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Topic :: Communications :: Chat",
        "Topic :: Utilities"
    ],
    entry_points={
        'console_scripts': ['tg-searcher=tg_searcher:main']
    }
)

