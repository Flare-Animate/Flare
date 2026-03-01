from pathlib import Path
p = Path('flare/sources/flare/startuppopup.cpp')
print('path exists:', p.exists())
text = p.read_text(encoding='utf-8')
print('length', len(text))
print('Flare count', text.count('Flare'))
print('flare count', text.lower().count('flare'))
for i, line in enumerate(text.splitlines()):
    if 'Flare' in line or 'flare' in line.lower():
        print('line', i+1, line.strip())
        break
