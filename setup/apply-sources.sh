#!/bin/bash
# Writes the AirPlay (shairport-sync) and Music Assistant (squeezelite) settings from the
# box's own: its name (prefs.json, set in the wizard), its sound card (config.toml) and
# whether each source is on (prefs.json). Run as root by ownbox-sources.path whenever
# prefs.json changes; a service is restarted only when its settings really changed.
set -euo pipefail

STATE=/home/@USER@/.local/state/ownbox/prefs.json
CONFIG=/home/@USER@/.config/ownbox/config.toml

IFS=$'\t' read -r NAME DEVICE AIRPLAY MA < <(python3 - "$STATE" "$CONFIG" <<'EOF'
import json, sys, tomllib
try:
    prefs = json.load(open(sys.argv[1]))
except Exception:
    prefs = {}
try:
    config = tomllib.load(open(sys.argv[2], "rb"))
except Exception:
    config = {}
name = (prefs.get("box_name") or config.get("device_name") or "own.audio box").replace('"', "'")
def best_card():
    # As ownbox picks (src/hardware.rs): an I2S DAC, then USB, then HDMI, then the jack.
    rank = {"i2s": 0, "usb": 1, "hdmi": 2, "jack": 3}
    found = []
    try:
        lines = open("/proc/asound/cards").read().splitlines()
    except Exception:
        lines = []
    for line in lines:
        if "[" not in line or "]" not in line or " - " not in line:
            continue
        cid = line.split("[", 1)[1].split("]", 1)[0].strip()
        driver = line.split("]", 1)[1].lstrip(":").strip().split(" - ", 1)[0]
        if driver == "USB-Audio":
            kind = "usb"
        elif driver.startswith("vc4-hdmi") or cid.startswith("vc4hdmi"):
            kind = "hdmi"
        elif cid == "Headphones" or driver.startswith("bcm2835"):
            kind = "jack"
        else:
            kind = "i2s"
        found.append((rank[kind], cid))
    return "plughw:CARD=%s" % min(found)[1] if found else "default"
# The output chosen in Settings, else config.toml's, else the best one found.
device = prefs.get("audio_device") or config.get("audio_device") or best_card()
device = device[len("alsa/"):] if device.startswith("alsa/") else device
on = lambda key: "yes" if prefs.get(key, True) is not False else "no"
# Fields separated by tabs: the name may have spaces.
print("\t".join([name.replace("\t", " "), device, on("airplay_enabled"), on("music_assistant_enabled")]))
EOF
)

# Puts $2 into file $1 when it differs; says whether it did.
replace() {
  if [ -f "$1" ] && cmp -s "$1" <(printf '%s' "$2"); then return 1; fi
  printf '%s' "$2" > "$1"
}

# A source counts once its service is there (a build installs the program before the unit).
installed() { systemctl cat "$1" >/dev/null 2>&1; }

hook() { echo "/usr/bin/curl -fsS -m 5 -X POST http://127.0.0.1:8686/api/outside/$1/$2"; }

AIRPLAY_CONF="general = {
  name = \"$NAME\";
  output_backend = \"alsa\";
  interpolation = \"soxr\";
};
sessioncontrol = {
  run_this_before_entering_active_state = \"$(hook airplay start)\";
  run_this_after_exiting_active_state = \"$(hook airplay stop)\";
  wait_for_completion = \"yes\";
  allow_session_interruption = \"yes\";
};
alsa = {
  output_device = \"$DEVICE\";
};
metadata = {
  enabled = \"yes\";
  include_cover_art = \"yes\";
  pipe_name = \"/tmp/shairport-sync-metadata\";
};
"

if installed shairport-sync.service; then
  if replace /etc/shairport-sync.conf "$AIRPLAY_CONF" && [ "$AIRPLAY" = yes ]; then
    systemctl restart shairport-sync.service || true
  fi
  if [ "$AIRPLAY" = yes ]; then systemctl enable --now --quiet shairport-sync.service; else systemctl disable --now --quiet shairport-sync.service; fi
fi

# squeezelite: -C closes the card after 2 idle seconds so own.audio and AirPlay can have it.
MA_CONF="SL_NAME=\"$NAME\"
SL_SOUNDCARD=\"$DEVICE\"
SB_EXTRA_ARGS=\"-C 2\"
"

if installed squeezelite.service; then
  if replace /etc/default/squeezelite "$MA_CONF" && [ "$MA" = yes ]; then
    systemctl restart squeezelite.service || true
  fi
  if [ "$MA" = yes ]; then systemctl enable --now --quiet squeezelite.service; else systemctl disable --now --quiet squeezelite.service; fi
fi
