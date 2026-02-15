@echo off
"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
dumpbin /exports thirdparty\thirdparty\glew\glew-1.9.0\bin\64bit\glew32.dll > scripts\glew_dump.txt
echo done