#!/bin/bash
# Writes the AirPlay (shairport-sync) and Music Assistant (squeezelite) settings from the
# box's own: its name (prefs.json, set in the wizard), its sound card (config.toml) and
# whether each source is on (prefs.json). Run as root by ownbox-sources.path whenever
# prefs.json changes; a service is restarted only when its settings really changed.
# Also the box's own settings that need root: its network name (on a box from the SD card
# image) and the sound card on the GPIO header, which goes into config.txt and takes a
# restart.
set -euo pipefail

STATE=/home/@USER@/.local/state/ownbox/prefs.json
CONFIG=/home/@USER@/.config/ownbox/config.toml

IFS=$'\t' read -r NAME DEVICE AIRPLAY MA HOST < <(python3 - "$STATE" "$CONFIG" <<'EOF'
import json, sys, tomllib
try:
    prefs = json.load(open(sys.argv[1]))
except Exception:
    prefs = {}
try:
    config = tomllib.load(open(sys.argv[2], "rb"))
except Exception:
    config = {}
import re
# The name lands in shairport-sync.conf and /etc/default/squeezelite inside quotes, and ownbox
# (not root) wrote it: control characters, quotes and backslashes go.
name = str(prefs.get("box_name") or config.get("device_name") or "own.audio box")
name = re.sub(r"[\x00-\x1f\x7f]+", " ", name).replace('"', "'").replace("\\", "/").replace("$", "").replace("`", "'").strip()[:60] or "own.audio box"
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
# An ALSA name only (plughw:CARD=sndrpihifiberry,DEV=0): anything else falls back.
if not re.fullmatch(r"[A-Za-z0-9_:=,.-]{1,80}", device):
    device = best_card()
on = lambda key: "yes" if prefs.get(key, True) is not False else "no"
# The network name from the name given on the box: "own.audio pirate radio" → pirate-radio
# (pirate-radio.local); "-" until one is given, so Imager's hostname stays until then.
import re, unicodedata
host = "-"
if prefs.get("box_name"):
    h = unicodedata.normalize("NFKD", prefs["box_name"]).encode("ascii", "ignore").decode().lower()
    h = re.sub(r"[^a-z0-9]+", "-", h.replace("own.audio", "")).strip("-")[:40].strip("-")
    host = "ownbox" if h in ("", "box") else h
# Fields separated by tabs: the name may have spaces.
print("\t".join([name.replace("\t", " "), device, on("airplay_enabled"), on("music_assistant_enabled"), host]))
EOF
)

# Puts $2 into file $1 when it differs; says whether it did.
replace() {
  if [ -f "$1" ] && cmp -s "$1" <(printf '%s' "$2"); then return 1; fi
  printf '%s' "$2" > "$1"
}

# A source counts once its service is there (a build installs the program before the unit).
installed() { systemctl cat "$1" >/dev/null 2>&1; }

# A source on or off. systemctl enable reloads all of systemd every time (1.3–1.5 s on a
# Pi 4, nothing else starts meanwhile), so only when the setting really changed; --no-block
# because squeezelite waits for the network, which held this script up at every boot.
want() {
  if [ "$2" = yes ]; then
    systemctl is-enabled --quiet "$1" || systemctl enable --quiet "$1"
    systemctl is-active --quiet "$1" || systemctl start --no-block "$1"
  else
    ! systemctl is-enabled --quiet "$1" || systemctl disable --quiet "$1"
    ! systemctl is-active --quiet "$1" || systemctl stop --no-block "$1"
  fi
}

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
  want shairport-sync.service "$AIRPLAY"
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
  want squeezelite.service "$MA"
fi

# The name as the box's address, only on a box made from the SD card image: a box set up
# by hand keeps the hostname its owner gave it.
if [ -e /etc/ownbox-image ] && [ "$HOST" != - ] && [ "$(hostname)" != "$HOST" ]; then
  hostnamectl set-hostname "$HOST"
  sed -i "s/^127\.0\.1\.1.*/127.0.1.1\t$HOST/" /etc/hosts
  systemctl try-restart avahi-daemon.service || true
fi

# Debug access (Settings → About the box, on the box's own screen): the user "debug" over
# SSH with the keys chosen there, and the journal kept on the card across restarts. "debug"
# reads every log and the state of the services but changes nothing; root stays with the
# USB console. ownbox (which a fault in the player could hand to someone) only keeps the
# choice, so it never leads to root. The keys are checked again here: a type and a key.
mapfile -t DEBUG < <(python3 - "$STATE" <<'EOF'
import json, re, sys
try:
    prefs = json.load(open(sys.argv[1]))
except Exception:
    prefs = {}
on = prefs.get("debug_access") is True
print("on" if on else "off")
if on:
    ok = re.compile(r"(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh\.com) [A-Za-z0-9+/=]{20,2000}")
    for key in (prefs.get("debug_keys") or [])[:20]:
        if isinstance(key, str) and ok.fullmatch(key):
            print(key)
EOF
)
# beta.13 put the keys in root's file; they leave it (keys added there by hand stay).
ROOT_AUTH=/root/.ssh/authorized_keys
if [ -f "$ROOT_AUTH" ] && grep -qx "# own.audio box debug access (Settings) begin" "$ROOT_AUTH"; then
  replace "$ROOT_AUTH" "$(awk '$0 == "# own.audio box debug access (Settings) begin" { skip = 1; next }
    $0 == "# own.audio box debug access (Settings) end" { skip = 0; next } !skip' "$ROOT_AUTH")
" || true
fi
DEBUG_HOME=/var/lib/ownbox-debug
DEBUG_AUTH=$DEBUG_HOME/.ssh/authorized_keys
if [ "${DEBUG[0]:-off}" = on ]; then
  if ! id debug >/dev/null 2>&1; then
    useradd --system --home-dir "$DEBUG_HOME" --no-create-home --shell /bin/bash \
      --groups systemd-journal,adm --comment "own.audio box debug access (read only)" debug
    # No password at all ("*"), not a locked one ("!"): sshd turns a locked account away
    # even with a key.
    usermod -p '*' debug
  fi
  # Root owns the folder and the keys: "debug" cannot add keys of its own.
  install -d -m 755 -o root -g root "$DEBUG_HOME" "$DEBUG_HOME/.ssh"
  replace "$DEBUG_AUTH" "$(printf '%s\n' "${DEBUG[@]:1}")
" && chmod 644 "$DEBUG_AUTH" || true
else
  if [ -f "$DEBUG_AUTH" ]; then
    rm -f "$DEBUG_AUTH"
    pkill -u debug 2>/dev/null || true
  fi
fi
JCONF=/etc/systemd/journald.conf.d/zz-ownbox-debug.conf
if [ "${DEBUG[0]:-off}" = on ]; then
  install -d /etc/systemd/journald.conf.d
  if replace "$JCONF" "[Journal]
Storage=persistent
SystemMaxUse=50M
"; then
    install -d /var/log/journal && systemd-tmpfiles --create --prefix /var/log/journal
    systemctl restart systemd-journald.service && journalctl --flush || true
  fi
  if installed ssh.service; then
    systemctl is-enabled --quiet ssh.service || systemctl enable --quiet ssh.service
    systemctl is-active --quiet ssh.service || systemctl start --no-block ssh.service
  fi
elif [ -f "$JCONF" ]; then
  rm -f "$JCONF"
  # A journal folder alone keeps the journal on the card (Storage=auto).
  rm -rf /var/log/journal
  systemctl restart systemd-journald.service || true
fi

# A sound card chosen on the box: its overlay into config.txt, which the firmware reads at
# boot only.
if [ -x /opt/ownbox/boot-config.sh ] && [ "$(/opt/ownbox/boot-config.sh)" = reboot ]; then
  systemctl reboot
fi
