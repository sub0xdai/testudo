# Specification: Infrastructure Production Readiness

**Spec ID:** AUD-06-infrastructure-readiness
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 2 (Reliability)
**Audit Refs:** K8s probes, resource limits, PDBs, DB backups, CI/CD, sealed secrets

---

## Overview

Harden the Kubernetes infrastructure and add CI/CD automation. The current K8s manifests lack health probes, resource limits, pod disruption budgets, and backup automation. There is no CI/CD pipeline — all testing and deployment is manual.

**Current state:**
- No liveness/readiness probes — unhealthy pods serve traffic, unresponsive containers never restart
- No resource requests/limits — uncontrolled memory growth, noisy neighbor risk
- No pod disruption budgets — cascading failures during rolling updates
- No database backup automation — PostgreSQL failure = total data loss
- No CI/CD pipeline — tests run locally, deployment is manual
- Sealed secrets referenced but not wired into deployments

**Target state:**
- All deployments have liveness/readiness probes, resource limits, and PDBs
- PostgreSQL backups run daily with point-in-time recovery
- GitHub Actions CI runs tests on every PR, builds images on merge
- Secrets managed via K8s sealed-secrets, not env vars in manifests

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add liveness probe (`/api/v1/health`) and readiness probe (`/api/v1/health/ready`) to backend deployment | Critical | Ops / K8s |
| FR-2 | Add health endpoint that checks DB pool and sidecar connectivity | Critical | Router |
| FR-3 | Add resource requests and limits to all deployments (backend, sidecar, websocket, db-processor) | High | Ops / K8s |
| FR-4 | Add PodDisruptionBudget for backend (minAvailable: 1) | High | Ops / K8s |
| FR-5 | Configure PostgreSQL CronJob for daily pg_dump to persistent volume or cloud storage | High | Ops / K8s |
| FR-6 | Create GitHub Actions workflow: `ci.yml` — run `cargo clippy && cargo test` and `bun run build` on PRs | Critical | CI/CD |
| FR-7 | Create GitHub Actions workflow: `deploy.yml` — build Docker images and push to registry on merge to master | High | CI/CD |
| FR-8 | Wire sealed-secrets into deployment manifests for ENCRYPTION_KEY, JWT_SECRET, DB credentials | High | Ops / K8s |
| FR-9 | Add liveness probe to CCXT sidecar deployment using existing `/health` endpoint | Medium | Ops / K8s |
| FR-10 | Document backup restoration procedure and test it | Medium | Ops / Docs |

---

## Technical Implementation

### 1) Health Endpoints (FR-2)

```rust
// GET /api/v1/health — liveness (am I running?)
async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

// GET /api/v1/health/ready — readiness (can I serve traffic?)
async fn health_ready(pool: web::Data<PgPool>, state: web::Data<AppState>) -> HttpResponse {
    // Check DB
    let db_ok = sqlx::query("SELECT 1").execute(pool.as_ref()).await.is_ok();
    // Check sidecar
    let sidecar_ok = state.sidecar_health.read().await.is_healthy();

    if db_ok && sidecar_ok {
        HttpResponse::Ok().json(serde_json::json!({"status": "ready", "db": true, "sidecar": true}))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready", "db": db_ok, "sidecar": sidecar_ok
        }))
    }
}
```

### 2) K8s Deployment Probes (FR-1)

```yaml
# backend/deployment.yml
containers:
  - name: testudo-backend
    resources:
      requests:
        cpu: "250m"
        memory: "512Mi"
      limits:
        cpu: "1000m"
        memory: "2Gi"
    livenessProbe:
      httpGet:
        path: /api/v1/health
        port: 8080
      initialDelaySeconds: 15
      periodSeconds: 10
      failureThreshold: 3
    readinessProbe:
      httpGet:
        path: /api/v1/health/ready
        port: 8080
      initialDelaySeconds: 10
      periodSeconds: 5
      failureThreshold: 3
```

### 3) PodDisruptionBudget (FR-4)

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: testudo-backend-pdb
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app: testudo-backend
```

### 4) Database Backup CronJob (FR-5)

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: postgres-backup
spec:
  schedule: "0 2 * * *"  # 2 AM daily
  jobTemplate:
    spec:
      template:
        spec:
          containers:
            - name: pg-backup
              image: postgres:16
              command:
                - /bin/sh
                - -c
                - pg_dump -h $PGHOST -U $PGUSER -d $PGDATABASE | gzip > /backups/testudo-$(date +%Y%m%d).sql.gz
              volumeMounts:
                - name: backup-volume
                  mountPath: /backups
          restartPolicy: OnFailure
          volumes:
            - name: backup-volume
              persistentVolumeClaim:
                claimName: postgres-backup-pvc
```

### 5) CI/CD — GitHub Actions (FR-6, FR-7)

```yaml
# .github/workflows/ci.yml
name: CI
on: [pull_request]
jobs:
  backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { submodules: true }
      - uses: dtolnay/rust-toolchain@stable
      - run: cd testudo-exchange && cargo clippy --all-targets -- -D warnings
      - run: cd testudo-exchange && cargo test
  extension:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
      - run: cd testudo-extension && bun install && bun run build
  sidecar:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
      - run: cd testudo-ccxt && bun install && bun test
```

---

## Verification

- [ ] `/api/v1/health` returns 200 when server is running
- [ ] `/api/v1/health/ready` returns 503 when DB is down
- [ ] K8s restarts pod after 3 failed liveness probes
- [ ] Resource limits prevent OOM kills under normal load
- [ ] PDB prevents all replicas from terminating during rolling update
- [ ] Daily backup CronJob produces valid .sql.gz file
- [ ] Backup can be restored to empty database successfully
- [ ] CI pipeline runs and passes on PR
- [ ] CI pipeline fails on clippy warnings or test failures
