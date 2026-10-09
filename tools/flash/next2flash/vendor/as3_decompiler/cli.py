"""CLI entry point for the AS3 decompiler."""

from __future__ import annotations

import argparse
import logging
import os
import sys

from .swf_reader import read_abc_blocks
from .abc_parser import ABCFile
from .class_decompiler import AS3Decompiler


# ═══════════════════════════════════════════════════════════════════════════
#  CLI
# ═══════════════════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(
        description='AS3 Decompiler — decompile SWF ActionScript 3 bytecode to source',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python as3_decompiler.py game.swf --list
  python as3_decompiler.py game.swf --class Main
  python as3_decompiler.py game.swf --all --outdir decompiled/
  python as3_decompiler.py game.swf --all --outdir decompiled/ --verbose
""")
    parser.add_argument('swf', help='Input SWF file')
    parser.add_argument('--list', action='store_true', help='List all classes')
    parser.add_argument('--class', dest='classname', help='Decompile a specific class')
    parser.add_argument('--all', action='store_true', help='Decompile all classes')
    parser.add_argument('--outdir', default='decompiled', help='Output directory (default: decompiled/)')
    parser.add_argument('--verbose', '-v', action='store_true', help='Verbose output')
    args = parser.parse_args()

    # Set up logging
    log_level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=log_level,
        format='%(message)s',
        stream=sys.stderr
    )
    logger = logging.getLogger(__name__)

    if not os.path.isfile(args.swf):
        logger.error(f'Error: {args.swf} not found')
        sys.exit(1)

    logger.info(f'Reading {args.swf}...')
    version, abc_blocks = read_abc_blocks(args.swf)
    logger.info(f'SWF version {version}')
    if not abc_blocks:
        logger.error('No DoABC tags found in this SWF.')
        sys.exit(1)

    logger.info(f'Found {len(abc_blocks)} ABC block(s)')

    for block_name, abc_data in abc_blocks:
        logger.info(f'Parsing ABC block: "{block_name}" ({len(abc_data)} bytes)')
        abc = ABCFile(abc_data)
        decomp = AS3Decompiler(abc)
        classes = decomp.list_classes()

        if args.list:
            print(f'{"#":>4}  {"Class":<50} {"Super":<30} {"Pkg"}')
            print('-' * 100)
            for c in classes:
                flag = '[I]' if c['is_interface'] else '   '
                print(f'{c["index"]:4}  {flag} {c["name"]:<46} {c["super"]:<30} {c["package"]}')
            print(f'Total: {len(classes)} classes/interfaces')

        elif args.classname:
            found = False
            for c in classes:
                if c['name'] == args.classname or c['full_name'] == args.classname:
                    print(f'--- {c["full_name"]} ---')
                    src = decomp.decompile_class(c['index'])
                    print(src)
                    found = True
                    break
            if not found:
                logger.error(f'Class "{args.classname}" not found.')
                # Suggest similar
                matches = [c for c in classes if args.classname.lower() in c['name'].lower()]
                if matches:
                    logger.info('Did you mean: ' + ', '.join(c['name'] for c in matches[:10]))

        elif args.all:
            outdir = args.outdir
            logger.info(f'Decompiling {len(classes)} classes to {outdir}/')
            count = decomp.decompile_all(outdir)
            logger.info(f'Successfully decompiled {count}/{len(classes)} classes')

        else:
            # Default: list classes
            logger.info(f'{len(classes)} classes found. Use --list, --class NAME, or --all')


if __name__ == '__main__':
    main()
