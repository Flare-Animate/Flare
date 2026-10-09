"""
as3_decompiler — Full AS3 decompiler package.

Reads SWF files, parses DoABC/DoABC2 tags, and decompiles all classes
back into ActionScript 3 source code.

Requires Python >= 3.8 (uses walrus operator := and dict insertion-order guarantee).

Usage:
    python -m as3_decompiler input.swf [--outdir output_dir]
    python -m as3_decompiler input.swf --list          # list classes only
    python -m as3_decompiler input.swf --class MyClass  # decompile one class
    python -m as3_decompiler input.swf --all            # decompile everything

Package structure:
    swf_reader.py          - SWF file reading and ABC block extraction
    abc_parser.py          - Binary readers, AVM2 constants, dataclasses, ABCFile parser
    opcodes.py             - AVM2 opcode constants and inc/dec pattern matching
    helpers.py             - Formatting, type-casting, and utility functions
    method_decompiler.py   - Single-method bytecode decompiler (stack simulation + control flow)
    class_decompiler.py    - Full class/interface decompiler with import resolution
    cli.py                 - Command-line interface
"""

import logging

log = logging.getLogger(__name__)

# Re-export everything for backward compatibility with the old monolithic module.
# External code using `from as3_decompiler import read_swf, ABCFile, ...` will
# continue to work unchanged.

from .swf_reader import *       # noqa: F401,F403
from .abc_parser import *       # noqa: F401,F403
from .opcodes import *          # noqa: F401,F403
from .helpers import *          # noqa: F401,F403
from .method_decompiler import *  # noqa: F401,F403
from .class_decompiler import *   # noqa: F401,F403
from .cli import main           # noqa: F401
from .abc_editor import *       # noqa: F401,F403

# Build package-level __all__ from all submodule exports
from . import swf_reader as _sw, abc_parser as _ap, opcodes as _op, helpers as _hp, method_decompiler as _md, class_decompiler as _cd, abc_editor as _ed
__all__ = list(getattr(_sw, '__all__', [])) + list(getattr(_ap, '__all__', [])) + \
          list(getattr(_op, '__all__', [])) + list(getattr(_hp, '__all__', [])) + \
          list(getattr(_md, '__all__', [])) + list(getattr(_cd, '__all__', [])) + \
          list(getattr(_ed, '__all__', [])) + \
          ['main']
del _sw, _ap, _op, _hp, _md, _cd, _ed

