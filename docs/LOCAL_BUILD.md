# Local Linux build and GUI smoke test

## Build

    scripts/dev/local_build.sh            # configure (first time) + ninja -j2 into /home/user/bld

- Mirrors `.github/workflows/workflow_linux.yml`: Release, `-DWITH_TRANSLATION=OFF -DWITH_SYSTEM_LZO=ON`, gcc, ccache if installed, `-w` to silence warnings.
- Prints only errors; exit code is the build result. Full logs: `$BUILD_DIR/cmake_configure.log` and `$BUILD_DIR/build.log`.
- The build dir lives OUTSIDE the repo and is reused, so later changes only need an incremental `scripts/dev/local_build.sh` (or `ninja -C /home/user/bld`).
- Env: `BUILD_DIR`, `JOBS` (default 2), `CC`/`CXX`, `RECONFIGURE=1`, `VERBOSE=1`. Extra args are passed to ninja (targets).
- Packages: the apt list in the "Install libraries" step of the workflow, plus `xvfb xdotool imagemagick` for the smoke test.

## GUI smoke test

    scripts/dev/gui_smoke.sh /tmp/smoke_out

Starts Xvfb, launches `$BUILD_DIR/bin/Flare` with an isolated `$HOME` and `FLAREROOT=<repo>/stuff`, waits (default 30 s, `SMOKE_TIMEOUT`) for a large visible window, saves `main.png`, then relaunches with `tests/flash_fixtures/sample.fla` as the load argument (`SMOKE_FILE`, empty to skip) and saves `load_sample.png`. Fails (exit 1 crash/early exit, 2 no window, 3 setup) with the log tail and `startup.log`, and checks for fatal markers or crash files. Processes are terminated with SIGTERM.
