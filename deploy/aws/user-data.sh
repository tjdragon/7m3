#!/usr/bin/env bash
# ==============================================================================
# 7ME3 - AWS EC2 / Nitro Enclave Deployment & Startup Script
# ==============================================================================
set -euo pipefail

echo "==> Configuring 7ME3 12-Factor Microservices on AWS..."

# 1. Install prerequisites
apt-get update -y
apt-get install -y ca-certificates curl jq

# 2. Setup application user
useradd -r -s /bin/false appuser || true

# 3. Create systemd environment file
# On AWS, secrets can be fetched directly from AWS Secrets Manager using IAM role
cat << 'EOF' > /etc/default/7me3-api
APP_ENV=production
PORT=80
LOG_FORMAT=json
SECRETS_PROVIDER=aws
DATABASE_URL=postgres://orders_user:secret://rds-aurora-db-password@aurora-cluster.us-east-1.rds.amazonaws.com:5432/orders_db
AUTO_MIGRATE=true
MESSAGE_BUS_BACKEND=kafka
KAFKA_BROKERS=b-1.orders-msk.us-east-1.kafka.amazonaws.com:9092,b-2.orders-msk.us-east-1.kafka.amazonaws.com:9092
AUDIT_SECRET_KEY=app-production-encryption-key
EOF

# 4. Systemd service unit for API Service
cat << 'EOF' > /etc/systemd/system/7me3-api.service
[Unit]
Description=7ME3 API Service (12-Factor Web Service)
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

# 5. Reload and start
systemctl daemon-reload
systemctl enable --now 7me3-api.service

echo "==> 7ME3 API Service deployed successfully on AWS EC2!"
