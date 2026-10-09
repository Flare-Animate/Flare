#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* 0=unknown 1=SWF 2=CFBF(legacy .fla) 3=XFL zip 4=XFL DOMDocument.xml */
int32_t flare_detect_format(const uint8_t *data, size_t len);
/* SWF header; returns 0 on success. compression: 'F','C','Z'. */
int32_t flare_swf_header(const uint8_t *data, size_t len, uint8_t *compression,
                         uint8_t *version, uint32_t *file_length);
/* DOMDocument width/height/frameRate; returns 0 on success. */
int32_t flare_xfl_dom_info(const uint8_t *data, size_t len, double *w, double *h, double *fps);
#ifdef __cplusplus
}
#endif
