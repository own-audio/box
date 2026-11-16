#!/bin/bash
# Writes a Raspberry Pi OS image to an SD card on macOS.
# Usage: sudo scripts/flash-sd.sh <image.img.xz> <diskN>
# Refuses anything that is not an external, physical disk between 4 and 128 GB.
set -euo pipefail

image="${1:?image .img.xz}"
disk="${2:?disk identifier, e.g. disk8}"
disk="${disk#/dev/}"

[ "$(id -u)" -eq 0 ] || { echo "Run with sudo."; exit 1; }
[ -f "$image" ] || { echo "No such image: $image"; exit 1; }
[[ "$disk" =~ ^disk[0-9]+$ ]] || { echo "Give a whole disk like disk8, not a partition."; exit 1; }

info="$(diskutil info "$disk")"
grep -qE "Device Location: +External|Removable Media: +Removable" <<<"$info" \
  || { echo "$disk is not external/removable. Stopping."; exit 1; }
grep -qE "Virtual: +No" <<<"$info" || { echo "$disk is a virtual disk. Stopping."; exit 1; }
bytes="$(diskutil info -plist "$disk" | plutil -extract TotalSize raw -)"
gb=$((bytes / 1000000000))
(( gb >= 4 && gb <= 128 )) || { echo "$disk is ${gb} GB, outside 4–128 GB. Stopping."; exit 1; }

echo "Target: /dev/$disk, ${gb} GB"
grep -E "Device / Media Name|Protocol|Device Location" <<<"$info" || true
read -r -p "Erase it and write $(basename "$image")? Type YES: " answer
[ "$answer" = "YES" ] || { echo "Cancelled."; exit 1; }

diskutil unmountDisk "/dev/$disk"
xz -dc "$image" | dd of="/dev/r$disk" bs=4m status=progress
sync
diskutil mountDisk "/dev/$disk" || true
echo "Done. The boot partition should be mounted as /Volumes/bootfs."
