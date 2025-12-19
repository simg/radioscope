#!/usr/bin/env bash
set -euo pipefail

# Configure Raspberry Pi for:
# - Built-in Wi-Fi (wlan0) as an access point (SSID/passphrase below)
# - External USB adapter (wlan1) put into monitor mode (wlan1mon)
#
# Usage:
#   PI_HOST=192.168.50.10 ./scripts/configure_pi_wifi.sh
# Optional env vars:
#   PI_USER (default: pi)
#   COUNTRY_CODE (default: GB)
#   SSID (default: radioscope)
#   PRESHARED_KEY (required)
#   AP_SUBNET (default: 192.168.50.1/24)
#   AP_GATEWAY_IP (default: derived from AP_SUBNET)
#   SSH_OPTS (pass extra flags to ssh)
#
# Reboot the Pi after running.

PI_HOST="${PI_HOST:-}"
PI_USER="${PI_USER:-pi}"
COUNTRY_CODE="${COUNTRY_CODE:-GB}"
SSID="${SSID:-radioscope}"
PRESHARED_KEY="${PRESHARED_KEY:-}"
AP_SUBNET="${AP_SUBNET:-192.168.50.1/24}"
AP_GATEWAY_IP="${AP_GATEWAY_IP:-${AP_SUBNET%/*}}"
MONITOR_IFACE="${MONITOR_IFACE:-wlan1}"
MONITOR_PHY="${MONITOR_PHY:-}"
SSH_OPTS="${SSH_OPTS:-}"

if [[ -z "${PI_HOST}" ]]; then
  echo "Set PI_HOST to the Pi's IP/hostname. Example: PI_HOST=192.168.50.10 $0" >&2
  exit 1
fi

if [[ -z "${PRESHARED_KEY}" ]]; then
  echo "Set PRESHARED_KEY to the Wi-Fi passphrase for the access point." >&2
  exit 1
fi

echo "[local] configuring ${PI_USER}@${PI_HOST} for AP + monitor mode..."

ssh -T ${SSH_OPTS} "${PI_USER}@${PI_HOST}" "AP_SUBNET='${AP_SUBNET}' AP_GATEWAY_IP='${AP_GATEWAY_IP}' SSID='${SSID}' PRESHARED_KEY='${PRESHARED_KEY}' COUNTRY_CODE='${COUNTRY_CODE}' MONITOR_IFACE='${MONITOR_IFACE}' MONITOR_PHY='${MONITOR_PHY}' bash -s" <<'EOSH'
set -euo pipefail

AP_SUBNET="${AP_SUBNET:-192.168.50.1/24}"
AP_GATEWAY_IP="${AP_GATEWAY_IP:-192.168.50.1}"
SSID="${SSID:-radioscope}"
PRESHARED_KEY="${PRESHARED_KEY:?PRESHARED_KEY required}"
COUNTRY_CODE="${COUNTRY_CODE:-GB}"
MONITOR_IFACE="${MONITOR_IFACE:-wlan1}"
MONITOR_PHY="${MONITOR_PHY:-}"
IW_BIN="/usr/sbin/iw"
IP_BIN="/sbin/ip"

echo "[pi] starting configuration..."
echo "[pi] installing packages..."
sudo apt-get update
sudo apt-get install -y hostapd dnsmasq iw rfkill wireless-tools dhcpcd5
sudo systemctl unmask hostapd.service || true

echo "[pi] unblocking wifi..."
sudo rfkill unblock all

echo "[pi] configuring dhcpcd for wlan0 (static ${AP_SUBNET})..."
sudo mkdir -p /etc/dhcpcd.conf.d
sudo tee /etc/dhcpcd.conf.d/radioscope.conf >/dev/null <<EODHCPCD
interface wlan0
static ip_address=${AP_SUBNET}
nohook wpa_supplicant
EODHCPCD
# ensure dhcpcd loads the radioscope override
sudo sed -i '/radioscope\.conf/d' /etc/dhcpcd.conf
echo "source /etc/dhcpcd.conf.d/radioscope.conf" | sudo tee -a /etc/dhcpcd.conf >/dev/null

echo "[pi] configuring dnsmasq for wlan0..."
sudo mkdir -p /etc/dnsmasq.d
sudo tee /etc/dnsmasq.d/radioscope.conf >/dev/null <<EODNS
interface=wlan0
bind-interfaces
dhcp-range=192.168.50.10,192.168.50.100,255.255.255.0,24h
dhcp-option=3,${AP_GATEWAY_IP}
dhcp-option=6,8.8.8.8,1.1.1.1
EODNS
# Ensure dnsmasq brings wlan0 up before binding.
sudo mkdir -p /etc/systemd/system/dnsmasq.service.d
sudo tee /etc/systemd/system/dnsmasq.service.d/radioscope.conf >/dev/null <<EODROP
[Unit]
After=dhcpcd.service hostapd.service network-online.target
Wants=hostapd.service

[Service]
ExecStartPre=/sbin/ip link set wlan0 up
ExecStartPre=/bin/sleep 2
ExecStartPre=/sbin/ip addr flush dev wlan0
ExecStartPre=/sbin/ip addr add ${AP_SUBNET} dev wlan0
EODROP

echo "[pi] configuring hostapd..."
sudo tee /etc/hostapd/hostapd.conf >/dev/null <<EOHAP
country_code=${COUNTRY_CODE}
interface=wlan0
driver=nl80211
ssid=${SSID}
hw_mode=g
channel=6
wmm_enabled=1
auth_algs=1
wpa=2
wpa_passphrase=${PRESHARED_KEY}
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
EOHAP
sudo sed -i 's|^#*DAEMON_CONF=.*$|DAEMON_CONF="/etc/hostapd/hostapd.conf"|' /etc/default/hostapd

echo "[pi] disabling wpa_supplicant on wlan0 to avoid conflicts..."
sudo systemctl disable --now wpa_supplicant@wlan0.service || true
sudo systemctl mask wpa_supplicant@wlan0.service || true
sudo systemctl enable dhcpcd.service || true

echo "[pi] setting wlan0 address immediately..."
sudo /sbin/ip addr flush dev wlan0 || true
sudo /sbin/ip addr add ${AP_SUBNET} dev wlan0 || true
sudo /sbin/ip link set wlan0 up || true

echo "[pi] creating wlan1 monitor-mode service..."
sudo tee /etc/systemd/system/wlan1mon.service >/dev/null <<'EOSVC'
[Unit]
Description=Set wlan1 to monitor mode as wlan1mon
After=network.target systemd-udev-settle.service

[Service]
Type=oneshot
Environment=MONITOR_IFACE=${MONITOR_IFACE}
Environment=MONITOR_PHY=${MONITOR_PHY}
ExecStartPre=/bin/sleep 2
ExecStart=/bin/sh -c '\
  set -e; \
  IFACE="${MONITOR_IFACE:-}"; \
  PHY="${MONITOR_PHY:-}"; \
  if [ -z "$IFACE" ] || ! /sbin/ip link show "$IFACE" >/dev/null 2>&1; then \
    IFACE=$(/usr/sbin/iw dev | awk '"'"'/Interface/ {if ($2!="wlan0") {print $2; exit}}'"'"'); \
  fi; \
  if [ -z "$IFACE" ] && /sbin/ip link show wlan1mon >/dev/null 2>&1; then IFACE="wlan1mon"; fi; \
  if [ -z "$PHY" ] && [ -n "$IFACE" ]; then \
    PHY=$(/usr/sbin/iw dev "$IFACE" info 2>/dev/null | awk '"'"'/wiphy/ {print "phy"$2}'"'"'); \
  fi; \
  if [ -z "$PHY" ]; then \
    PHY=$(/usr/sbin/iw list | awk '"'"'/Wiphy/ {p=$2} /Supported interface modes/ {flag=1} /monitor/ {if(flag){print p; exit}} /Supported commands/ {flag=0}'"'"'); \
  fi; \
  PHY=$(echo "$PHY" | sed -e '"'"'s/^phy#/phy/'"'"'); \
  if [ -z "$PHY" ]; then \
    echo "No PHY with monitor support found; skipping" >&2; \
    exit 0; \
  fi; \
  /sbin/ip link set "$IFACE" down || true; \
  if /usr/sbin/iw dev "$IFACE" set type monitor 2>/dev/null; then \
    /sbin/ip link set "$IFACE" name wlan1mon || true; \
  else \
    /usr/sbin/iw dev wlan1mon del 2>/dev/null || true; \
    /usr/sbin/iw phy "$PHY" interface add wlan1mon type monitor || true; \
  fi; \
  # Fallback if driver renames but stays managed
  STATE=$(/usr/sbin/iw dev wlan1mon info 2>/dev/null | awk '"'"'/type/ {print $2}'"'"'); \
  if [ "$STATE" != "monitor" ]; then \
    /usr/sbin/iw dev wlan1mon del 2>/dev/null || true; \
    /usr/sbin/iw phy "$PHY" interface add wlan1mon type monitor || true; \
  fi; \
  /sbin/ip link set wlan1mon up 2>/dev/null || true; \
  echo "wlan1mon set to monitor (source iface: $IFACE, phy: $PHY)" \
'
RemainAfterExit=yes
Restart=no

[Install]
WantedBy=multi-user.target
EOSVC

echo "[pi] enabling services..."
sudo systemctl daemon-reload
sudo systemctl enable dnsmasq hostapd wlan1mon.service
sudo systemctl restart dhcpcd || true
sudo ip link set wlan0 up || true
sudo systemctl restart dnsmasq hostapd wlan1mon.service || true
sudo ip addr show wlan0

echo "[pi] done. Reboot the Pi to fully apply network changes."
EOSH
echo "[local] configuration script completed. Reboot the Pi to apply."
