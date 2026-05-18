#!/bin/bash
export TAHOMA2DVERSION=1.6.1

if [ -d /usr/local/Cellar/qt@5 ]
then
   export QTDIR=/usr/local/opt/qt@5
elif [ -d /opt/homebrew/opt/qt@5 ]
then
   export QTDIR=/opt/homebrew/opt/qt@5
else
   export QTDIR=/usr/local/opt/qt
fi
export TOONZDIR=toonz/build/toonz

# If found, use Xcode Release build
if [ -d $TOONZDIR/Release ]
then
   export TOONZDIR=$TOONZDIR/Release
fi

if [ -d $TOONZDIR/Flare.app/tahomastuff ]
then
   # In case of prior builds, replace stuff folder
   rm -rf $TOONZDIR/Flare.app/tahomastuff
fi

if [ -d thirdparty/apps/ffmpeg/bin ]
then
   echo ">>> Copying FFmpeg to Flare.app/ffmpeg"
   if [ -d $TOONZDIR/Flare.app/ffmpeg ]
   then
      # In case of prior builds, replace ffmpeg folder
      rm -rf $TOONZDIR/Flare.app/ffmpeg
   fi
   mkdir $TOONZDIR/Flare.app/ffmpeg
   cp -R thirdparty/apps/ffmpeg/bin/ffmpeg thirdparty/apps/ffmpeg/bin/ffprobe $TOONZDIR/Flare.app/ffmpeg
   chmod -R 755 $TOONZDIR/Flare.app/ffmpeg
fi

if [ -d thirdparty/apps/rhubarb ]
then
   echo ">>> Copying Rhubarb Lip Sync to Flare.app/rhubarb"
   if [ -d $TOONZDIR/Flare.app/rhubarb ]
   then
      # In case of prior builds, replace rhubarb folder
      rm -rf $TOONZDIR/Flare.app/rhubarb
   fi
   mkdir $TOONZDIR/Flare.app/rhubarb
   cp -R thirdparty/apps/rhubarb/rhubarb thirdparty/apps/rhubarb/res $TOONZDIR/Flare.app/rhubarb
   chmod -R 755 $TOONZDIR/Flare.app/rhubarb
fi

if [ ! -d $TOONZDIR/Flare.app/Contents/Frameworks ]
then
   mkdir $TOONZDIR/Flare.app/Contents/Frameworks
fi

if [ -d thirdparty/canon/Framework ]
then
   if [ ! -d $TOONZDIR/Flare.app/Contents/Frameworks/EDSDK.framework ]
   then
      echo ">>> Copying canon framework to Flare.app/Contents/Frameworks/EDSDK.Framework"
      cp -R thirdparty/canon/Framework/ $TOONZDIR/Flare.app/Contents/Frameworks
      chmod -R 755 $TOONZDIR/Flare.app/Contents/Frameworks/EDSDK.framework
   fi
fi

if [ ! -d $TOONZDIR/Flare.app/Contents/Frameworks/libgphoto2 ]
then
   echo ">>> Copying libghoto2 supporting directories to Flare.app/Contents/Frameworks"
   cp -R /usr/local/lib/libgphoto2 $TOONZDIR/Flare.app/Contents/Frameworks
   cp -R /usr/local/lib/libgphoto2_port $TOONZDIR/Flare.app/Contents/Frameworks

   rm $TOONZDIR/Flare.app/Contents/Frameworks/libgphoto2/print-camera-list
   find $TOONZDIR/Flare.app/Contents/Frameworks/libgphoto2* -name *.la -exec rm -f {} \;
fi

echo ">>> Creating DSYM files"
if [ -d $TOONZDIR/DSYM ]
then
   rm -rf $TOONZDIR/DSYM
fi

for X in `find $TOONZDIR/Flare.app/Contents/MacOS -type f`
do
   dsymutil -o $TOONZDIR/DSYM $X
   strip -S $X
done

if [ -d $TOONZDIR/Flare.app/DSYM ]
then
   rm -rf $TOONZDIR/Flare.app/DSYM
fi

echo ">>> Configuring Flare.app for deployment"

$QTDIR/bin/macdeployqt $TOONZDIR/Flare.app -verbose=0 -always-overwrite \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/lzocompress \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/lzodecompress \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/tcleanup \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/tcomposer \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/tconverter \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/tfarmcontroller \
   -executable=$TOONZDIR/Flare.app/Contents/MacOS/tfarmserver 

for FW in `echo "QtDBus QtPdf QtQml QtQmlModels QtQuick QtVirtualKeyboard"`
do
   if [ ! -d $TOONZDIR/Flare.app/Contents/Frameworks/$FW.framework ]
   then
      echo ">>> Copying missing $FW.framework to Flare.app/Contents/Frameworks"
      cp -r $QTDIR/Frameworks/$FW.framework $TOONZDIR/Flare.app/Contents/Frameworks
   fi
done

if [ ! -d $TOONZDIR/Flare.app/Contents/lib ]
then
   echo ">>> Adding Contents/lib symbolic link to Flare.app/Contents/Frameworks"
   ln -s Frameworks $TOONZDIR/Flare.app/Contents/lib
fi

echo ">>> Correcting library paths"
function checkLibFile() {
   local LIBFILE=$1   
   for DEPFILE in `otool -L $LIBFILE | sed -e "s/ (.*$//" | grep -e"\/usr\/local" -e"@rpath" -e"\.\./\.\./\.\." | grep -v "/qt"`
   do
      local Z=`echo $DEPFILE | cut -c 1-1`
      if [ "$Z" = "/" -o "$Z" = "@" ]
      then
         local Y=`basename $DEPFILE`
         local W=`basename $LIBFILE`
         local X=`echo $DEPFILE | grep "\.framework\/"`
         if [ "$X" = "" -a ! -f $TOONZDIR/Flare.app/Contents/Frameworks/$Y ]
         then
            local SRC=$DEPFILE
            local Z=`echo $DEPFILE | cut -c 1-16`
            local Z2=`echo $DEPFILE | cut -c 1-6`
            if [ "$Z" = "@loader_path/../" ]
            then
               local V=`echo $DEPFILE | sed -e"s/^.*\/\.\.\///"`
               local SRC=/usr/local/$V
            elif [ "$Z2" = "@rpath" ]
            then
                local SRC=/usr/local/lib/$Y
            fi
            echo "Copying $SRC to Frameworks"
            cp $SRC $TOONZDIR/Flare.app/Contents/Frameworks
            chmod 644 $TOONZDIR/Flare.app/Contents/Frameworks/$Y
            local ORIGDEPFILE=$DEPFILE
            checkLibFile $TOONZDIR/Flare.app/Contents/Frameworks/$Y
            DEPFILE=$ORIGDEPFILE
         fi
         if [ "$Y" != "$W" ]
         then
            echo "Fixing $DEPFILE in $LIBFILE"
            if [ "$X" != "" ]
            then
               local Y=`echo $DEPFILE | sed -e"s/^.*\/\.\.\///" -e"s/@rpath.//"`
               install_name_tool -change $DEPFILE @executable_path/../Frameworks/$Y $LIBFILE
            else
               install_name_tool -change $DEPFILE @executable_path/../Frameworks/$Y $LIBFILE
            fi
         fi
         FIXCHECK=`otool -D $LIBFILE | grep -v ":" | grep -e"\/usr\/local"`
         if [ "$FIXCHECK" == "$DEPFILE" ]
         then
            echo "   Fixed ID!"
            install_name_tool -id @executable_path/../Frameworks/$Y $LIBFILE
         fi
      fi
   done
}

for FILE in `find $TOONZDIR/Flare.app/Contents -type f | grep -v -e"\.h" -e"\.prl" -e"\.plist" -e"\.conf" -e"\.icns" -e"EDSDK" -e"\/Headers\/"`
do
   checkLibFile $FILE
done

echo ">>> Moving DYSM to Flare.app"
mv $TOONZDIR/DSYM $TOONZDIR/Flare.app

echo ">>> Creating Flare-install-osx.pkg"

toonz/installer/osx/app.rb $TOONZDIR stuff toonz/installer/osx/scripts $TAHOMA2DVERSION

mv $TOONZDIR/Flare-install-osx.pkg $TOONZDIR/..

echo ">>> Creating Flare-portable-osx.dmg"

cp -R stuff $TOONZDIR/Flare.app/tahomastuff
chmod -R 777 $TOONZDIR/Flare.app/tahomastuff

find $TOONZDIR/Flare.app/tahomastuff -name .gitkeep -exec rm -f {} \;

cd $TOONZDIR

# Due to random ERROR: Bundle creation error: "hdiutil: create failed - Resource busy\n"
# We'll try to create the DMG a few times

let MAXTRY=10

for TRY in $(seq 1 $MAXTRY)
do
   if [ $TRY -gt  1 ]
   then
      echo ">>> DMG file creation failed.  Retrying $TRY/$MAXTRY..."
   fi

    $QTDIR/bin/macdeployqt Flare.app -dmg -verbose=0
    if [ -f Flare.dmg ]
    then
       echo ">>> DMG file created successfully"
       mv Flare.dmg ../Flare-portable-osx.dmg
       exit 0
    fi
done

echo ">>> DMG file creation failed after too many attempts. Aborting!"
exit 1

