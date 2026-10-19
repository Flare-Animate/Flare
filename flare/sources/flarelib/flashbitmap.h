#pragma once

#include "../flash/media.h"
#include "../flash/document.h"
#include "trasterimage.h"

// TRasterImage's header selects its own DLL decoration; these APIs belong to
// flarelib and are called from the scene importer and preflight.
#undef DVAPI
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace FlashNative {

// Decode a source-validated embedded payload into an editable native raster.
// This checks actual decoder output in addition to the Flash header bounds.
DVAPI TRasterImageP bitmapImage(const Flash::BitmapPayload &payload,
                                double dpi);
DVAPI TRasterImageP bitmapImage(Flash::Package &package,
                                const Flash::DocumentInfo &document,
                                const QString &libraryName, double dpi);

}  // namespace FlashNative
