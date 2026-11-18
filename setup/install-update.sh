#!/bin/bash
# Installs an ownbox update, or puts the previous one back (run as root by
# ownbox-update.service when ownbox leaves a request). ownbox runs as the user and has
# already checked the signature; this checks again on a root-owned copy, with the
# installed binary, so nothing the user can write reaches /opt unverified.
# Results go to /var/lib/ownbox, where ownbox can read but not write.
set -uo pipefail

BOX_USER=@USER@
DIR=/home/@USER@/.local/state/ownbox/update
OPT=/opt/ownbox
STATE=/var/lib/ownbox
install -d -m 755 "$STATE"
CURRENT_VERSION=$("$OPT/ownbox" --version 2>/dev/null | tr -cd 'A-Za-z0-9.+-' || true)
CURRENT_VERSION=${CURRENT_VERSION:-unknown}

result() { printf '%s %s\n' "$(date -Iseconds)" "$*" > "$STATE/update-result"; echo "$*"; }

# Up to a minute for the new ownbox to answer on its port.
answers() {
  for _ in $(seq 60); do
    curl -fsS -m 2 -o /dev/null http://127.0.0.1:8686/api/status && return 0
    sleep 1
  done
  return 1
}

swap_back() {
  install -m 755 "$OPT/ownbox.previous" "$OPT/ownbox.incoming"
  mv "$OPT/ownbox.incoming" "$OPT/ownbox"
  systemctl restart ownbox.service
}

# The box's system files from a release (ownbox-setup.tar.gz, signed like the program):
# checked with the installed ownbox, unpacked into a folder of root's, and only the files
# named here are put in place, at the places named here — nothing the archive says about
# paths counts. A file updates the one the box already has; the ones marked "new" are put
# in even if missing (they came with a later release). @USER@ becomes this box's user.
install_setup() {
  local bundle="$DIR/ownbox-setup.tar.gz" work src dest mode always
  [ -f "$bundle" ] && [ ! -L "$bundle" ] || return 0
  work=$(mktemp -d /run/ownbox-setup.XXXXXX)
  install -m 600 "$bundle" "$work/setup.tar.gz" && install -m 600 "$bundle.minisig" "$work/setup.tar.gz.minisig" \
    || { rm -rf "$work"; echo "no system files"; return 1; }
  rm -f "$bundle" "$bundle.minisig"
  if ! "$OPT/ownbox" verify "$work/setup.tar.gz" "$work/setup.tar.gz.minisig"; then
    rm -rf "$work"; echo "system files: the signature does not match"; return 1
  fi
  mkdir "$work/files"
  tar -xzf "$work/setup.tar.gz" -C "$work/files" --no-same-owner --no-same-permissions || { rm -rf "$work"; return 1; }
  while read -r src dest mode always; do
    [ -f "$work/files/$src" ] && [ ! -L "$work/files/$src" ] || continue
    [ -e "$dest" ] || [ "$always" = new ] || continue
    sed "s/@USER@/$BOX_USER/g" "$work/files/$src" > "$work/next"
    install -D -m "$mode" "$work/next" "$dest"
    echo "  $dest"
  done <<'FILES'
boot-config.sh /opt/ownbox/boot-config.sh 755 -
install-update.sh /opt/ownbox/install-update.sh 755 -
apply-sources.sh /opt/ownbox/apply-sources.sh 755 -
image/ownbox-boot.sh /opt/ownbox/ownbox-boot.sh 755 -
image/ownbox-boot.service /etc/systemd/system/ownbox-boot.service 644 -
ownbox.service /etc/systemd/system/ownbox.service 644 -
ownbox-kiosk.service /etc/systemd/system/ownbox-kiosk.service 644 -
ownbox-update.path /etc/systemd/system/ownbox-update.path 644 -
ownbox-update.service /etc/systemd/system/ownbox-update.service 644 -
ownbox-recover.service /etc/systemd/system/ownbox-recover.service 644 new
ownbox-sources.path /etc/systemd/system/ownbox-sources.path 644 -
ownbox-sources.service /etc/systemd/system/ownbox-sources.service 644 -
squeezelite.service /etc/systemd/system/squeezelite.service 644 -
50-ownbox-network.rules /etc/polkit-1/rules.d/50-ownbox-network.rules 644 -
51-ownbox-sources.rules /etc/polkit-1/rules.d/51-ownbox-sources.rules 644 -
ownbox-journald.conf /etc/systemd/journald.conf.d/ownbox.conf 644 -
ownbox-wifi-powersave.conf /etc/NetworkManager/conf.d/ownbox-wifi-powersave.conf 644 -
dnsmasq-ownbox-setup.conf /etc/NetworkManager/dnsmasq-shared.d/ownbox-setup.conf 644 -
ownbox-brcmfmac.conf /etc/modprobe.d/ownbox-brcmfmac.conf 644 new
FILES
  rm -rf "$work"
  systemctl daemon-reload
  # Units that start at boot since a later release (their [Install] says so).
  [ -e /etc/systemd/system/ownbox-sources.service ] && systemctl enable --quiet ownbox-sources.service
  return 0
}

# ownbox kept stopping (ownbox-recover.service, from ownbox.service's OnFailure). An update
# installed in the last day is the likely cause: back to the previous version, once per
# version, so two bad ones never swap back and forth. Then start it again in any case, after
# a pause, so a box whose sound card was briefly missing does not stay silent for good.
if [ "${1:-}" = recover ]; then
  sleep 30
  last=$(cat "$STATE/update-result" 2>/dev/null || true)
  installed_at=$(date -d "${last%% *}" +%s 2>/dev/null || echo 0)
  if [[ "$last" == *" installed "* ]] && [ $(( $(date +%s) - installed_at )) -lt 86400 ] \
     && [ -x "$OPT/ownbox.previous" ] && [ ! -e "$STATE/recovered-from-$CURRENT_VERSION" ]; then
    touch "$STATE/recovered-from-$CURRENT_VERSION"
    PREVIOUS=$(cat "$STATE/previous-version" 2>/dev/null || echo "?")
    cp -p "$OPT/ownbox" "$OPT/ownbox.swap"
    install -m 755 "$OPT/ownbox.previous" "$OPT/ownbox.incoming"
    mv "$OPT/ownbox.incoming" "$OPT/ownbox"
    mv "$OPT/ownbox.swap" "$OPT/ownbox.previous"
    echo "$CURRENT_VERSION" > "$STATE/previous-version"
    result "rolled back from $CURRENT_VERSION to $PREVIOUS: it kept stopping"
  fi
  systemctl reset-failed ownbox.service
  systemctl start ownbox.service
  exit 0
fi

# Only the real folder: a link planted in its place would point root elsewhere.
[ "$(realpath -e "$DIR" 2>/dev/null)" = "$DIR" ] || { rm -f "$DIR/request" 2>/dev/null; result "refused: $DIR is not a plain folder"; exit 1; }
if [ -L "$DIR/request" ]; then rm -f "$DIR/request"; exit 1; fi
[ -f "$DIR/request" ] || exit 0
read -r ACTION VERSION < <(head -c 200 "$DIR/request")
rm -f "$DIR/request"
CURRENT=$("$OPT/ownbox" --version 2>/dev/null || echo "?")

case "$ACTION" in
  install)
    # Copy first: the user's files could change between the check and the install.
    rm -f "$OPT/ownbox.incoming" "$OPT/ownbox.incoming.minisig"
    install -m 600 "$DIR/ownbox.new" "$OPT/ownbox.incoming" || { result "refused: no file"; exit 1; }
    install -m 600 "$DIR/ownbox.new.minisig" "$OPT/ownbox.incoming.minisig" || { result "refused: no signature"; exit 1; }
    rm -f "$DIR/ownbox.new" "$DIR/ownbox.new.minisig"
    if ! "$OPT/ownbox" verify "$OPT/ownbox.incoming" "$OPT/ownbox.incoming.minisig"; then
      rm -f "$OPT/ownbox.incoming" "$OPT/ownbox.incoming.minisig"
      result "refused ${VERSION:-?}: the signature does not match"
      exit 1
    fi
    rm -f "$OPT/ownbox.incoming.minisig"
    # The system files first, so the new program starts under its own units.
    echo "system files:"
    cp /etc/systemd/system/ownbox-kiosk.service "$STATE/kiosk-unit-before" 2>/dev/null || : > "$STATE/kiosk-unit-before"
    install_setup || echo "system files left as they were"
    chmod 755 "$OPT/ownbox.incoming"
    # A full card must not leave the box without either version.
    cp -p "$OPT/ownbox" "$OPT/ownbox.previous.new" && mv "$OPT/ownbox.previous.new" "$OPT/ownbox.previous" \
      || { rm -f "$OPT/ownbox.previous.new" "$OPT/ownbox.incoming"; result "refused ${VERSION:-?}: cannot keep the current version"; exit 1; }
    echo "$CURRENT" > "$STATE/previous-version"
    sync
    mv "$OPT/ownbox.incoming" "$OPT/ownbox" || { result "refused ${VERSION:-?}: cannot put it in place"; exit 1; }
    sync
    systemctl restart ownbox.service
    if answers; then
      result "installed $("$OPT/ownbox" --version 2>/dev/null || echo "${VERSION:-?}")"
      # What the new system files change outside ownbox: config.txt takes a restart of the
      # Pi, the kiosk's unit and zoom a restart of the kiosk.
      if [ "$("$OPT/boot-config.sh" 2>/dev/null)" = reboot ]; then
        echo "config.txt changed; restarting the Pi"
        sync
        systemctl reboot
      elif ! cmp -s "$STATE/kiosk-unit-before" /etc/systemd/system/ownbox-kiosk.service \
           && systemctl is-active --quiet ownbox-kiosk.service; then
        systemctl restart ownbox-kiosk.service
      fi
    else
      swap_back
      answers
      result "rolled back from ${VERSION:-?} to $CURRENT: the new version did not start"
    fi
    ;;
  rollback)
    [ -x "$OPT/ownbox.previous" ] || { result "nothing to go back to"; exit 1; }
    PREVIOUS=$(cat "$STATE/previous-version" 2>/dev/null || echo "?")
    # The two swap places, so going back can be undone the same way.
    cp -p "$OPT/ownbox" "$OPT/ownbox.swap"
    swap_back
    mv "$OPT/ownbox.swap" "$OPT/ownbox.previous"
    echo "$CURRENT" > "$STATE/previous-version"
    answers
    result "went back from $CURRENT to $PREVIOUS"
    ;;
  *) result "unknown request: $ACTION" ;;
esac
