#!/bin/bash
# Installs an ownbox update, or puts the previous one back (run as root by
# ownbox-update.service when ownbox leaves a request). ownbox runs as the user and has
# already checked the signature; this checks again on a root-owned copy, with the
# installed binary, so nothing the user can write reaches /opt unverified.
# Results go to /var/lib/ownbox, where ownbox can read but not write.
set -uo pipefail

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
