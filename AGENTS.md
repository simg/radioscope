# Agents Guide

This repository is configured to run on a Raspberry Pi 4 with an external Wi‑Fi dongle in monitor mode and a built-in Wi‑Fi AP. Keep these points in mind when making changes or debugging:

## Cross-compilation
- Target: `aarch64-unknown-linux-gnu`.
- Sysroot: `/opt/pi-sysroot` with Pi libraries/headers (`alsa.pc`, `pcap.pc` under `usr/lib/aarch64-linux-gnu/pkgconfig`).
- Env vars for cross builds (export before `cargo build --target aarch64-unknown-linux-gnu`):
  ```
  PKG_CONFIG=/usr/bin/pkg-config
  PKG_CONFIG_ALLOW_CROSS=1
  PKG_CONFIG_SYSROOT_DIR=/opt/pi-sysroot
  PKG_CONFIG_LIBDIR=/opt/pi-sysroot/usr/lib/aarch64-linux-gnu/pkgconfig
  PKG_CONFIG_PATH=/opt/pi-sysroot/usr/lib/aarch64-linux-gnu/pkgconfig
  ```
- Unset these for native host builds to avoid pointing at the Pi sysroot.

## Services on the Pi
- `radioscope.service`: runs the app as root; HTTP binds to `0.0.0.0:8080` by default; monitor interface `wlan1mon`.
- `wlan1mon.service`: sets the USB dongle into monitor mode and names it `wlan1mon`. Uses `MONITOR_IFACE`/`MONITOR_PHY` envs or auto-detects a non-`wlan0` interface/monitor-capable phy. It does not delete the dongle iface; it converts or creates `wlan1mon`.
- `hostapd`, `dnsmasq`, `dhcpcd`: built-in Wi‑Fi runs an AP on `wlan0`, static IP `192.168.50.1/24`, DHCP range `192.168.50.10-100`.

## Networking notes
- AP clients should reach the UI at `http://192.168.50.1:8080`. If they can’t:
  - Verify `wlan0` has `192.168.50.1/24` and dhcpcd is running.
  - Check `dnsmasq` is active and bound to `wlan0`.
  - Ensure `wlan1mon` exists and is type `monitor` (`iw dev`).
- The sniffer opens `wlan1mon` without forcing rfmon; the interface must already be in monitor mode via `wlan1mon.service`.

## Deploy
- `scripts/deploy.sh`: copies the built binary and systemd unit to the Pi (`PI_HOST`, `PI_USER`, `SSH_OPTS` configurable), installs to `/usr/local/bin` and `/etc/systemd/system`, reloads/starts `radioscope.service`.

## SSR/UI
- Dioxus SSR builds the VDOM before rendering (`NoOpMutations` rebuild) to avoid SSR panics.

## Troubleshooting commands (on Pi)
- Service logs: `sudo journalctl -u radioscope.service -b -n 200`.
- Interface state: `iw dev`, `ip addr show wlan0`, `ip addr show wlan1mon`.
- DHCP leases: `cat /var/lib/misc/dnsmasq.leases`.
- Monitor check: `sudo tcpdump -i wlan1mon -I -c 5` (should work when `wlan1mon` is monitor).
