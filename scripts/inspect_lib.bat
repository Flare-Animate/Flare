@echo off
"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
dumpbin /headers %1 > "%TEMP%\dumpbin_out.txt" 2>&1
type "%TEMP%\dumpbin_out.txt"