#!/bin/bash
# Builds the own.audio box SD card image: Raspberry Pi OS Lite (64-bit) with ownbox, mpv,
# the kiosk (cage + Chromium), AirPlay 2 and Squeezelite installed, for every supported
# board (Zero 2 W, 3, 4, 5). Each boot, ownbox-boot.sh looks at the Pi: a screen means the
# kiosk, none means a headless box run from a phone.
#
# Run as root on 64-bit ARM Linux (GitHub's ubuntu-24.04-arm runner): the chroot then runs
# natively, no emulation.
# Usage: sudo setup/image/build.sh <ownbox binary> <version> <output .img.xz>
set -euo pipefail

BASE_URL=https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2026-10-06/2026-10-06-raspios-trixie-arm64-lite.img.xz
BASE_SHA256=483db18a48da399b5b7022ffae9a07bc6d99daab30e593aedac09b69ff422843
# ownbox runs as its own system user; Raspberry Pi Imager's settings (if any) add a
# login user for SSH beside it.
BOX_USER=ownbox
GROW=2500M

bin=$(realpath "${1:?ownbox binary}")
version=${2:?version}
out=$(realpath -m "${3:?output .img.xz}")
here=$(cd "$(dirname "$0")" && pwd)
setup=$(dirname "$here")
[ "$(id -u)" = 0 ] || { echo "run as root"; exit 1; }
[ "$(uname -m)" = aarch64 ] || { echo "run on 64-bit ARM"; exit 1; }

work=$(mktemp -d /var/tmp/ownbox-image.XXXX)
root=$work/root
loop=
cleanup() {
  set +e
  for m in boot/firmware dev/pts dev proc sys; do mountpoint -q "$root/$m" && umount "$root/$m"; done
  mountpoint -q "$root" && umount "$root"
  [ -n "$loop" ] && losetup -d "$loop"
  rm -rf "$work"
}
trap cleanup EXIT

echo "== base image"
curl -fsSL -o "$work/base.img.xz" "$BASE_URL"
echo "$BASE_SHA256  $work/base.img.xz" | sha256sum -c -
xz -dc "$work/base.img.xz" > "$work/box.img"
rm "$work/base.img.xz"
truncate -s "+$GROW" "$work/box.img"
parted -s "$work/box.img" resizepart 2 100%
loop=$(losetup -Pf --show "$work/box.img")
e2fsck -pf "${loop}p2" || [ $? -le 1 ]
resize2fs "${loop}p2"
mkdir -p "$root"
mount "${loop}p2" "$root"
mount "${loop}p1" "$root/boot/firmware"
for m in dev dev/pts proc sys; do mount --bind "/$m" "$root/$m"; done

in_root() { chroot "$root" /usr/bin/env DEBIAN_FRONTEND=noninteractive LC_ALL=C.UTF-8 "$@"; }
# Nothing starts inside the chroot.
printf '#!/bin/sh\nexit 101\n' > "$root/usr/sbin/policy-rc.d"
chmod 755 "$root/usr/sbin/policy-rc.d"
resolv_backup=$work/resolv.conf
cp -a "$root/etc/resolv.conf" "$resolv_backup" 2>/dev/null || true
rm -f "$root/etc/resolv.conf"
cp /etc/resolv.conf "$root/etc/resolv.conf"

echo "== packages"
in_root apt-get update -q
in_root apt-get install -y -q --no-install-recommends \
  mpv avahi-daemon polkitd squeezelite cage chromium fonts-dejavu-core python3 rfkill dnsmasq-base unattended-upgrades wlr-randr

echo "== AirPlay 2 (built from source)"
cp -r "$setup/airplay" "$root/var/tmp/ownbox-airplay"
in_root bash /var/tmp/ownbox-airplay/build.sh
# The build tools go again. The libraries the two programs link to came in as the -dev
# packages' dependencies, so they are marked as wanted first, or autoremove takes them too.
cat > "$root/var/tmp/keep-libs.sh" <<'KEEP'
for lib in $(ldd /usr/local/bin/shairport-sync /usr/local/bin/nqptp | awk '/=> \//{print $3}' | sort -u); do
  real=$(readlink -f "$lib")
  dpkg -S "$real" 2>/dev/null || dpkg -S "/usr$real" 2>/dev/null || dpkg -S "${real#/usr}" 2>/dev/null
done | cut -d: -f1 | sort -u | xargs apt-mark manual
KEEP
in_root sh /var/tmp/keep-libs.sh
in_root apt-get purge -y -q build-essential git autoconf automake libtool pkg-config xxd \
  libpopt-dev libconfig-dev libasound2-dev libavahi-client-dev libssl-dev libsoxr-dev \
  libplist-dev libsodium-dev libgcrypt20-dev uuid-dev libavutil-dev libavcodec-dev \
  libavformat-dev libswresample-dev systemd-dev
in_root apt-get autoremove -y -q --purge
rm -rf "$root/var/tmp/ownbox-airplay" "$root/var/tmp/ownbox-airplay-build"
if in_root ldd /usr/local/bin/shairport-sync /usr/local/bin/nqptp | grep 'not found'; then
  echo "AirPlay lost a library with the build tools"; exit 1
fi

echo "== ownbox $version"
in_root useradd --system --create-home --home-dir "/home/$BOX_USER" --shell /bin/bash \
  --groups audio,video,input,render,netdev,gpio,i2c,spi,plugdev "$BOX_USER"
fill() { sed "s/@USER@/$BOX_USER/g" "$1"; }
install -d "$root/opt/ownbox"
install -m 755 "$bin" "$root/opt/ownbox/ownbox"
for f in boot-config.sh install-update.sh apply-sources.sh; do
  fill "$setup/$f" > "$root/opt/ownbox/$f"; chmod 755 "$root/opt/ownbox/$f"
done
fill "$here/ownbox-boot.sh" > "$root/opt/ownbox/ownbox-boot.sh"; chmod 755 "$root/opt/ownbox/ownbox-boot.sh"
for f in ownbox.service ownbox-kiosk.service ownbox-update.path ownbox-update.service ownbox-recover.service \
  ownbox-sources.path ownbox-sources.service squeezelite.service; do
  fill "$setup/$f" > "$root/etc/systemd/system/$f"
done
install -m 644 "$here/ownbox-boot.service" "$root/etc/systemd/system/ownbox-boot.service"
fill "$setup/50-ownbox-network.rules" > "$root/etc/polkit-1/rules.d/50-ownbox-network.rules"
fill "$setup/51-ownbox-sources.rules" > "$root/etc/polkit-1/rules.d/51-ownbox-sources.rules"
install -D -m 644 "$setup/ownbox-journald.conf" "$root/etc/systemd/journald.conf.d/ownbox.conf"
install -D -m 644 "$setup/ownbox-wifi-powersave.conf" "$root/etc/NetworkManager/conf.d/ownbox-wifi-powersave.conf"
install -D -m 644 "$setup/ownbox-brcmfmac.conf" "$root/etc/modprobe.d/ownbox-brcmfmac.conf"
install -D -m 644 "$setup/dnsmasq-ownbox-setup.conf" "$root/etc/NetworkManager/dnsmasq-shared.d/ownbox-setup.conf"
install -D -o 0 -g 0 -m 644 "$here/config.toml" "$root/home/$BOX_USER/.config/ownbox/config.toml"
in_root chown -R "$BOX_USER:$BOX_USER" "/home/$BOX_USER"
in_root python3 /dev/stdin /opt/ownbox/cursors < "$setup/blank-cursor.py"
# The kiosk's address; ownbox-boot.sh rewrites it at every boot.
printf 'KIOSK_URL=http://localhost:8686\n' > "$root/etc/default/ownbox-kiosk"

echo "== services"
in_root systemctl enable ownbox-boot.service ownbox.service ownbox-kiosk.service \
  ownbox-update.path ownbox-sources.path ownbox-sources.service squeezelite.service avahi-daemon.service
in_root systemctl set-default graphical.target
in_root systemctl disable getty@tty1.service
# The box asks nothing on its console: no first-boot user prompt (Imager's settings, when
# given, still create a login user through cloud-init).
in_root systemctl mask userconfig.service 2>/dev/null || true
# Nor systemd's own first-boot wizard: on a box with a screen it waited on the console for
# a keyboard (locale, keymap, time zone, root password), and the kiosk never came.
in_root systemctl mask systemd-firstboot.service
# And told so on the kernel's command line too, should a later base image unmask it.
sed -i '1s/$/ systemd.firstboot=off/' "$root/boot/firmware/cmdline.txt"
# Fewer writes to the SD card: no daily package-list updates, manual-page index or dpkg
# backups (the box updates itself from GitHub, not apt), and zram's swap is never written
# back to the card. The journal already lives in memory (Raspberry Pi OS's default).
in_root systemctl mask apt-daily.timer apt-daily-upgrade.timer apt-listchanges.timer \
  man-db.timer dpkg-db-backup.timer rpi-zram-writeback.timer
# NetworkManager keeps the Wi-Fi switched off on a fresh Raspberry Pi OS; the box needs it
# for its own setup Wi-Fi before anyone has chosen a country.
printf '[main]\nNetworkingEnabled=true\nWirelessEnabled=true\nWWANEnabled=true\n' \
  > "$root/var/lib/NetworkManager/NetworkManager.state"
echo ownbox > "$root/etc/hostname"
sed -i 's/^127\.0\.1\.1.*/127.0.1.1\townbox/' "$root/etc/hosts"
printf 'own.audio box %s (Raspberry Pi OS Lite 2026-10-06, arm64)\n' "$version" > "$root/etc/ownbox-image"

# Beta images (a version with a pre-release part, 1.0.0-beta.4): a root shell over USB, so
# a box that never reaches a network can still be looked into. The Zero 2 W's data port
# (or a Pi 4's USB-C) to a computer shows up as a serial port there (/dev/cu.usbmodem…,
# /dev/ttyACM0). OTG mode, not peripheral: the same port still takes a USB sound card.
# Only on those two: on a Pi 3 the same USB controller carries its whole hub — the
# Ethernet and every USB port — and the overlay took them away (the Pi 3 then hung at
# cloud-init's "pre-network" stage). The Pi 5 is left out until tried.
# Anyone with the box in hand can read its SD card anyway; a release leaves this out.
case "$version" in
  *-*)
    echo "== serial console (beta)"
    sed -i '1s/$/ modules-load=dwc2,g_serial systemd.debug_shell=ttyGS0/' "$root/boot/firmware/cmdline.txt"
    printf '\n# own.audio box beta: a root shell over USB (g_serial on ttyGS0); releases leave it out\n[pi02]\ndtoverlay=dwc2\n[pi4]\ndtoverlay=dwc2\n[all]\n' \
      >> "$root/boot/firmware/config.txt"
    ;;
esac

echo "== slimming"
# What a box never reads: manual pages, package docs (the copyright files stay) and the
# languages other than Czech and English. dpkg is told too, so the box's own updates of
# these packages do not bring them back.
cat > "$root/etc/dpkg/dpkg.cfg.d/ownbox-slim" <<'SLIM'
path-exclude /usr/share/man/*
path-exclude /usr/share/doc/*
path-include /usr/share/doc/*/copyright
path-exclude /usr/share/info/*
path-exclude /usr/share/locale/*
path-include /usr/share/locale/cs/*
path-include /usr/share/locale/en*/*
path-include /usr/share/locale/locale.alias
SLIM
rm -rf "$root"/usr/share/man/* "$root"/usr/share/info/*
find "$root/usr/share/doc" -mindepth 1 ! -name copyright ! -type d -delete 2>/dev/null || true
find "$root/usr/share/doc" -mindepth 1 -type d -empty -delete 2>/dev/null || true
find "$root/usr/share/locale" -mindepth 1 -maxdepth 1 -type d ! -name cs ! -name 'en*' -exec rm -rf {} + 2>/dev/null || true
# Packages a box never uses: firmware for Wi-Fi chips no Raspberry Pi carries, the
# compiler and kernel headers left over from AirPlay's build, Raspberry Pi Connect and
# mkvtoolnix. apt lists what goes with them, so a removal that takes too much shows.
# (apt takes no wildcards, hence the names from dpkg; gcc-14-base stays, libgcc needs it.)
drop=$(in_root dpkg-query -Wf '${Package}\n' \
  | grep -E '^(firmware-(atheros|mediatek|realtek)|(gcc|cpp)-14(-aarch64-linux-gnu|-for-host)?|linux-headers-.*|rpi-connect-lite|mkvtoolnix)$' || true)
if [ -n "$drop" ]; then
  # shellcheck disable=SC2086
  echo "dropping: $(echo $drop)"
  if in_root apt-get purge -y -q --autoremove $drop > "$work/purge.log" 2>&1; then
    grep -E '^Removing ' "$work/purge.log" | sed 's/^/  /'
  else
    echo "apt could not drop them:"; tail -20 "$work/purge.log"
  fi
fi
for must in mpv chromium cage squeezelite avahi-daemon network-manager raspi-firmware; do
  in_root dpkg -s "$must" >/dev/null 2>&1 || { echo "slimming removed $must"; exit 1; }
done
# The 25 biggest packages, so the next one to drop is easy to find.
# (awk, not head: head closing the pipe early fails the build under pipefail.)
in_root dpkg-query -Wf '${Installed-Size}\t${Package}\n' | sort -rn \
  | awk -F'\t' 'NR <= 25 {printf "  %6.1f MB  %s\n", $1/1024, $2}'

echo "== cleaning up"
in_root apt-get clean
rm -rf "$root"/var/lib/apt/lists/* "$root"/var/log/apt/* "$root"/tmp/* "$root"/var/tmp/*
rm -f "$root/usr/sbin/policy-rc.d" "$root/etc/resolv.conf"
cp -a "$resolv_backup" "$root/etc/resolv.conf" 2>/dev/null || true
du -shx "$root" | sed 's/^/root file system: /'
for m in boot/firmware dev/pts dev proc sys; do umount "$root/$m"; done
umount "$root"
e2fsck -pf "${loop}p2" || [ $? -le 1 ]
# The file system shrinks back to what it holds plus a little room: Imager then writes a
# smaller image, and Raspberry Pi OS grows it to the whole card at the first boot
# ("resize" in cmdline.txt). The build needed the extra room only for AirPlay's compile.
resize2fs -M "${loop}p2"
bs=$(dumpe2fs -h "${loop}p2" 2>/dev/null | awk -F: '/^Block size/{gsub(/ /,"",$2); print $2}')
blocks=$(dumpe2fs -h "${loop}p2" 2>/dev/null | awk -F: '/^Block count/{gsub(/ /,"",$2); print $2}')
blocks=$(( blocks + 256 * 1024 * 1024 / bs ))
resize2fs "${loop}p2" "$blocks"
e2fsck -pf "${loop}p2" || [ $? -le 1 ]
# Free blocks to zeros, so they compress to nothing.
zerofree "${loop}p2"
losetup -d "$loop"; loop=
start=$(parted -ms "$work/box.img" unit s print | awk -F: '$1 == 2 { sub(/s$/, "", $2); print $2 }')
sectors=$(( blocks * bs / 512 ))
[ -n "$start" ] || { echo "no second partition"; exit 1; }
echo "$start, $sectors" | sfdisk --no-reread --no-tell-kernel -N 2 "$work/box.img"
truncate -s $(( (start + sectors) * 512 )) "$work/box.img"
ls -lh "$work/box.img" | awk '{print "image: " $5}'

echo "== compressing"
xz -T0 -9 -c "$work/box.img" > "$out"
( cd "$(dirname "$out")" && sha256sum "$(basename "$out")" > "$(basename "$out").sha256" )
ls -lh "$out"
