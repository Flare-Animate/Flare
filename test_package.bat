@echo off
cd build\RelWithDebInfo
if exist FlarePortable rmdir /S /Q FlarePortable
mkdir FlarePortable
:: copy exe
for /f "delims=" %%i in ('dir /b /s "build\Flare.exe" ^| findstr /v "CMakeFiles"') do copy /Y "%%i" FlarePortable\Flare.exe
:: copy dlls
for /f "delims=" %%d in ('dir /b /s "build\*.dll" ^| findstr /i "RelWithDebInfo" ^| findstr /v "CMakeFiles"') do copy /Y "%%d" FlarePortable\
echo done
pause