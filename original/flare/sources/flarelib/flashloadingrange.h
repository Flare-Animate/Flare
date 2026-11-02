#pragma once

#include "flare/txshsimplelevel.h"

namespace FlashNative {
namespace detail {

// Native level loading resets this process-global range. A candidate import
// must use the unrestricted range without consuming the open scene's range.
class LoadingRangeGuard final {
  TFrameId m_from, m_to;

public:
  LoadingRangeGuard() {
    getLoadingLevelRange(m_from, m_to);
    setLoadingLevelRange(TFrameId(1), TFrameId(0));
  }
  ~LoadingRangeGuard() { setLoadingLevelRange(m_from, m_to); }

  LoadingRangeGuard(const LoadingRangeGuard &)            = delete;
  LoadingRangeGuard &operator=(const LoadingRangeGuard &) = delete;
};

}  // namespace detail
}  // namespace FlashNative
