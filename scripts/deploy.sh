#!/bin/bash
# Builds ownbox for the Pi on the Mac and installs it there with its services.
# Usage: scripts/deploy.sh [ssh-host]   (default: rpi)
# ownbox runs as the user you log in as over SSH.
# Needs: zig, cargo-zigbuild, rustup target aarch64-unknown-linux-gnu.
set -euo pipefail
host="${1:-rpi}"
cd "$(dirname "$0")/.."

# glibc 2.36 as the floor: the Pi has 2.41 (Trixie), and Bookworm would still run it.
cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.36
bin=target/aarch64-unknown-linux-gnu/release/ownbox

user="$(ssh "$host" id -un)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
for f in setup/ownbox.service setup/ownbox-kiosk.service setup/blank-cursor.py setup/boot-config.sh setup/install-update.sh setup/ownbox-update.path setup/ownbox-update.service setup/ownbox-recover.service setup/50-ownbox-network.rules setup/ownbox-journald.conf setup/ownbox-wifi-powersave.conf setup/ownbox-brcmfmac.conf setup/51-ownbox-sources.rules setup/apply-sources.sh setup/ownbox-sources.path setup/ownbox-sources.service setup/airplay/shairport-restart.conf setup/squeezelite.service setup/dnsmasq-ownbox-setup.conf; do
  sed "s/@USER@/$user/g" "$f" > "$stage/$(basename "$f")"
done
scp -q "$bin" "$stage"/* "$host:/tmp/"
ssh "$host" 'set -e
  sudo install -d /opt/ownbox
  sudo install -m 644 /tmp/50-ownbox-network.rules /etc/polkit-1/rules.d/50-ownbox-network.rules
  # Outside sources (AirPlay, Music Assistant): their settings follow the box name and card.
  sudo install -m 644 /tmp/51-ownbox-sources.rules /etc/polkit-1/rules.d/51-ownbox-sources.rules
  sudo install -m 755 /tmp/apply-sources.sh /opt/ownbox/apply-sources.sh 2>/dev/null || { sudo install -d /opt/ownbox; sudo install -m 755 /tmp/apply-sources.sh /opt/ownbox/apply-sources.sh; }
  sudo install -m 644 /tmp/ownbox-sources.path /tmp/ownbox-sources.service /etc/systemd/system/
  # Updates from GitHub Releases: ownbox asks, this root unit installs (or goes back).
  sudo install -m 755 /tmp/install-update.sh /opt/ownbox/install-update.sh
  sudo install -m 644 /tmp/ownbox-update.path /tmp/ownbox-update.service /tmp/ownbox-recover.service /etc/systemd/system/
  # cloud-init set the box up on its first boot; after that it only cost ~8 s per boot.
  [ -e /etc/cloud/cloud-init.disabled ] || sudo touch /etc/cloud/cloud-init.disabled
  if ! cmp -s /tmp/ownbox-journald.conf /etc/systemd/journald.conf.d/ownbox.conf; then
    sudo install -D -m 644 /tmp/ownbox-journald.conf /etc/systemd/journald.conf.d/ownbox.conf
    sudo systemctl restart systemd-journald
  fi
  sudo install -D -m 644 /tmp/dnsmasq-ownbox-setup.conf /etc/NetworkManager/dnsmasq-shared.d/ownbox-setup.conf
  if ! cmp -s /tmp/ownbox-wifi-powersave.conf /etc/NetworkManager/conf.d/ownbox-wifi-powersave.conf; then
    sudo install -D -m 644 /tmp/ownbox-wifi-powersave.conf /etc/NetworkManager/conf.d/ownbox-wifi-powersave.conf
    sudo nmcli general reload conf || true
  fi
  # The Wi-Fi chip's own roaming off; the driver reads it at the next boot.
  sudo install -D -m 644 /tmp/ownbox-brcmfmac.conf /etc/modprobe.d/ownbox-brcmfmac.conf
  # Raspberry Pi OS keeps Wi-Fi blocked until a country is set (regulatory domain).
  [ "$(sudo raspi-config nonint get_wifi_country 2>/dev/null)" = "${WIFI_COUNTRY:-CZ}" ] || sudo raspi-config nonint do_wifi_country "${WIFI_COUNTRY:-CZ}"
  [ -e /opt/ownbox/cursors/default/cursors/default ] || sudo python3 /tmp/blank-cursor.py /opt/ownbox/cursors
  # Phone control needs the LAN; boxes set up before it listened on themselves only.
  # And IPv6 too: an iPhone may try the box over IPv6 first and never fall back.
  sed -i -E "s/^listen = \"(127\.0\.0\.1|0\.0\.0\.0):8686\"$/listen = \"[::]:8686\"/" ~/.config/ownbox/config.toml 2>/dev/null || true
  sudo install -m 755 /tmp/ownbox /opt/ownbox/ownbox
  sudo install -m 644 /tmp/ownbox.service /tmp/ownbox-kiosk.service /etc/systemd/system/
  # What differs by build (phase 8): config.txt block, CPU cap, the kiosk scale.
  sudo install -m 755 /tmp/boot-config.sh /opt/ownbox/boot-config.sh
  if [ "$(sudo /opt/ownbox/boot-config.sh)" = reboot ]; then
    echo "config.txt changed (previous kept as config.txt.ownbox-previous): takes effect at the next reboot"
  fi
  # Only where AirPlay was built (setup/airplay/build.sh).
  if [ -x /usr/local/bin/shairport-sync ]; then
    sudo install -D -m 644 /tmp/shairport-restart.conf /etc/systemd/system/shairport-sync.service.d/ownbox-restart.conf
  fi
  # Music Assistant: the Debian package, run by our own unit (see the comment in it).
  dpkg -s squeezelite >/dev/null 2>&1 || sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends squeezelite
  squeezelite_new=no
  cmp -s /tmp/squeezelite.service /etc/systemd/system/squeezelite.service || squeezelite_new=yes
  sudo install -m 644 /tmp/squeezelite.service /etc/systemd/system/squeezelite.service
  sudo systemctl daemon-reload
  [ "$squeezelite_new" = no ] || sudo systemctl try-restart squeezelite.service
  sudo systemctl enable --quiet ownbox.service ownbox-kiosk.service
  sudo systemctl enable --now --quiet ownbox-sources.path
  sudo systemctl enable --quiet ownbox-sources.service
  sudo systemctl enable --now --quiet ownbox-update.path
  sudo systemctl start ownbox-sources.service
  sudo systemctl restart ownbox.service
  sudo systemctl restart ownbox-kiosk.service
  sleep 2
  echo "ownbox: $(systemctl is-active ownbox.service)"
  echo "$(vcgencmd get_throttled)"'
