// Differential generator: runs the ORIGINAL cornerdetector.cpp on LCG-driven
// inputs and prints Rust test fixtures. Each case runs in a forked child so
// cases that hit C++ UB (out-of-range vector access, trapped by
// _GLIBCXX_ASSERTIONS) are dropped instead of recorded.
#include <vector>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <unistd.h>
#include <sys/wait.h>
#include "tgeometry.h"
#include "cornerdetector.h"

static uint64_t st;
static uint32_t rnd() {
  st = st * 6364136223846793005ULL + 1442695040888963407ULL;
  return (uint32_t)(st >> 33);
}

int main() {
  int emitted = 0;
  for (uint64_t seed = 1; emitted < 80 && seed < 2000; seed++) {
    st = seed;
    int n = 2 + rnd() % 40;
    int mode = rnd() % 4;
    std::vector<T3DPointD> p;
    double x = 0, y = 0;
    for (int i = 0; i < n; i++) {
      if (mode == 0) {  // integer lattice walk with turns
        x += (int)(rnd() % 21) - 10;
        y += (int)(rnd() % 21) - 10;
      } else if (mode == 1) {  // fractional
        x += ((int)(rnd() % 4001) - 2000) / 100.0;
        y += ((int)(rnd() % 4001) - 2000) / 100.0;
      } else if (mode == 3) {  // near-duplicates: operator== is an epsilon (1e-8) test
        x += (rnd() % 3 == 0) ? 0.0 : (int)(rnd() % 13) - 6;
        y += (rnd() % 3 == 0) ? 0.0 : (int)(rnd() % 13) - 6;
        if (rnd() % 2) {
          double e = (rnd() % 2) ? 5e-9 : 2e-8;  // below / above epsilon
          p.push_back(T3DPointD(x, y, 0));
          x += e;
        }
      } else {  // long straight runs with sharp turns, with duplicates
        static int dx, dy;
        if (i == 0 || rnd() % 6 == 0) { dx = (int)(rnd() % 9) - 4; dy = (int)(rnd() % 9) - 4; }
        if (rnd() % 8 != 0) { x += dx * 3; y += dy * 3; }
      }
      p.push_back(T3DPointD(x, y, (double)(rnd() % 3)));
    }
    int minS = 1 + rnd() % 4, minD = (int)(rnd() % 8) - 3, maxD = 1 + rnd() % 12;
    double maxA = 60 + rnd() % 80;
    int fd[2];
    pipe(fd);
    pid_t pid = fork();
    if (pid == 0) {
      std::vector<int> c;
      detectCorners(p, minS, minD, maxD, maxA, c);
      for (size_t k = 1; k < c.size(); k++) (void)k;
      int cnt = (int)c.size();
      write(fd[1], &cnt, 4);
      if (cnt) write(fd[1], c.data(), cnt * 4);
      _exit(0);
    }
    close(fd[1]);
    int cnt = -1;
    int r = read(fd[0], &cnt, 4);
    std::vector<int> c;
    if (r == 4 && cnt > 0) {
      c.resize(cnt);
      read(fd[0], c.data(), cnt * 4);
    }
    int status;
    waitpid(pid, &status, 0);
    close(fd[0]);
    if (r != 4 || !WIFEXITED(status) || WEXITSTATUS(status) != 0) continue;
    // empty results hit UB in the original dedupe (begin()+1 on empty); a
    // clean exit with 0 may still be luck, so keep only non-empty ones.
    if (cnt <= 0) continue;
    printf("        (&[");
    for (auto &q : p) printf("(%.17g, %.17g, %.17g), ", q.x, q.y, q.z);
    printf("], %d, %d, %d, %.17g, &[", minS, minD, maxD, maxA);
    for (int v : c) printf("%d, ", v);
    printf("]),\n");
    emitted++;
  }
  fprintf(stderr, "emitted %d\n", emitted);
}
