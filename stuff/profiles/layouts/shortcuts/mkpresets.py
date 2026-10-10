"""Regenerates otflash/otflipaclip from defflare and fills otanimate gaps. Run from this dir; also self-checks parse."""
import re, configparser
def rd(f):
    d={}
    for l in open(f,encoding='utf-8'):
        m=re.match(r'([A-Za-z_0-9]+)=(.*)$',l.rstrip('\n'))
        if m:d[m[1]]=m[2]
    return d
def wr(f,d):
    open(f,'w',encoding='utf-8',newline='\n').write('[shortcuts]\n'+''.join(f'{k}={v}\n' for k,v in d.items()))
base=rd('defflare.ini')
def derive(ov):
    d=dict(base)
    for k,v in ov.items():
        for o in d:
            if d[o]==v and o!=k:d[o]=''
        d[k]=v
    return d
flash={'T_Selection':'V','T_ControlPointEditor':'A','T_Brush':'B','T_Geometric':'M','T_Fill':'K','T_PaintBrush':'I','T_Eraser':'E','T_Hand':'H','T_Zoom':'Z','T_Rotate':'Q','T_Type':'T','T_Tape':'',
 'MI_Insert':'F5','MI_InsertAbove':'','MI_Clear':'Shift+F5','MI_SetKeyframes':'F6','MI_Play':'Return','MI_Preview':'Ctrl+Return',
 'MI_PrevFrame':'Ctrl+,','MI_NextFrame':'Ctrl+.','MI_PrevDrawing':',','MI_NextDrawing':'.','MI_Group':'Ctrl+G','MI_Ungroup':'Ctrl+Shift+G','MI_Duplicate':'Ctrl+D','MI_OnionSkin':'Alt+Shift+O','MI_Loop':'','MI_SaveScene':'Ctrl+Shift+S','MI_SaveAll':'Ctrl+S'}
flip={'T_Brush':'B','T_Eraser':'E','T_Fill':'F','T_Selection':'L','T_Hand':'H','T_Zoom':'Z','T_Type':'T','MI_Play':'Space','MI_OnionSkin':'O','MI_Insert':'Ctrl+Shift+N','MI_Undo':'Ctrl+Z','MI_Redo':'Ctrl+Shift+Z','MI_PrevFrame':'Left','MI_NextFrame':'Right','MI_Duplicate':'Ctrl+D'}
wr('otflash.ini',derive(flash));wr('otflipaclip.ini',derive(flip))
a=rd('otanimate.ini');used={v for v in a.values() if v}
for k,v in base.items():
    if k not in a:a[k]=v if v not in used else ''
wr('otanimate.ini',a)
for f in ['otflash','otflipaclip','otanimate']:
    c=configparser.ConfigParser();c.optionxform=str;c.read(f+'.ini',encoding='utf-8');print(f,len(c['shortcuts']))
