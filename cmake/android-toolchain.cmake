# Wraps the NDK toolchain + Qt for Android. Usage:
#  cmake -S . -B build-android -DCMAKE_TOOLCHAIN_FILE=cmake/android-toolchain.cmake \
#    -DANDROID_NDK=$ANDROID_NDK_ROOT -DQT_ANDROID_DIR=<Qt>/android_arm64_v8a
if(NOT ANDROID_NDK)
  set(ANDROID_NDK "$ENV{ANDROID_NDK_ROOT}")
endif()
if(NOT EXISTS "${ANDROID_NDK}/build/cmake/android.toolchain.cmake")
  message(FATAL_ERROR "Set ANDROID_NDK (or ANDROID_NDK_ROOT) to a valid NDK")
endif()
set(ANDROID_ABI arm64-v8a CACHE STRING "")
set(ANDROID_PLATFORM android-28 CACHE STRING "")
set(ANDROID_STL c++_shared CACHE STRING "")
if(QT_ANDROID_DIR)
  list(APPEND CMAKE_FIND_ROOT_PATH "${QT_ANDROID_DIR}")
  set(CMAKE_PREFIX_PATH "${QT_ANDROID_DIR}" CACHE STRING "")
endif()
set(FLARE_ANDROID ON CACHE BOOL "")
include("${ANDROID_NDK}/build/cmake/android.toolchain.cmake")
