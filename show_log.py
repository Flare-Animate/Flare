import subprocess,os
os.chdir(r"c:\Users\charl\Documents\Flare")
import subprocess
print('LOCAL HEAD:')
print(subprocess.run(['git','log','-10','--oneline'],capture_output=True,text=True).stdout)
print('FETCHING ORIGIN...')
print(subprocess.run(['git','fetch'],capture_output=True,text=True).stdout)
print('REMOTE HEAD:')
print(subprocess.run(['git','log','origin/B4uti4github-patch-1','-10','--oneline'],capture_output=True,text=True).stdout)
