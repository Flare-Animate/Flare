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
/* Uncompressed SWF tags (incl. End); -1 on error. codes may be NULL. */
int32_t flare_swf_tag_count(const uint8_t *data, size_t len);
int32_t flare_swf_tag_codes(const uint8_t *data, size_t len, uint16_t *codes, size_t max);
/* First XFL timeline: returns layer count, total frames in *frames; -1 on error. */
int32_t flare_xfl_layer_count(const uint8_t *data, size_t len, uint32_t *frames);
/* Moho .moho/.mohoproj -> normalised JSON (layers, bones, tracks); NULL on error. Free with flare_moho_free. */
char *flare_moho_parse(const uint8_t *data, size_t len);
void flare_moho_free(char *p);
/* XFL dir -> per-frame SVGs + frames.tsv (layer,name,row,file) in out_dir. Returns log
   ("ok N\n..." or "error ..."); NULL on bad args. Free with flare_moho_free. */
char *flare_xfl_export_svg(const char *xfl_dir_utf8, const char *out_dir_utf8);
/* Flash-style symbol library (TSV: kind\tfolder\tname\tlevel; kind mc|gr|btn|dir). Adds line, writes normalized TSV to out; returns full len. */
size_t flare_symlib_add(const uint8_t *lib, size_t lib_len, const uint8_t *line, size_t line_len, uint8_t *out, size_t cap);
/* TFilePath (common/tsystem/tfilepath.cpp, standard mode). kind: 0 type,1 name,2 levelName,3 parentDir,4 dots,5 frame(decimal, -1 none). Writes UTF-8 to out; returns full len. */
size_t flare_tfilepath_query(const char *path_utf8, int32_t kind, char *out, size_t cap);
/* TEnv::getSystemVarPathSetValue split (tenv.cpp). Entries joined by '
'; returns full len, copies up to cap. */
size_t flare_tenv_path_set(const char *value_utf8, char *out, size_t cap);
#ifdef __cplusplus
}
#endif

/* FlipaClip .fcp -> JSON {fps,width,height,frames[]}; NULL on error. Free with flare_fcp_free. */
char *flare_fcp_parse(const unsigned char *data, size_t len);
void flare_fcp_free(char *p);
