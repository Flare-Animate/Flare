#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <sys/param.h>
#include <string>
#include <cstdint>
#include "orig.cpp"  // original casmfileinfo.cpp, copied next to a stub casmfileinfo.h

static uint64_t st;
static uint32_t rnd() { st = st * 6364136223846793005ULL + 1442695040888963407ULL; return (uint32_t)(st >> 33); }
static void emit(const std::string &d) {
  FILE *f = fopen(CASE_PATH, "wb");
  fwrite(d.data(), 1, d.size(), f); fclose(f);
  int s, e; bool il;
  int cnt = compute_casm_range(CASE_PATH, s, e, il);
  printf("        (b\"");
  for (unsigned char c : d) {
    if (c == '\n') printf("\\n"); else if (c == '\r') printf("\\r"); else if (c == '\t') printf("\\t");
    else if (c == '"' || c == '\\') printf("\\%c", c); else if (c >= 32 && c < 127) printf("%c", c); else printf("\\x%02x", c);
  }
  printf("\", %d, %d, %d, %s),\n", cnt, s, e, il ? "true" : "false");
}
int main() {
  const char *kw[] = {"FRAME", "FIELD", "FRAMEX", "frame", "NOTE", "FRAM"};
  const char *pre[] = {"", " ", "\t", "  \t ", "x "};
  for (int seed = 1; seed <= 60; seed++) {
    st = seed;
    std::string d;
    int lines = rnd() % 12;
    for (int i = 0; i < lines; i++) {
      d += pre[rnd() % 5];
      d += kw[rnd() % 6];
      int m = rnd() % 5;
      if (m == 0) d += " " + std::to_string(rnd() % 1000);
      else if (m == 1) d += "_a" + std::to_string(rnd() % 100000) + "b7";
      else if (m == 2) d += " 99999999999999";
      else if (m == 3) d += std::string(rnd() % 4, ' ');
      if (rnd() % 7 == 0) d += std::string(4090 + rnd() % 12, 'z') + " FRAME 5";
      d += (rnd() % 4 == 0) ? "\r\n" : "\n";
    }
    if (rnd() % 3 == 0 && !d.empty()) d.pop_back();
    emit(d);
  }
  emit(std::string("FRAME 1\n") + std::string(8180, ' ') + "FRAME 9\n");
  emit(std::string(4094, ' ') + "FRAME 4\nFRAME 5\n");
  emit(std::string("FRA\0ME 3\nFRAME 2\n", 17));
}
