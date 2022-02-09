#!/bin/sh

curl https://download.libsodium.org/libsodium/releases/libsodium-1.0.18-stable.tar.gz > libsodium.tar.gz
tar -xf libsodium.tar.gz
rm libsodium.tar.gz

INSTALL_TO="$(pwd)/sodium"
cd libsodium-stable
./configure --prefix="$INSTALL_TO"
make && make check
make install
