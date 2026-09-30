Vendored from Next2Flash
========================

Source
------
  https://github.com/SSF2-Mods-Official/Next2Flash
  branch/tag: main @ da21d6a1 ("Release v0.1.1", 2026-05-28)
  path in upstream: app/as3_decompiler/
  licence: MIT - see ./LICENSE, which is byte-identical to upstream's root
           LICENSE file (blob c96865912a909671552b0e0191853da6410f6bb0)

What this is
------------
Next2Flash's AVM2 / ABC toolchain: an AVM2 bytecode parser, a method and class
decompiler, an ABC editor, and SWF read/patch primitives. Flare uses it as an
*optional* sidecar behind tools/flash/next2flash/flare_as3_bridge.py; the C++
side is flare/sources/common/flash/As3Bridge.{h,cpp}. Flare's own C++ readers
handle FLA/XFL/SWF bitmaps and timelines, so nothing here is required to import
a Flash file - when the helper is absent, AS3 decompilation is simply skipped.

Public API of the vendored package
----------------------------------
  swf_reader        read_swf, iter_tags, read_abc_blocks, extract_abc_blocks
  abc_parser        AVM2 ABC binary parser, ~65 exported names
  opcodes           167 AVM2 opcode constants
  helpers           decompiler formatting helpers
  method_decompiler MethodDecompiler
  class_decompiler  AS3Decompiler
  abc_editor        ABCEditor, Assembler, disassemble
  abc_patcher       serialize_abc, transplant_class, extract_method_texts
  swf_patcher       read_swf_full, write_swf_from_tags, recompile_class(...)
  cli               argparse entry point (python -m as3_decompiler)

Local modifications
-------------------
The vendored copy is *not* byte-identical to upstream: it carries Flare's own
lint pass, which removed only unused code. Audit before re-syncing; the changes
are recorded here so a future re-vendor is a three-way merge rather than a blind
copy.

  abc_editor.py     -4/+24  dropped unused imports (math, re-imported
                             InstanceInfo/ClassInfo/ScriptInfo, the MN_RTQName*
                             and MN_Multiname* tables, INSTANCE_ProtectedNs,
                             _wd64, _U30_POOL_DESCS, the CONSTANT_* block,
                             _BRANCH_OPCODES) and a dead `end_pos = pos`.
  abc_parser.py      -1/+1   `p = self._parse_method_bodies(...)` ->
                             the discarded assignment removed.
  class_decompiler.py +7/-2  added `__all__ = ['AS3Decompiler']`. Upstream had
                             none, so `from .class_decompiler import *` leaked
                             everything it had star-imported into the package
                             namespace.
  helpers.py         -9/+1   removed a duplicate dead stub of
                             `_find_op_outside_parens` (upstream defines it
                             twice: an empty stub at line 309 and the real
                             implementation at 344; the real one is what
                             method_decompiler.py uses), unused logging and re
                             imports, plus two explanatory comments.
  swf_patcher.py     -7/+5   removed unused top-level io and re (re is
                             re-imported function-locally at line 852 and used
                             at 876), and dropped patch_swf_tags and
                             extract_method_texts from __all__.
  swf_reader.py      -4/+3   removed unused sys and Optional imports and two
                             comment-only `pass` annotations.

Six of the twelve files are byte-identical to upstream main @ da21d6a1:
__init__.py, __main__.py, abc_patcher.py, cli.py, method_decompiler.py and
opcodes.py (plus LICENSE).

Re-vendoring
------------
  1. git clone --filter=blob:none https://github.com/SSF2-Mods-Official/Next2Flash
  2. git -C Next2Flash checkout da21d6a1
  3. diff -r Next2Flash/app/as3_decompiler <this directory>
  4. Apply the "Local modifications" list above as a rebase, not a replacement.
  5. Update the SHA at the top of this file and re-run
     tools/flash/tests/test_as3_bridge.py.

The MIT notice in ./LICENSE must be preserved in any distribution.
