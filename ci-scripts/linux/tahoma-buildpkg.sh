#!/bin/bash
export TAHOMA2DVERSION=1.6.1
#source /opt/qt515/bin/qt515-env.sh

echo ">>> Temporary install of Flare"
SCRIPTPATH=`dirname "$0"`
export BUILDDIR=$SCRIPTPATH/../../toonz/build
cd $BUILDDIR
# Leave one processor available for other processing if possible
parallel=$(($(nproc) < 2 ? 1 : $(nproc) - 1))
sudo make -j "$parallel" install

sudo ldconfig

echo ">>> Creating appDir"
if [ -d appdir ]
then
   rm -rf appdir
fi
mkdir -p appdir/usr

echo ">>> Copy and configure Flare installation in appDir"
cp -r /opt/flare/* appdir/usr
cp appdir/usr/share/applications/*.desktop appdir
cp appdir/usr/share/icons/hicolor/128x128/apps/*.png appdir
mv appdir/usr/lib/flare/* appdir/usr/lib
rmdir appdir/usr/lib/flare

echo ">>> Creating Flare directory"
if [ -d Flare ]
then
   rm -rf Flare
fi
mkdir Flare

echo ">>> Copying stuff to Flare/tahomastuff"

mv appdir/usr/share/flare/stuff Flare/tahomastuff
chmod -R 777 Flare/tahomastuff
rmdir appdir/usr/share/flare

find Flare/tahomastuff -name .gitkeep -exec rm -f {} \;

if [ -d ../../thirdparty/apps/ffmpeg/bin ]
then
   echo ">>> Copying FFmpeg to Flare/ffmpeg"
   if [ -d Flare/ffmpeg ]
   then
      rm -rf Flare/ffmpeg
   fi
   mkdir -p Flare/ffmpeg
   cp -R ../../thirdparty/apps/ffmpeg/bin/ffmpeg ../../thirdparty/apps/ffmpeg/bin/ffprobe Flare/ffmpeg
   chmod -R 755 Flare/ffmpeg
fi

if [ -d ../../thirdparty/apps/rhubarb ]
then
   echo ">>> Copying Rhubarb Lip Sync to Flare/rhubarb"
   if [ -d Flare/rhubarb ]
   then
      rm -rf Flare/rhubarb
   fi
   mkdir -p Flare/rhubarb
   cp -R ../../thirdparty/apps/rhubarb/rhubarb ../../thirdparty/apps/rhubarb/res Flare/rhubarb
   chmod 755 -R Flare/rhubarb
fi

if [ -d ../../thirdparty/canon/Library ]
then
   echo ">>> Copying canon libraries"
   cp -R ../../thirdparty/canon/Library/x86_64/* appdir/usr/lib
fi

echo ">>> Copying libghoto2 supporting directories"
cp -r /usr/local/lib/libgphoto2 appdir/usr/lib
cp -r /usr/local/lib/libgphoto2_port appdir/usr/lib

rm appdir/usr/lib/libgphoto2/print-camera-list
find appdir/usr/lib/libgphoto2* -name *.la -exec rm -f {} \;
find appdir/usr/lib/libgphoto2* -name *.so -exec patchelf --set-rpath '$ORIGIN/../..' {} \;

echo ">>> Creating Flare/Flare.AppImage"

if [ -f /usr/lib/qt5/bin/linuxdeployqt ]
then
   LINUXDEPLOYQT=/usr/lib/qt5/bin/linuxdeployqt
else
if [ ! -f linuxdeployqt*.AppImage ]
then
   wget -c "https://github.com/probonopd/linuxdeployqt/releases/download/continuous/linuxdeployqt-continuous-x86_64.AppImage"
   chmod a+x linuxdeployqt*.AppImage
fi
   LINUXDEPLOYQT=./linuxdeployqt*.AppImage
fi

export LD_LIBRARY_PATH=appdir/usr/lib/flare
$LINUXDEPLOYQT appdir/usr/bin/Flare -bundle-non-qt-libs -verbose=0 -always-overwrite -no-strip \
   -extra-plugins=platforms/libqwayland-xcomposite-glx.so,platforms/libqwayland-generic.so,platforms/libqwayland-egl.so,platforms/libqwayland-xcomposite-egl.so,wayland-decoration-client,wayland-graphics-integration-client,wayland-graphics-integration-server,wayland-shell-integration \
   -executable=appdir/usr/bin/lzocompress \
   -executable=appdir/usr/bin/lzodecompress \
   -executable=appdir/usr/bin/tcleanup \
   -executable=appdir/usr/bin/tcomposer \
   -executable=appdir/usr/bin/tconverter \
   -executable=appdir/usr/bin/tfarmcontroller \
   -executable=appdir/usr/bin/tfarmserver 

rm appdir/AppRun
cp ../sources/scripts/AppRun appdir
chmod 775 appdir/AppRun

$LINUXDEPLOYQT appdir/usr/bin/Flare -appimage -no-strip 

mv Flare*.AppImage Flare/Flare.AppImage

echo ">>> Creating Flare Linux package"

tar zcf Flare-linux.tar.gz Flare

echo ">>> Creating Flare Debian Package"

chmod +x ../installer/linux/deb-creator/debcreator.sh 

../installer/linux/deb-creator/debcreator.sh \
 -p $TAHOMA2DVERSION \
 -v $TAHOMA2DVERSION \
 -t ../installer/linux/deb-creator/deb-template \
 -x ./appdir \
 -f ./Flare/ffmpeg \
 -r ./Flare/rhubarb \
 -s ../../stuff

 mv tahoma2d_*_amd64.deb Flare-linux.deb