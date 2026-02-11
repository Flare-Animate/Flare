#!/bin/tcsh
cp $SCRIPTROOT/flaredef.$1  $HOME/flaredef  
source $HOME/.cshrc
source $HOME/.login
./copy_plugin.sh
./installName.sh


