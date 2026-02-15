@echo off
"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
lib /def:thirdparty/libmypaint/dist/64/libmypaint.def /OUT:thirdparty/libmypaint/dist/64/libmypaint.lib
echo done