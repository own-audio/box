#!/bin/bash
# The radio's first boot: installs ownbox without a screen from what scripts/prepare-sd.sh
# put on the boot partition, then reboots into it. Run once by cloud-init (user-data).
set -euo pipefail
exec >>/var/log/ownbox-firstboot.log 2>&1
echo "== $(date -Is) first boot"
here=/boot/firmware/ownbox

# The Wi-Fi may come up a little after cloud-init's network wait.
for i in $(seq 1 20); do
  apt-get update && break
  echo "apt-get update failed ($i), waiting"; sleep 15
done
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends mpv avahi-daemon polkitd squeezelite

install -d /opt/ownbox
install -m 755 "$here/ownbox" /opt/ownbox/ownbox
install -m 755 "$here/boot-config.sh" /opt/ownbox/boot-config.sh
install -m 755 "$here/install-update.sh" /opt/ownbox/install-update.sh
install -m 644 "$here/ownbox.service" "$here/ownbox-update.path" "$here/ownbox-update.service" "$here/ownbox-recover.service" /etc/systemd/system/
install -m 644 "$here/50-ownbox-network.rules" /etc/polkit-1/rules.d/50-ownbox-network.rules
install -D -m 644 "$here/ownbox-journald.conf" /etc/systemd/journald.conf.d/ownbox.conf
install -m 644 "$here/squeezelite.service" "$here/ownbox-sources.path" "$here/ownbox-sources.service" /etc/systemd/system/
install -m 644 "$here/51-ownbox-sources.rules" /etc/polkit-1/rules.d/51-ownbox-sources.rules
install -m 755 "$here/apply-sources.sh" /opt/ownbox/apply-sources.sh
install -D -m 644 "$here/ownbox-wifi-powersave.conf" /etc/NetworkManager/conf.d/ownbox-wifi-powersave.conf
install -D -m 644 "$here/ownbox-brcmfmac.conf" /etc/modprobe.d/ownbox-brcmfmac.conf
if [ ! -e /home/@USER@/.config/ownbox/config.toml ]; then
  install -D -o @USER@ -g @USER@ -m 644 "$here/config.toml" /home/@USER@/.config/ownbox/config.toml
  chown -R @USER@:@USER@ /home/@USER@/.config
fi

raspi-config nonint do_wifi_country @WIFI_COUNTRY@ || true
/opt/ownbox/boot-config.sh || echo "boot-config failed"
# cloud-init has done its job; it would cost seconds on every boot.
touch /etc/cloud/cloud-init.disabled

systemctl daemon-reload
systemctl enable ownbox.service ownbox-update.path ownbox-sources.path ownbox-sources.service squeezelite.service
rm -rf "$here"
echo "== $(date -Is) done, rebooting"
systemctl reboot
