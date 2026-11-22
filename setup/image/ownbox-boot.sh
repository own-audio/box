#!/bin/bash
# Every boot of a box made from the SD card image (setup/image/build.sh), before ownbox:
#   - a screen connected or not: the kiosk runs, or the box is headless (a phone is its
#     screen, and the first phone to open it pairs without a code);
#   - Wi-Fi unblocked when no country was set (Raspberry Pi OS keeps it blocked until
#     one is; the world regulatory domain applies until then);
#   - boot-config.sh for this board: the config.txt block, the CPU cap, the kiosk scale.
#     The first boot changes config.txt and reboots once.
set -uo pipefail
CONFIG=/home/@USER@/.config/ownbox/config.toml

# The display driver may still be probing the panel this early.
udevadm settle --timeout=10 || true
screen=false
for i in 1 2 3 4 5; do
  if timeout 5 grep -qsx connected /sys/class/drm/card*-*/status; then screen=true; break; fi
  sleep 1
done
headless=$([ "$screen" = true ] && echo false || echo true)
# Root writes in a folder ownbox owns: never through a link put there, and only when the
# value changes (each write starts apply-sources.sh through its path unit).
if [ -L "$CONFIG" ] || [ ! -f "$CONFIG" ]; then
  echo "config.toml is not a plain file; leaving it"
elif ! grep -qx "headless = $headless" "$CONFIG"; then
  new=$(mktemp "$CONFIG.XXXXXX")
  grep -v '^headless *=' "$CONFIG" > "$new" || true
  echo "headless = $headless" >> "$new"
  chown --reference="$CONFIG" "$new"; chmod --reference="$CONFIG" "$new"
  mv "$new" "$CONFIG"
fi
echo "screen: $screen"

if [ -z "$(raspi-config nonint get_wifi_country 2>/dev/null)" ]; then
  rfkill unblock wifi || true
fi

# cloud-init has one job, Imager's settings at the first start; at the second start it is
# switched off. Not only when it says it finished: on the Pi 4 it never did, and from a
# plain power supply every start waited minutes on cloud-init-local before going on.
# (Its status says "running" this early, so the first start only leaves a mark.)
if [ ! -e /etc/cloud/cloud-init.disabled ]; then
  if [ -e /var/lib/ownbox/started-once ] || [ -e /var/lib/cloud/instance/boot-finished ]; then
    touch /etc/cloud/cloud-init.disabled
  else
    install -d /var/lib/ownbox && touch /var/lib/ownbox/started-once
  fi
fi

if [ "$(/opt/ownbox/boot-config.sh)" = reboot ]; then
  echo "config.txt written for $(tr -d '\0' </proc/device-tree/model); rebooting once"
  systemctl reboot
fi
