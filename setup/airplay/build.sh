#!/bin/bash
# Builds and installs AirPlay 2 for the box: nqptp (the timing peer AirPlay 2 needs) and
# shairport-sync with AirPlay 2. Raspberry Pi OS's own shairport-sync is built without
# AirPlay 2 and nqptp is not packaged, hence the build. Run on the Pi as a sudo-capable
# user; takes a while on a Pi 3. Re-running rebuilds the pinned versions.
set -euo pipefail

# Releases by their commits, not their tags: a tag can be moved, a commit cannot. The
# tag is checked too, so a moved tag fails the build instead of passing unnoticed.
NQPTP_VERSION=1.2.4
NQPTP_COMMIT=591f425d9da69f1c4e09f3ad09611b758937b3e5
SHAIRPORT_VERSION=4.3.7
SHAIRPORT_COMMIT=0b1c4391ffd398e7b145eb4b98416261380adeea
WORK=/var/tmp/ownbox-airplay-build
HERE=$(cd "$(dirname "$0")" && pwd)

sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends \
  build-essential git autoconf automake libtool pkg-config xxd \
  libpopt-dev libconfig-dev libasound2-dev avahi-daemon libavahi-client-dev libssl-dev \
  libsoxr-dev libplist-dev libsodium-dev libgcrypt20-dev uuid-dev \
  libavutil-dev libavcodec-dev libavformat-dev libswresample-dev \
  systemd-dev

mkdir -p "$WORK"
cd "$WORK"

if [ ! -d nqptp ]; then git clone --quiet https://github.com/mikebrady/nqptp.git; fi
cd nqptp
git fetch --quiet --tags
[ "$(git rev-parse "refs/tags/$NQPTP_VERSION^{commit}")" = "$NQPTP_COMMIT" ] || { echo "nqptp $NQPTP_VERSION moved"; exit 1; }
git checkout --quiet "$NQPTP_COMMIT"
autoreconf -fi >/dev/null
./configure --with-systemd-startup >/dev/null
make -j"$(nproc)" >/dev/null
sudo make install >/dev/null
cd ..

if [ ! -d shairport-sync ]; then git clone --quiet https://github.com/mikebrady/shairport-sync.git; fi
cd shairport-sync
git fetch --quiet --tags
[ "$(git rev-parse "refs/tags/$SHAIRPORT_VERSION^{commit}")" = "$SHAIRPORT_COMMIT" ] || { echo "shairport-sync $SHAIRPORT_VERSION moved"; exit 1; }
git checkout --quiet "$SHAIRPORT_COMMIT"
autoreconf -fi >/dev/null
./configure --sysconfdir=/etc --with-alsa --with-soxr --with-avahi --with-ssl=openssl \
  --with-systemd --with-airplay-2 --with-metadata >/dev/null
make -j"$(nproc)" >/dev/null
sudo make install >/dev/null

sudo install -D -m 644 "$HERE/shairport-restart.conf" /etc/systemd/system/shairport-sync.service.d/ownbox-restart.conf
if ! systemd-detect-virt --quiet --chroot; then
  sudo systemctl daemon-reload
  sudo systemctl enable --now nqptp
else
  # Building the SD card image, in a chroot: enabled, it starts at the box's first boot.
  sudo systemctl enable nqptp shairport-sync
fi
echo "built: $(shairport-sync -V)"
