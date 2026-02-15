import pefile
src = r"c:\Users\charl\Documents\Flare\build\OpenToonzPortable\OpenToonzPortable\libmypaint-1-4-0.dll"
dst_def = r"c:\Users\charl\Documents\Flare\thirdparty\libmypaint\dist\64\libmypaint.def"
pe = pefile.PE(src)
exports = []
if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
    for exp in pe.DIRECTORY_ENTRY_EXPORT.symbols:
        if exp.name:
            exports.append(exp.name.decode('utf-8', errors='ignore'))
with open(dst_def, 'w', newline='\n') as f:
    f.write('LIBRARY libmypaint-1-4-0.dll\nEXPORTS\n')
    for e in exports:
        f.write(e + '\n')
print('WROTE_DEF', dst_def, 'EXPORT_COUNT', len(exports))