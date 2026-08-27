#!/bin/sh
# pam_pwdfile file (0640 root:$USER); unset enables tap-to-unlock.
# Usage: sudo ./make-pin.sh <4-digit-PIN> [username] | --unset [username]
set -eu

PIN_FILE="${MECHANIX_PIN_FILE:-/etc/mechanix/pin.passwd}"
USER="${2:-mecha}"

if [ "${1:-}" = "--unset" ]; then
    sed -i "/^${USER}:/d" "$PIN_FILE" 2>/dev/null || true
    if [ -f "$PIN_FILE" ]; then
        chown root:"$USER" "$PIN_FILE"
        chmod 0640 "$PIN_FILE"
    fi
    echo "PIN removed for ${USER}"
    exit 0
fi

PIN="${1:?usage: make-pin.sh <4-digit-PIN> [username] | --unset [username]}"
case "$PIN" in
    ''|*[!0-9]*) echo "error: PIN must be digits only" >&2; exit 1 ;;
esac
[ "${#PIN}" -eq 4 ] || { echo "error: PIN must be 4 digits" >&2; exit 1; }

HASH="$(openssl passwd -6 "$PIN")"
install -d -m 0750 -o root -g "$USER" "$(dirname "$PIN_FILE")"
sed -i "/^${USER}:/d" "$PIN_FILE" 2>/dev/null || true
printf '%s:%s\n' "$USER" "$HASH" >> "$PIN_FILE"
chown root:"$USER" "$PIN_FILE"
chmod 0640 "$PIN_FILE"
echo "PIN file updated: $PIN_FILE (user: $USER)"
