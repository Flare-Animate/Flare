# Building Flare for Android (experimental)

Status: scaffolding. Toolchain, Gradle project and CI job exist; desktop-only native deps
(libtiff, SuperLU, OpenCV, ...) still need Android ports, so a full APK is not yet produced.
All files are additive; desktop builds are unaffected.

## Requirements
JDK 17, Android SDK (API 34) + NDK r26+, Qt 6.5+ for Android (arm64_v8a), CMake 3.22+, Ninja.

## Configure
```
export ANDROID_NDK_ROOT=/path/to/ndk
cmake -S . -B build-android -G Ninja \
  -DCMAKE_TOOLCHAIN_FILE=$PWD/cmake/android-toolchain.cmake \
  -DANDROID_NDK=$ANDROID_NDK_ROOT -DQT_ANDROID_DIR=/path/to/Qt/6.5.3/android_arm64_v8a
```

## Gradle
```
cd flare/android
gradle :app:assembleDebug -PqtAndroidDir=/path/to/Qt/6.5.3/android_arm64_v8a
```

CI: `.github/workflows/workflow_android.yml` (non-blocking; runs the configure step).
