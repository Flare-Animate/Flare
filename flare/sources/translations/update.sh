#!/bin/bash
#
# Usage: update.sh LANGUAGE

BASE_DIR=$(cd `dirname "$0"`; pwd)
flareLANG=$1

[ -d "${BASE_DIR}/${flareLANG}" ] || mkdir -p "${BASE_DIR}/${flareLANG}"
cd "${BASE_DIR}/${flareLANG}"
lupdate ../../colorfx/ -ts colorfx.ts
lupdate ../../common/ -ts tnzcore.ts
lupdate ../../tnztools/ -ts tnztools.ts
lupdate ../../flare/ -ts flare.ts
lupdate ../../flarelib/ -ts flarelib.ts
lupdate ../../flareqt/ -ts flareqt.ts
lupdate ../../image/ -ts image.ts


