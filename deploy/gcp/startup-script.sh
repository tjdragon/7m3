#!/usr/bin/env bash
# ==============================================================================
# 7ME3 - Google Compute Engine (GCE) / Confidential VM Startup Script
# ==============================================================================
set -euo pipefail

echo "==> Configuring 7ME3 on Google Compute Engine / Confidential VM..."

apt-get update -y
apt-get install -y ca-certificates curl jq

useradd -r -s /bin/false appuser || true

# On GCP, we can query GCP Secret Manager via IAM Service Account attached to the VM / Confidential VM
GCP_PROJECT_ID=$(curl -s -H "Metadata-Flavor: Google" http://metadata.google.internal/computeMetadata/v1/project/project-id)

cat << EOF > /etc/default/7me3-api
APP_ENV=production
PORT=80
LOG_FORMAT=json
SECRETS_PROVIDER=gcp
GCP_PROJECT_ID=${GCP_PROJECT_ID}
DATABASE_URL=postgres://orders_user:secret://cloudsql-db-password@10.0.0.5:5432/orders_db
AUTO_MIGRATE=true
MESSAGE_BUS_BACKEND=kafka
KAFKA_BROKERS=bootstrap.orders-kafka.us-central1.managedkafka.gcp.internal:9092
AUDIT_SECRET_KEY=app-production-encryption-key
EOF

cat << 'EOF' > /etc/systemd/system/7me3-api.service
[Unit]
Description=7ME3 API Service (12-Factor Web Service on GCP)
After=network.target

[Service]
Type=simple
User=appuser
Group=appuser
EnvironmentFile=/etc/default/7me3-api
ExecStart=/usr/local/bin/api-service
Restart=always
RestartSec=5s
LimitNOFILE=65536

# Factor IX: Disposability (Graceful Shutdown)
KillSignal=SIGTERM
TimeoutStopSec=30s

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now 7me3-api.service

echo "==> 7ME3 API Service running on GCE / Confidential VM!"
