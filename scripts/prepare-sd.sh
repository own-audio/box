#!/bin/bash
# Prepares a freshly flashed Raspberry Pi OS Lite card (scripts/flash-sd.sh) as the radio:
# the headless box that installs itself on first boot (setup/radio/firstboot.sh).
# Usage: WIFI_SSID=… WIFI_PASSWORD=… scripts/prepare-sd.sh [/Volumes/bootfs]
# Optional: OWNBOX_USER (default: your login here), SSH_KEY_FILE (~/.ssh/id_ed25519.pub),
# WIFI_COUNTRY (CZ), TIMEZONE (this Mac's), OWNBOX_SERVER (otherwise asked on the phone).
# The Wi-Fi password goes onto the card only, never into the repository.
set -euo pipefail
boot="${1:-/Volumes/bootfs}"
key="${SSH_KEY_FILE:-$HOME/.ssh/id_ed25519.pub}"
user="${OWNBOX_USER:-$USER}"
country="${WIFI_COUNTRY:-CZ}"
tz="${TIMEZONE:-$(readlink /etc/localtime | sed 's|.*/zoneinfo/||')}"
[ -f "$key" ] || { echo "No SSH key at $key (set SSH_KEY_FILE)"; exit 1; }
[[ "$user" =~ ^[a-z_][a-z0-9_-]*$ ]] || { echo "OWNBOX_USER must be a Linux user name"; exit 1; }
[[ "$country" =~ ^[A-Z]{2}$ ]] || { echo "WIFI_COUNTRY must be two capital letters"; exit 1; }
: "${WIFI_SSID:?WIFI_SSID}" "${WIFI_PASSWORD:?WIFI_PASSWORD}"
cd "$(dirname "$0")/.."
[ -f "$boot/cmdline.txt" ] && [ -f "$boot/config.txt" ] || { echo "$boot is not a Pi boot partition"; exit 1; }

cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.36
bin=target/aarch64-unknown-linux-gnu/release/ownbox

fill() { sed -e "s|@USER@|$user|g" -e "s|@TIMEZONE@|$tz|g" -e "s|@WIFI_COUNTRY@|$country|g" "$@"; }
fill -e "s|SSH_KEY|$(cat "$key")|" setup/radio/user-data > "$boot/user-data"
# JSON strings are valid YAML; they keep quotes and backslashes in the password intact.
q() { python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"; }
cat > "$boot/network-config" <<EOF
network:
  version: 2
  wifis:
    renderer: NetworkManager
    wlan0:
      dhcp4: true
      regulatory-domain: "$country"
      access-points:
        $(q "$WIFI_SSID"):
          password: $(q "$WIFI_PASSWORD")
      optional: true
EOF
grep -q 'cfg80211.ieee80211_regdom=' "$boot/cmdline.txt" \
  || sed -i '' "1s/\$/ cfg80211.ieee80211_regdom=$country/" "$boot/cmdline.txt"

mkdir -p "$boot/ownbox"
cp "$bin" "$boot/ownbox/"
for f in setup/ownbox.service setup/boot-config.sh setup/install-update.sh setup/ownbox-update.path \
  setup/ownbox-update.service setup/50-ownbox-network.rules setup/ownbox-journald.conf setup/ownbox-wifi-powersave.conf \
  setup/squeezelite.service setup/ownbox-sources.path setup/ownbox-sources.service setup/51-ownbox-sources.rules setup/apply-sources.sh \
  setup/radio/config.toml setup/radio/firstboot.sh; do
  fill "$f" > "$boot/ownbox/$(basename "$f")"
done
[ -z "${OWNBOX_SERVER:-}" ] || printf 'server_url = "%s"\n' "$OWNBOX_SERVER" >> "$boot/ownbox/config.toml"
sync
echo "Ready: $boot. Eject the card, put it in the radio, power it on."
echo "First boot installs everything and reboots (~10 min on a Zero 2 W); then open"
echo "http://ownbox-radio.local:8686/ on a phone: the first phone pairs itself."
