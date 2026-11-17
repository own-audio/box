#!/bin/bash
# Writes what differs between builds of the box, found on the Pi itself (phase 8):
#   - config.txt: one marked block (power button, Bluetooth off, an audio HAT's overlay),
#     replaced on every deploy, never edited by hand; takes effect at the next boot;
#   - the CPU cap for this board (tmpfiles, applied now);
#   - the kiosk's scale factor for this panel.
# Run as root by scripts/deploy.sh. Prints "reboot" when config.txt changed.
set -euo pipefail

# At boot ownbox-boot.sh and apply-sources.sh (started by the same boot's change to
# config.toml) can both run this; two writers of config.txt at once could leave neither's.
exec 9>/run/ownbox-boot-config.lock
flock 9

CONFIG_TXT=${CONFIG_TXT:-/boot/firmware/config.txt}
CONFIG_TOML=${CONFIG_TOML:-/home/@USER@/.config/ownbox/config.toml}
PREFS_JSON=${PREFS_JSON:-/home/@USER@/.local/state/ownbox/prefs.json}
BEGIN="# own.audio box begin: written by the deploy, edit config.toml instead"
END="# own.audio box end"

toml() {
  python3 - "$CONFIG_TOML" "$1" <<'EOF'
import sys, tomllib
try:
    print(str(tomllib.load(open(sys.argv[1], "rb")).get(sys.argv[2], "")).strip())
except Exception:
    print("")
EOF
}
# The sound card chosen on the box (Settings, the setup Wi-Fi) wins over config.toml's.
pref() {
  python3 - "$PREFS_JSON" "$1" <<'EOF'
import json, sys
try:
    v = json.load(open(sys.argv[1])).get(sys.argv[2])
    print("-" if v is None else str(v).strip())
except Exception:
    print("-")
EOF
}
HAT=$(pref audio_hat); [ "$HAT" = - ] && HAT=$(toml audio_hat)
BUTTONS=$(pref hat_buttons); [ "$BUTTONS" = - ] && BUTTONS=$(toml buttons)
case "$HAT" in *[!a-z0-9-]*) echo "audio_hat: not an overlay name: $HAT" >&2; exit 1 ;; esac
if [ -n "$HAT" ] && [ ! -e "/boot/firmware/overlays/$HAT.dtbo" ]; then
  echo "audio_hat: no overlay $HAT.dtbo" >&2; exit 1
fi

MODEL=$(tr -d '\0' < /proc/device-tree/model)

# A DAC on the GPIO header takes GPIO2/3 (I²C), where the power button sits on a bare Pi.
# A HAT with an EEPROM is found by the firmware (and shows in /proc/device-tree/hat); one
# without is named in audio_hat. With a DAC the button moves to GPIO17 for shutdown; GPIO3
# still wakes the Pi from halt, so one button wired to both does both (HARDWARE.md).
if [ "$BUTTONS" = phat-beat ]; then
  # The pHAT BEAT's own buttons (Pimoroni's pins, pressed = low): on/off shuts down, the
  # other five reach ownbox as media keys. Only GPIO3 wakes a halted Pi, so after a
  # shutdown the radio starts again by unplugging it.
  BUTTON="dtoverlay=gpio-shutdown,gpio_pin=12
dtoverlay=gpio-key,gpio=6,keycode=164,label=PLAYPAUSE
dtoverlay=gpio-key,gpio=5,keycode=163,label=NEXTSONG
dtoverlay=gpio-key,gpio=13,keycode=165,label=PREVIOUSSONG
dtoverlay=gpio-key,gpio=16,keycode=115,label=VOLUMEUP
dtoverlay=gpio-key,gpio=26,keycode=114,label=VOLUMEDOWN"
elif [ -n "$BUTTONS" ]; then
  echo "buttons: unknown set: $BUTTONS" >&2; exit 1
elif [ -n "$HAT" ] || [ -d /proc/device-tree/hat ]; then
  BUTTON="dtoverlay=gpio-shutdown,gpio_pin=17"
else
  BUTTON="dtoverlay=gpio-shutdown"
fi

BLOCK="$BEGIN
[all]
$BUTTON
dtoverlay=disable-bt"
if [ -n "$HAT" ]; then
  BLOCK="$BLOCK
dtparam=audio=off
dtoverlay=$HAT"
fi
BLOCK="$BLOCK
$END"

# The old config.txt minus our block, and minus the two lines set by hand before the block
# existed (they now live in it).
REST=$(awk -v b="$BEGIN" -v e="$END" '
  $0 == b { skip = 1; next }
  $0 == e { skip = 0; next }
  skip { next }
  $0 == "dtoverlay=gpio-shutdown" || $0 == "dtoverlay=disable-bt" { next }
  { print }' "$CONFIG_TXT")
NEW="$REST
$BLOCK"
if ! cmp -s "$CONFIG_TXT" <(printf '%s\n' "$NEW"); then
  cp "$CONFIG_TXT" "$CONFIG_TXT.ownbox-previous"
  printf '%s\n' "$NEW" > "$CONFIG_TXT.ownbox-new"
  # FAT has no journal: the new file is on the card before it replaces the old one, and the
  # rename is on the card before anyone reboots.
  sync "$CONFIG_TXT.ownbox-new"
  mv "$CONFIG_TXT.ownbox-new" "$CONFIG_TXT"
  sync
  echo reboot
fi

# CPU cap: the Pi 3 B+ is held to 800 MHz (its supply sags above that, measured); a Pi 4
# starts at 1.2 GHz until it is measured; anything else runs free.
case "$MODEL" in
  *"Pi 3"*) MAX=800000 ;;
  *"Pi 4"*) MAX=1200000 ;;
  *) MAX="" ;;
esac
# Files are written only when they change: this runs at every boot and after every change
# of the box's settings, and the SD card should not take a write each time.
write() { cmp -s "$1" <(printf '%s\n' "$2") || printf '%s\n' "$2" > "$1"; }
if [ -n "$MAX" ]; then
  write /etc/tmpfiles.d/ownbox-cpu.conf "$(printf '# own.audio box: CPU cap for %s (written by the deploy).\nw /sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq - - - - %s' "$MODEL" "$MAX")"
  systemd-tmpfiles --create /etc/tmpfiles.d/ownbox-cpu.conf
else
  rm -f /etc/tmpfiles.d/ownbox-cpu.conf
fi

# The UI is laid out for 800 × 480 points. Touch Display 2 (720 × 1280, landscape) gets a
# scale factor so the same layout fills it at finger size.
SCALE=1
for c in /sys/class/drm/card*-DSI-*; do
  [ "$(cat "$c/status" 2>/dev/null)" = connected ] || continue
  case "$(head -1 "$c/modes" 2>/dev/null)" in 720x1280|1280x720) SCALE=1.6 ;; esac
done
write /etc/default/ownbox-kiosk "$(printf 'KIOSK_URL=http://localhost:8686\nKIOSK_SCALE=%s' "$SCALE")"
