"""Allow running as: python -m as3_decompiler"""
import logging

log = logging.getLogger(__name__)

from .cli import main

if __name__ == '__main__':
    main()
