#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* 0=unknown 1=SWF 2=CFBF(legacy .fla) 3=XFL zip 4=XFL DOMDocument.xml 5=SOL(shared object)
   6=XDTS 7=xstage 8=TLV 9=PLI 10=TZP 11=TNZ/TZU 12=Flipnote PPM 13=KWZ 14=Wick
   15=FlippaClip 16=zipped Moho 17=Moho 18=.anime/.anme 19=Tahoma 20=PLT 21=TPL */
int32_t flare_detect_format(const uint8_t *data, size_t len);
/* Same, with the file extension (no dot, any case) as tie-breaker for formats that have no
   content magic of their own: project folders, .pli vs .plt, .tpl. ext may be NULL. */
int32_t flare_detect_format_ext(const uint8_t *data, size_t len, const char *ext);
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
/* Frame count of SWF / zip FLA / XFL folder or DOMDocument.xml (UTF-8 path); -1 if unknown. */
int32_t flare_flash_frame_count(const char *path_utf8);
/* CFBF (legacy FLA) streams as "name\tsize\n" lines; NULL if not CFBF. Free with flare_moho_free. */
char *flare_cfbf_streams(const uint8_t *data, size_t len);
typedef struct FlareCfbf FlareCfbf;
FlareCfbf *flare_cfbf_open(const uint8_t *data, size_t len);
void flare_cfbf_free(FlareCfbf *h);
size_t flare_cfbf_count(const FlareCfbf *h);
const uint8_t *flare_cfbf_stream(const FlareCfbf *h, size_t i, size_t *len);
size_t flare_cfbf_bitmaps(const FlareCfbf *h, size_t i, uint64_t *out, size_t cap);
/* Flash Local Shared Object (.sol): 1 if the header matches, else 0. */
int32_t flare_lso_is_lso(const uint8_t *data, size_t len);
/* .sol -> JSON {name, formatVersion, body}; NULL if not AMF0. Free with flare_lso_free. */
char *flare_lso_parse(const uint8_t *data, size_t len);
void flare_lso_free(char *p);
/* AS3 / ABC, native Rust (common/flash/ac_as3_*.rs). data = SWF image or bare ABC block.
   JSON-returning calls give NULL on bad arguments; free strings with flare_as3_free. */
char *flare_as3_version(void);
char *flare_as3_list(const uint8_t *data, size_t len);
char *flare_as3_decompile(const uint8_t *data, size_t len);
char *flare_as3_decompile_class(const uint8_t *data, size_t len, int32_t block, int32_t class_idx);
char *flare_as3_disasm(const uint8_t *data, size_t len, int32_t block, int32_t method);
char *flare_as3_compile(const char *src_dir_utf8, const char *out_swf_utf8);
int32_t flare_as3_has_compiler(void);
void flare_as3_free(char *p);
typedef struct FlareAs3Result FlareAs3Result;
FlareAs3Result *flare_as3_patch_strings(const uint8_t *data, size_t len, const char *patch_json_utf8);
FlareAs3Result *flare_as3_patch_class(const uint8_t *data, size_t len, int32_t block, int32_t class_idx,
                                      const char *edited_src_utf8);
const uint8_t *flare_as3_result_bytes(const FlareAs3Result *r, size_t *len);
const char *flare_as3_result_json(const FlareAs3Result *r);
void flare_as3_result_free(FlareAs3Result *r);
/* AnimationCraft bridge (ac-core/ac-xfl/ac-render/ac-audio; common/flash/ac_bridge.rs). Results are logs
   starting "ok" or "error"; NULL on NULL arguments. Free with flare_moho_free. */
int32_t flare_ac_available(void);
/* XFL/FLA (zip, folder, DOMDocument.xml, .xfl) -> out_dir/{frames.tsv (layer,name,row,file), sound.tsv
   (row,file,startOffset,endOffset in frames), meta.tsv (w,h,fps,frames)} plus one SVG per drawing/tween frame. */
char *flare_ac_xfl_import(const char *path_utf8, const char *out_dir_utf8);
/* Scene manifest TSV (doc/cell/sound lines) -> .fla zip, or an XFL folder when out ends in .xfl. */
char *flare_ac_xfl_export(const char *manifest_utf8, const char *out_utf8);
/* Scene manifest -> HTML5 publish (single_file: one .html at out_path, else a folder). */
char *flare_ac_html5_export(const char *manifest_utf8, const char *out_utf8, int32_t single_file);
/* XFL/FLA -> HTML5 publish. */
char *flare_ac_html5_export_xfl(const char *xfl_utf8, const char *out_utf8, int32_t single_file);
#ifdef __cplusplus
}
#endif

/* FlipaClip .fcp -> JSON {fps,width,height,frames[]}; NULL on error. Free with flare_fcp_free. */
char *flare_fcp_parse(const unsigned char *data, size_t len);
void flare_fcp_free(char *p);
