#pragma once

// Redirect to the canonical TFlash implementation in the Flare tree to avoid
// duplicate/incompatible stub definitions that can cause type conversion and
// ODR issues during compilation.
// NOTE: the relative path is intentionally used so that this header can still be
// included from the toonz compatibility tree.
#include "../../../../flare/sources/common/flash/tflash.h"
