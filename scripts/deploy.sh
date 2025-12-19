#!/usr/bin/env bash
set -euo pipefail

# Cross-compile and deploy the radioscope binary + systemd unit to a Pi.
# Usage: PI_HOST=192.168.1.50 ./scripts/deploy.sh

TARGET="aarch64-unknown-linux-gnu"
PI_HOST="${1:-${PI_HOST:-}}"
PI_USER="${PI_USER:-pi}"
REMOTE_DIR="${REMOTE_DIR:-/home/${PI_USER}/radioscope}"
SSH_OPTS="${SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=10}"

if [[ -z "${PI_HOST}" ]]; then
  echo "Usage: PI_HOST=<pi-ip> ./scripts/deploy.sh"
  exit 1
fi

# echo "[deploy] building release binary for ${TARGET}..."
# cargo build --release --target "${TARGET}"

echo "[deploy] ensuring remote directory ${REMOTE_DIR}..."
ssh ${SSH_OPTS} "${PI_USER}@${PI_HOST}" "mkdir -p ${REMOTE_DIR}"

echo "[deploy] copying artifacts..."
scp ${SSH_OPTS} "target/${TARGET}/release/radioscope" "${PI_USER}@${PI_HOST}:${REMOTE_DIR}/"
scp ${SSH_OPTS} "deploy/radioscope.service" "${PI_USER}@${PI_HOST}:${REMOTE_DIR}/"

echo "[deploy] installing service..."
ssh ${SSH_OPTS} "${PI_USER}@${PI_HOST}" <<EOF
set -e
sudo mv ${REMOTE_DIR}/radioscope /usr/local/bin/radioscope
sudo mv ${REMOTE_DIR}/radioscope.service /etc/systemd/system/radioscope.service
sudo systemctl daemon-reload
sudo systemctl enable radioscope.service
sudo systemctl restart radioscope.service
EOF

echo "[deploy] done."
