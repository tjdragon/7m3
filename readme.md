# 7ME3: Cloud-Agnostic 12-Factor Microservices Platform in Rust

A reference architecture and production-grade implementation demonstrating **The 12-Factor App** principles across **AWS (EC2 / Nitro Enclaves)**, **Google Cloud (GCE / Confidential VMs)**, **Kubernetes**, and **Bare Metal**.

See [ARCHITECTURE.md](file:///home/tj/Code/7Me3/ARCHITECTURE.md) for the full architectural specification, 12-factor mapping table, and confidential computing design.

---

## The Three Core Abstractions

1. **Database Access ([crates/platform/src/database.rs](file:///home/tj/Code/7Me3/crates/platform/src/database.rs))**:
   - Backed by PostgreSQL (`sqlx::PgPool`).
   - Attached resource with auto-reconnect, connection pooling, and health probing (`SELECT 1`).
   - Dynamic secret URI interpolation (e.g. `postgres://user:secret://DB_PASSWORD@host:5432/db`).
   - Automated idempotent database migrations runner.

2. **Secrets Management ([crates/platform/src/secrets/](file:///home/tj/Code/7Me3/crates/platform/src/secrets))**:
   - `SecretProvider` trait with dynamic dispatch via `SECRETS_PROVIDER`.
   - **Environment Variables**: [EnvSecretProvider](file:///home/tj/Code/7Me3/crates/platform/src/secrets/env.rs) (local dev, bare metal).
   - **Mounted Volumes**: [FileSecretProvider](file:///home/tj/Code/7Me3/crates/platform/src/secrets/file.rs) (Kubernetes projected secrets, confidential VM `tmpfs` ramdisks).
   - **AWS Secrets Manager**: [AwsSecretsManagerProvider](file:///home/tj/Code/7Me3/crates/platform/src/secrets/aws.rs) (EC2 IAM roles, Nitro Enclaves).
   - **GCP Secret Manager**: [GcpSecretManagerProvider](file:///home/tj/Code/7Me3/crates/platform/src/secrets/gcp.rs) (GCE Service Account metadata tokens, GKE Workload Identity, AMD SEV-SNP VMs).

3. **Inter-Service Messaging ([crates/platform/src/messaging/](file:///home/tj/Code/7Me3/crates/platform/src/messaging))**:
   - `EventPublisher` and `EventSubscriber` traits.
   - **In-Memory Bus**: [InMemoryEventBus](file:///home/tj/Code/7Me3/crates/platform/src/messaging/in_memory.rs) (zero-infrastructure local dev and blazing-fast integration testing).
   - **Apache Kafka / Redpanda / AWS MSK / GCP Managed Kafka**: [KafkaBus](file:///home/tj/Code/7Me3/crates/platform/src/messaging/kafka.rs).

---

## Services

- **[api-service](file:///home/tj/Code/7Me3/services/api-service)**: Web microservice built on `axum`. Exposes `/api/orders`, `/health/live`, `/health/ready`, and `/api/system/secrets-audit`. Inserts orders into Postgres and emits `orders.created` events.
- **[worker-service](file:///home/tj/Code/7Me3/services/worker-service)**: Downstream asynchronous processing service. Subscribes to `orders.created` events, processes business logic, and updates orders to `COMPLETED` in Postgres.

---

## Quickstart

### 1. Start Backing Services
```bash
docker compose up -d postgres redpanda
```

### 2. Run API Service
```bash
cargo run -p api-service
```

### 3. Run Worker Service
```bash
cargo run -p worker-service
```

### 4. Test Endpoints
```bash
# Check health
curl -s http://localhost:8080/health/ready | jq .

# Verify secret abstraction
curl -s http://localhost:8080/api/system/secrets-audit | jq .

# Create order
curl -s -X POST http://localhost:8080/api/orders \
  -H "Content-Type: application/json" \
  -d '{"customer_name": "Acme Corp", "amount_cents": 19900, "currency": "USD"}' | jq .

# List orders
curl -s http://localhost:8080/api/orders | jq .
```

---

## Deployments

- **Kubernetes**: [deploy/k8s/](file:///home/tj/Code/7Me3/deploy/k8s/)
- **AWS EC2 & Nitro Enclaves**: [deploy/aws/user-data.sh](file:///home/tj/Code/7Me3/deploy/aws/user-data.sh)
- **GCP Compute Engine & Confidential VMs**: [deploy/gcp/startup-script.sh](file:///home/tj/Code/7Me3/deploy/gcp/startup-script.sh)
- **Bare Metal Hardening**: [deploy/baremetal/7me3-api.service](file:///home/tj/Code/7Me3/deploy/baremetal/7me3-api.service)