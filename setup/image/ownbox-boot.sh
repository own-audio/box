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
  if grep -qsx connected /sys/class/drm/card*-*/status; then screen=true; break; fi
  sleep 1
done
headless=$([ "$screen" = true ] && echo false || echo true)
if grep -q '^headless *=' "$CONFIG"; then
  sed -i "s/^headless *=.*/headless = $headless/" "$CONFIG"
else
  echo "headless = $headless" >> "$CONFIG"
fi
echo "screen: $screen"

if [ -z "$(raspi-config nonint get_wifi_country 2>/dev/null)" ]; then
  rfkill unblock wifi || true
fi

# cloud-init has done its one job (Imager's settings) once a boot has finished it; it would
# only cost seconds and writes at every boot after. (Its status says "running" this early.)
if [ ! -e /etc/cloud/cloud-init.disabled ] && [ -e /var/lib/cloud/instance/boot-finished ]; then
  touch /etc/cloud/cloud-init.disabled
fi

if [ "$(/opt/ownbox/boot-config.sh)" = reboot ]; then
  echo "config.txt written for $(tr -d '\0' </proc/device-tree/model); rebooting once"
  systemctl reboot
fi
