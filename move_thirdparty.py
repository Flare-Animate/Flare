import os,shutil
src=r"c:\Users\charl\Documents\Flare\thirdparty\thirdparty"
dst=r"c:\Users\charl\Documents\Flare\thirdparty"
print('src exists', os.path.isdir(src))
for name in os.listdir(src):
    s=os.path.join(src,name)
    d=os.path.join(dst,name)
    print('move',s,'->',d)
    if os.path.isdir(s):
        shutil.move(s,d)
    else:
        shutil.move(s,d)
# remove src dir if empty
if os.path.isdir(src) and not os.listdir(src):
    os.rmdir(src)
print('done')