#!/usr/bin/env bash
set -euo pipefail

# Gather Wi-Fi/AP/monitor-mode diagnostics on the Pi.
# Usage: PI_HOST=192.168.10.125 ./scripts/troubleshoot_wifi.sh
# Optional: PI_USER (default: pi), SSH_OPTS

PI_HOST="${PI_HOST:-}"
PI_USER="${PI_USER:-pi}"
SSH_OPTS="${SSH_OPTS:-}"

if [[ -z "$PI_HOST" ]]; then
  echo "Set PI_HOST to the Pi's IP/hostname. Example: PI_HOST=192.168.10.125 $0" >&2
  exit 1
fi

ssh -T ${SSH_OPTS} "${PI_USER}@${PI_HOST}" 'bash -s' <<'EOSH'
set -euo pipefail
echo "=== hostname ==="
hostname

echo "=== systemctl statuses ==="
sudo systemctl status hostapd dnsmasq wlan1mon.service 2>&1 | sed "s/^/    /"

echo "=== journal (hostapd + wlan1mon) ==="
sudo journalctl -u hostapd -u wlan1mon.service -b | tail -n 120 | sed "s/^/    /"

echo "=== config files ==="
for f in /etc/hostapd/hostapd.conf /etc/dhcpcd.conf.d/radioscope.conf /etc/dnsmasq.d/radioscope.conf /etc/systemd/system/wlan1mon.service; do
  echo "--- $f ---"
  if sudo test -f "$f"; then
    sudo cat "$f"
  else
    echo "    (missing)"
  fi
done

echo "=== iw list (supported modes) ==="
/usr/sbin/iw list | awk "/Supported interface modes/{flag=1;print;next}/Supported commands/{flag=0}flag" | sed "s/^/    /"

echo "=== interfaces ==="
ip addr show wlan0 | sed "s/^/wlan0: /"
ip addr show wlan1 2>/dev/null | sed "s/^/wlan1: /"
ip addr show wlan1mon 2>/dev/null | sed "s/^/wlan1mon: /"

echo "=== manual monitor attempt (non-fatal) ==="
sudo ip link set wlan1 down 2>/dev/null || echo "    wlan1 down failed"
sudo iw dev wlan1mon del 2>/dev/null || true
sudo iw dev wlan1 set type monitor 2>/dev/null && echo "    set type monitor ok" || echo "    set type monitor failed"
sudo ip link set wlan1 name wlan1mon 2>/dev/null || echo "    rename to wlan1mon failed"
sudo ip link set wlan1mon up 2>/dev/null && echo "    wlan1mon up ok" || echo "    wlan1mon up failed"
ip addr show wlan1mon 2>/dev/null | sed "s/^/wlan1mon: /"

echo "=== dnsmasq leases ==="
if sudo test -f /var/lib/misc/dnsmasq.leases; then
  sudo cat /var/lib/misc/dnsmasq.leases
else
  echo "    (no leases file)"
fi

echo "=== done ==="
EOSH
