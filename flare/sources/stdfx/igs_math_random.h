#pragma once

#ifndef igs_math_random_h
#define igs_math_random_h

/*
履歴
2001-04-25 update
2001-06-19 rename to 'calculator_random_plus_long.h'
2005-02-03 rename to 'ptbl_random.h'
2005-02-10 separate to 'ptbl_random.h' and 'ptbl_random.cxx'
解説 (2005-02-03.thu wrote)
疑似乱数整数列発生関数rand()は処理系定義であるため、
たとえば、radhat9とWindowsとでは疑似乱数列が違う。
また、random()関数は、radhat9にはあるが、MS-Cにはない。
よって、ポータビリティを持たせるため独立した関数定義をする
参考:「ANSI C/C++辞典」 平林雅英 共立出版株式会社
*/

// Implemented in igs_math_random.rs (flare_formats).
extern "C" {
long flare_igs_random_next(unsigned long *seed);
double flare_igs_random_next_d(unsigned long *seed);
}

namespace igs {
namespace math {
class random {
public:
  random() : seed_(1) {}

  /* 乱数種 seed(0〜std::numeric_limits<long>::max()) */
  void seed(unsigned long seed) { this->seed_ = seed; }
  unsigned long seed(void) const { return this->seed_; }

  /* 乱数生成 0〜std::numeric_limits<long>::max() */
  long next(void) { return flare_igs_random_next(&this->seed_); }
  double next_d(void) { /* 0 ... 1 */
    return flare_igs_random_next_d(&this->seed_);
  }
private:
  unsigned long seed_;
};
}
}

#endif /* !igs_math_random_h */
