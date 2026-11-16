#!/bin/bash
# Installs the kiosk on the Pi from the Mac: Chromium full screen in cage on tty1.
# Usage: setup/install-kiosk.sh [ssh-host]   (default: rpi)
set -euo pipefail
host="${1:-rpi}"
here="$(cd "$(dirname "$0")" && pwd)"

user="$(ssh "$host" id -un)"
sed "s/@USER@/$user/g" "$here/ownbox-kiosk.service" > "${TMPDIR:-/tmp}/ownbox-kiosk.service"
scp -q "${TMPDIR:-/tmp}/ownbox-kiosk.service" "$here/placeholder.html" "$host:/tmp/"
ssh "$host" 'set -e
  sudo install -d /opt/ownbox
  sudo install -m 644 /tmp/placeholder.html /opt/ownbox/placeholder.html
  sudo install -m 644 /tmp/ownbox-kiosk.service /etc/systemd/system/ownbox-kiosk.service
  # The placeholder until ownbox serves the UI on localhost:8686 (phase 1).
  [ -f /etc/default/ownbox-kiosk ] || echo "KIOSK_URL=file:///opt/ownbox/placeholder.html" | sudo tee /etc/default/ownbox-kiosk >/dev/null
  sudo systemctl daemon-reload
  sudo systemctl set-default graphical.target
  sudo systemctl disable getty@tty1.service
  sudo systemctl enable ownbox-kiosk.service
  sudo systemctl restart ownbox-kiosk.service
  sleep 8
  systemctl is-active ownbox-kiosk.service'
