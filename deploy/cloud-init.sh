#!/bin/bash
# Cloud-init script for DOOM demo chain — single validator (--dev mode)
# Provision: Ubuntu 22.04 on DigitalOcean

set -euo pipefail

BINARY_URL="https://github.com/matt-nowakowski/chainreactor-node/releases/download/v1.19.10-doom/cr-node-doom-linux-amd64"
BINARY_PATH="/usr/local/bin/cr-node-doom"
DATA_DIR="/var/lib/cr-node-doom"
SERVICE_NAME="cr-node-doom"

# --- Install binary ---
echo "Downloading DOOM binary..."
curl -L -o "$BINARY_PATH" "$BINARY_URL"
chmod +x "$BINARY_PATH"

# --- Create data directory ---
mkdir -p "$DATA_DIR"

# --- Create systemd service ---
cat > /etc/systemd/system/${SERVICE_NAME}.service <<EOF
[Unit]
Description=Chainreactor DOOM Demo Node
After=network.target

[Service]
Type=simple
ExecStart=${BINARY_PATH} \\
  --dev \\
  --base-path ${DATA_DIR} \\
  --rpc-external \\
  --rpc-cors all \\
  --rpc-methods unsafe \\
  --name "doom-demo-validator" \\
  --ethereum-node-url wss://ethereum-rpc.publicnode.com \\
  --tnf-port 2020
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# --- Start the node ---
systemctl daemon-reload
systemctl enable ${SERVICE_NAME}
systemctl start ${SERVICE_NAME}

echo "DOOM demo node started. RPC available on port 9944."
