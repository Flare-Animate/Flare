@echo on
:: OpenCV and Boost are installed conditionally by the workflow when the cache
:: is missing (see "Install OpenCV and Boost if cache is missing" step).
:: Do NOT re-install them here — that would defeat the cache and cause version
:: mismatches if the toolset or version env vars ever diverge.

if exist vcpkg\bootstrap-vcpkg.bat (
  echo vcpkg bootstrap present, skipping clone
  goto :vcpkg_ready
)

if exist vcpkg (
  echo Found stale/partial vcpkg directory; removing
  rmdir /S /Q vcpkg || (echo ERROR: failed to remove stale vcpkg folder & exit /b 1)
)

set CLONE_ATTEMPTS=0
:clone_retry
set /A CLONE_ATTEMPTS+=1
echo Attempt %CLONE_ATTEMPTS%: git clone vcpkg (depth=1)
git clone --depth 1 https://github.com/microsoft/vcpkg.git vcpkg
if %ERRORLEVEL% EQU 0 goto :vcpkg_ready

echo git clone failed with exit code %ERRORLEVEL% on attempt %CLONE_ATTEMPTS%
if %CLONE_ATTEMPTS% GEQ 3 (
  echo All git clone attempts failed — trying zip fallback
  curl -fsSL -o vcpkg.zip https://github.com/microsoft/vcpkg/archive/refs/heads/master.zip || (echo ERROR: curl vcpkg.zip failed & exit /b 1)
  7z x -y vcpkg.zip -ovcpkg_tmp || (echo ERROR: unzip vcpkg.zip failed & exit /b 1)
  if exist vcpkg_tmp\vcpkg-master (
    move /Y vcpkg_tmp\vcpkg-master vcpkg || (echo ERROR: move vcpkg failed & exit /b 1)
    rmdir /S /Q vcpkg_tmp
    del /Q vcpkg.zip
    goto :vcpkg_ready
  ) else (
    echo ERROR: zip fallback didn't produce expected folder
    dir vcpkg_tmp /s
    exit /b 1
  )
) else (
  echo Retrying in 3s...
  timeout /t 3 /nobreak
  goto :clone_retry
)

:vcpkg_ready
:: Diagnostics
echo --- vcpkg folder listing ---
dir vcpkg /s

if not exist vcpkg\bootstrap-vcpkg.bat (
  echo ERROR: bootstrap-vcpkg.bat missing in vcpkg\ directory
  dir vcpkg /s
  exit /b 1
)

pushd vcpkg
set BOOT_ATTEMPTS=0
:boot_retry
set /A BOOT_ATTEMPTS+=1
echo Bootstrap attempt %BOOT_ATTEMPTS% ...
call bootstrap-vcpkg.bat
if %ERRORLEVEL% EQU 0 goto :boot_ok
echo bootstrap-vcpkg.bat failed on attempt %BOOT_ATTEMPTS%
if %BOOT_ATTEMPTS% GEQ 3 (
  echo ERROR: bootstrap-vcpkg.bat failed after 3 attempts
  popd
  exit /b 1
)
echo Retrying in 10s...
timeout /t 10 /nobreak
goto :boot_retry
:boot_ok
.\vcpkg.exe install lzo:x64-windows || (echo ERROR: vcpkg install lzo failed & popd & exit /b 1)
popd
echo VCPKG_ROOT=%GITHUB_WORKSPACE%\vcpkg>> %GITHUB_ENV%
