import re
inp = 'scripts/glew_dump.txt'
out = 'thirdparty/glew/glew-1.9.0/lib/glew64.def'
with open(inp, 'r', encoding='utf-8', errors='ignore') as f:
    lines = f.readlines()
names = []
for line in lines:
    m = re.match(r"\s*\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+(\S+)", line)
    if m:
        names.append(m.group(1))
if not names:
    # fallback: try to find lines after 'ordinal' header
    start = False
    for line in lines:
        if line.strip().lower().startswith('ordinal'):
            start = True
            continue
        if start:
            parts = line.split()
            if len(parts) >= 4:
                names.append(parts[-1])
if not names:
    print('No exports found in', inp)
else:
    with open(out, 'w') as f:
        f.write('LIBRARY "glew32.dll"\nEXPORTS\n')
        for n in names:
            f.write(n + '\n')
    print('Wrote', out)