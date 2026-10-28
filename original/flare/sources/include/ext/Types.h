#pragma once

#ifndef TYPES_H
#define TYPES_H

/**
 * @author  Fabrizio Morciano <fabrizio.morciano@gmail.com>
 */

#include "tcommon.h"

#undef DVAPI
#undef DVVAR
#ifdef TNZEXT_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

#include <map>
#include <vector>
#include <stdexcept>
#include <iostream>

#if defined(_MSC_VER) && (_MSC_VER <= 1200)
// to avoid annoying warning
#pragma warning(push)
#pragma warning(disable : 4290)
#endif

// Parity tests live in tnzext/Types.rs (flare_formats); the range_error throws
// stay here because a C++ exception cannot cross the C ABI.
extern "C" {
bool flare_ext_is_odd(int v);
bool flare_ext_is_even(int v);
}

namespace ToonzExt {
namespace Type {
enum Corner { STRAIGHT, SPIRE, UNKNOWN };
}

/**
   * @brief An useful alias for a pair of double.
   */
typedef std::pair<double, double> Interval;

inline std::ostream &operator<<(std::ostream &os, const Interval &interval) {
  return os << '[' << interval.first << ',' << interval.second << ']';
}

/**
   * @brief List of intervals.
   */
typedef std::vector<Interval> Intervals;

/**
   * @brief This class is an abstraction to wrap all odd number.
   *
   * It is useful if you need to manage parameter of a function,
   * that are in some sub set of integer.
   * @sa EvenInt.
   */
// -3 -1 1 3
class DVAPI OddInt {
  int val_;

public:
  OddInt(int v) : val_(v) {}
  /**
*@brief Cast an integer, if is not an even exception is
*       thrown.
*/
  operator int() const {
    if (!isOdd()) throw std::range_error("Value is Even!!!");
    return val_;
  }

  /**
*@copydoc operator int() const
*/
  operator int() {
    if (!isOdd()) throw std::range_error("Value is Even!!!");
    return val_;
  }

  /**
*@brief Simple check to verify that a number is odd,
*       without exception.
*/
  bool isOdd() const { return flare_ext_is_odd(val_); }
};

/**
   * @brief This class is an abstraction to wrap all odd number.
   *
   * It is useful if you need to manage parameter of a function,
   * that are in some sub set of integer.
   * @sa EvenInt.
   */
// -4 -2 0 2 4..
class DVAPI EvenInt {
  int val_;

public:
  EvenInt(int v) : val_(v) {}

  /**
*@brief Cast an integer, if is not an even exception is
*       thrown.
*/
  operator int() const {
    if (!isEven()) throw std::range_error("Value is Odd!!!");
    return val_;
  }

  /**
*@copydoc operator int() const
*/
  operator int() {
    if (!isEven()) throw std::range_error("Value is Odd!!!");
    return val_;
  }

  /**
*@brief Simple check to verify that a number is even,
*       without exception.
*/
  bool isEven() const { return flare_ext_is_even(val_); }
};
}

#if defined(_MSC_VER) && (_MSC_VER <= 1200)
#pragma warning(pop)
#endif

#endif /* TYPES_H */
//-----------------------------------------------------------------------------
//  End Of File
//-----------------------------------------------------------------------------
