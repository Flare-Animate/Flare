#pragma once

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define BLOCKROWS 3

#define NEXT2(x) (((x) + 1) & -2)  // uguale o successivo (pad)
#define NEXT4(x) (((x) + 3) & -4)  // uguale o successivo multiplo di 4	(pad)
#define FIXDPI(x) ((x) / 65536.)   // fixedpt

typedef long psdByte;
typedef unsigned short psdUint16;
typedef long psdPixel;

enum { RAWDATA, RLECOMP, ZIPWITHOUTPREDICTION, ZIPWITHPREDICTION };

struct TPSDChannelInfo {
  int id;                         // channel id
  int comptype;                   // channel's compression type
  psdPixel rows, cols, rowbytes;  // computed by dochannel()
  // FIXME: need depth??
  psdByte length;   // channel byte count in file
  psdByte filepos;  // file offset of channel data (AFTER compression type)
  psdByte *rowpos;  // row data file positions (RLE ONLY)
  unsigned char *unzipdata;  // uncompressed data (ZIP ONLY)
};

// Pure helpers live in psdutils.rs (flare_formats). The functions that read
// through a caller-owned FILE* stay here as inline C++: they are nothing but
// fseek/fread/fgetc and cannot be forwarded to another runtime's stream.
extern "C" {
int flare_psd_unpackrow(unsigned char *out, const unsigned char *in,
                        long outlen, long inlen);
int flare_psd_unzip(const unsigned char *src, int src_len, unsigned char *dst,
                    int dst_len);
int flare_psd_unzip_pred(const unsigned char *src, int src_len,
                         unsigned char *dst, int dst_len, int row_size,
                         int color_depth);
void *flare_psd_malloc(long n);
void *flare_psd_calloc(long n, int size);
}

inline int unpackrow(unsigned char *out, unsigned char *in, psdPixel outlen,
                     psdPixel inlen) {
  return flare_psd_unpackrow(out, in, outlen, inlen);
}

inline void readrow(FILE *psd, TPSDChannelInfo *chan,
                    psdPixel row,             // row index
                    unsigned char *inbuffer,  // dest buffer for the uncompressed row
                    unsigned char *tmpbuffer)  // temp rlebuffer for compressed
                                               // data, 2xrb in size
{
  psdPixel n = 0, rlebytes;
  psdByte pos;

  int seekres = 0;

  switch (chan->comptype) {
  case RAWDATA: /* uncompressed */
    pos     = chan->filepos + chan->rowbytes * row;
    seekres = fseek(psd, pos, SEEK_SET);
    if (seekres != -1) n = (psdPixel)fread(inbuffer, 1, chan->rowbytes, psd);
    break;
  case RLECOMP:
    pos     = chan->rowpos[row];
    seekres = fseek(psd, pos, SEEK_SET);
    if (seekres != -1) {
      rlebytes =
          (psdPixel)fread(tmpbuffer, 1, chan->rowpos[row + 1] - pos, psd);
      n = unpackrow(inbuffer, tmpbuffer, chan->rowbytes, rlebytes);
    }
    break;
  case ZIPWITHPREDICTION:
  case ZIPWITHOUTPREDICTION:
    memcpy(inbuffer, chan->unzipdata + chan->rowbytes * row, chan->rowbytes);
    return;
  }
  // if we don't recognise the compression type, skip the row

  if (n < chan->rowbytes) {
    // zero out unwritten part of row
    memset(inbuffer + n, 0, chan->rowbytes - n);
  }
}

// Read 2byte unsigned binary value. BigEndian.
inline unsigned read2UBytes(FILE *f) {
  unsigned n = fgetc(f) << 8;
  return n |= fgetc(f);
}
// Read 2byte binary value. BigEndian.
inline int read2Bytes(FILE *f) {
  unsigned n = fgetc(f) << 8;
  n |= fgetc(f);
  return n < 0x8000 ? n : n - 0x10000;
}
// Read 4byte signed binary value. BigEndian.
inline long read4Bytes(FILE *f) {
  long n = fgetc(f) << 24;
  n |= fgetc(f) << 16;
  n |= fgetc(f) << 8;
  return n | fgetc(f);
}

inline void skipBlock(FILE *f) {
  psdByte n = read4Bytes(f);
  if (n) fseek(f, n, SEEK_CUR);
}

inline void *mymalloc(long n) { return flare_psd_malloc(n); }
inline void *mycalloc(long n, int size) { return flare_psd_calloc(n, size); }

inline int psdUnzipWithoutPrediction(unsigned char *src_buf, int src_len,
                                     unsigned char *dst_buf, int dst_len) {
  return flare_psd_unzip(src_buf, src_len, dst_buf, dst_len);
}
// ZIP WITH PREDICTION: returns 1 on success, 0 on failure
inline int psdUnzipWithPrediction(unsigned char *src_buf, int src_len,
                                  unsigned char *dst_buf, int dst_len,
                                  int row_size, int color_depth) {
  return flare_psd_unzip_pred(src_buf, src_len, dst_buf, dst_len, row_size,
                              color_depth);
}
