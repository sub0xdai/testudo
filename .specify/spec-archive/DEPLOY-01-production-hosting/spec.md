# Specification: Production Hosting

**Spec ID:** DEPLOY-01-production-hosting
**Date:** 2026-03-27
**Status:** Draft
**Class:** Infrastructure
**Priority:** P0 — Product cannot be used without hosting.

---

## Infrastructure

| Resource | Details |
|---|---|
| **Domain** | `testudo.vip` (Cloudflare DNS) |
| **Static hosting** | Cloudflare Pages (free tier) |
| **Compute** | DigitalOcean droplet — 2 vCPU, 4GB RAM, 80GB disk, Ubuntu |
| **Database** | PostgreSQL (installed on droplet, not running) |
| **Web server** | nginx (installed on droplet) |

## Routing

| Hostname | Target | What it serves |
|---|---|---|
| `testudo.vip` | Cloudflare Pages | Landing page, docs, pricing, terms, privacy |
| `desk.testudo.vip` | Droplet nginx → static files | testudo-journal SPA |
| `api.testudo.vip` | Droplet nginx → `:8080` | Rust backend (Actix-web) |
| `ws.testudo.vip` | Droplet nginx → `:4000` | WebSocket server (tokio-tungstenite) |

---

## Phase 1: Cloudflare Pages (testudo-web)

### 1.1 Create the Pages project

1. Cloudflare Dashboard → **Workers & Pages** (sidebar)
2. Click **Create application**
3. Select the **Pages** tab at the top
4. Click **Connect to Git**
5. Authorize GitHub, select the `testudo-web` repository
6. Click **Begin setup**

### 1.2 Configure build settings

| Setting | Value |
|---|---|
| Framework preset | None |
| Build command | `bun run build` |
| Build output directory | `dist` |

Expand **Environment variables (advanced)** and add:

| Variable | Value |
|---|---|
| `BUN_VERSION` | `1.1.0` |

Click **Save and Deploy**. First build takes ~1-2 minutes.

### 1.3 Custom domain

1. After deploy finishes, go to project → **Custom domains** tab
2. Click **Set up a custom domain**
3. Enter `testudo.vip`
4. Cloudflare auto-detects it's the DNS provider and adds the CNAME
5. Click **Activate domain**

### 1.4 Verify

- `https://testudo.vip` serves the landing page
- `https://testudo.vip/docs` serves documentation
- `https://testudo.vip/pricing` serves pricing page
- Git push to `testudo-web` master triggers auto-deploy

---

## Phase 2: PostgreSQL

### 2.1 Start and enable

```bash
systemctl start postgresql
systemctl enable postgresql
```

### 2.2 Create database and user

```bash
sudo -u postgres psql <<SQL
CREATE USER testudo WITH PASSWORD 'GENERATE_SECURE_PASSWORD_HERE';
CREATE DATABASE testudo OWNER testudo;
GRANT ALL PRIVILEGES ON DATABASE testudo TO testudo;
SQL
```

### 2.3 Configure authentication

Edit `/etc/postgresql/*/main/pg_hba.conf` — ensure local connections use md5 or scram-sha-256:

```
local   testudo     testudo                             scram-sha-256
```

Reload: `systemctl reload postgresql`

### 2.4 Run migrations

```bash
# Option A: sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres
export DATABASE_URL="postgres://testudo:PASSWORD@localhost/testudo"
cd testudo-exchange/crates/sqlx_postgres
sqlx migrate run

# Option B: from the binary (if it runs migrations on startup)
```

### 2.5 Verify

```bash
psql -U testudo -d testudo -c "SELECT 1;"
```

---

## Phase 3: Backend Services

### 3.1 Build the Rust binary

**Option A: Cross-compile locally (faster)**

```bash
# On local machine (if Linux x86_64, or with cross)
cd testudo-exchange
cargo build --release
scp target/release/testudo-exchange root@DROPLET_IP:/opt/testudo/
scp target/release/ws-stream root@DROPLET_IP:/opt/testudo/
```

**Option B: Build on droplet (simpler, slower)**

```bash
# Install Rust on droplet
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Clone and build
git clone ... /opt/testudo/src
cd /opt/testudo/src/testudo-exchange
cargo build --release
cp target/release/testudo-exchange /opt/testudo/
cp target/release/ws-stream /opt/testudo/
```

### 3.2 Environment file

Create `/opt/testudo/.env`:

```bash
DATABASE_URL=postgres://testudo:PASSWORD@localhost/testudo
RUST_LOG=info
HOST=127.0.0.1
PORT=8080
WS_PORT=4000
JWT_SECRET=GENERATE_SECURE_SECRET_HERE
ENCRYPTION_KEY=GENERATE_32_BYTE_HEX_KEY_HERE
CEX_SIDECAR_URL=http://127.0.0.1:3100
SIDECAR_PSK=GENERATE_SECURE_PSK_HERE
```

### 3.3 Systemd service — API server

Create `/etc/systemd/system/testudo-api.service`:

```ini
[Unit]
Description=Testudo API Server
After=postgresql.service network.target
Requires=postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/testudo
EnvironmentFile=/opt/testudo/.env
ExecStart=/opt/testudo/testudo-exchange
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 3.4 Systemd service — WebSocket server

Create `/etc/systemd/system/testudo-ws.service`:

```ini
[Unit]
Description=Testudo WebSocket Server
After=postgresql.service network.target
Requires=postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/testudo
EnvironmentFile=/opt/testudo/.env
ExecStart=/opt/testudo/ws-stream
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 3.5 CEX Sidecar (Node.js)

```bash
# Install Node.js on droplet (if not present)
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt-get install -y nodejs

# Deploy sidecar
scp -r testudo-ccxt/ root@DROPLET_IP:/opt/testudo/cex-sidecar/
ssh root@DROPLET_IP "cd /opt/testudo/cex-sidecar && npm install --production"
```

Create `/etc/systemd/system/testudo-cex.service`:

```ini
[Unit]
Description=Testudo CEX Sidecar
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/testudo/cex-sidecar
Environment=PORT=3100
Environment=SIDECAR_PSK=SAME_PSK_AS_ABOVE
ExecStart=/usr/bin/node index.js
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 3.6 Enable and start all services

```bash
systemctl daemon-reload
systemctl enable testudo-api testudo-ws testudo-cex
systemctl start testudo-api testudo-ws testudo-cex
```

### 3.7 Verify

```bash
systemctl status testudo-api testudo-ws testudo-cex
curl http://localhost:8080/api/v1/health
curl http://localhost:3100/health
```

---

## Phase 4: Desk (testudo-journal)

### 4.1 Build locally

```bash
cd testudo-journal
bun run build
```

### 4.2 Deploy to droplet

```bash
scp -r dist/ root@DROPLET_IP:/var/www/testudo-journal/
```

### 4.3 Verify

Files should be at `/var/www/testudo-journal/index.html` etc.

---

## Phase 5: Nginx + SSL

### 5.1 Nginx config

Create `/etc/nginx/sites-available/testudo`:

```nginx
# Desk — testudo-journal SPA
server {
    listen 80;
    server_name desk.testudo.vip;

    root /var/www/testudo-journal;
    index index.html;

    # SPA fallback — all routes serve index.html
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Cache static assets
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff2?)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
}

# API — Rust backend
server {
    listen 80;
    server_name api.testudo.vip;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # CORS handled by Actix-web, not nginx
    }
}

# WebSocket
server {
    listen 80;
    server_name ws.testudo.vip;

    location / {
        proxy_pass http://127.0.0.1:4000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Keep WebSocket connections alive
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }
}
```

### 5.2 Enable site

```bash
ln -s /etc/nginx/sites-available/testudo /etc/nginx/sites-enabled/
nginx -t
systemctl reload nginx
```

### 5.3 SSL via Let's Encrypt

```bash
apt install certbot python3-certbot-nginx -y
certbot --nginx -d desk.testudo.vip -d api.testudo.vip -d ws.testudo.vip
```

Certbot auto-configures nginx for HTTPS and sets up auto-renewal.

---

## Phase 6: DNS (Cloudflare)

In Cloudflare Dashboard → `testudo.vip` → DNS:

| Type | Name | Content | Proxy |
|---|---|---|---|
| A | `desk` | `DROPLET_IP` | Proxied (orange cloud) |
| A | `api` | `DROPLET_IP` | DNS only (grey cloud)* |
| A | `ws` | `DROPLET_IP` | DNS only (grey cloud)* |

*API and WS should be DNS-only to avoid Cloudflare intercepting/buffering WebSocket connections and API requests. The droplet handles SSL via certbot directly.

Alternative: If you want Cloudflare's WAF/DDoS protection on API, set to Proxied but configure Cloudflare to allow WebSocket connections (it does by default on Pro+, but can be flaky on Free).

---

## Phase 7: Frontend Configuration Updates

Update hardcoded URLs in frontend code:

### testudo-journal (Desk)

API base URL needs to point to `https://api.testudo.vip`
WebSocket URL needs to point to `wss://ws.testudo.vip`

### testudo-extension

API base URL: `https://api.testudo.vip`
WebSocket URL: `wss://ws.testudo.vip`

### testudo-web (Astro dev proxy — dev only)

The `astro.config.mjs` proxy routes (`/desk` → localhost:3002, `/api` → localhost:8080) are dev-only and don't affect production.

---

## Acceptance Criteria

- [ ] `https://testudo.vip` serves landing page from Cloudflare Pages
- [ ] `https://testudo.vip/docs` serves documentation
- [ ] `https://desk.testudo.vip` serves the Desk SPA
- [ ] `https://api.testudo.vip/api/v1/health` returns 200
- [ ] WebSocket connects at `wss://ws.testudo.vip`
- [ ] SIWE login works end-to-end (wallet → nonce → verify → cookies)
- [ ] Extension pairs with Desk via 6-digit code
- [ ] Git push to `testudo-web` auto-deploys landing page
- [ ] All HTTPS, no mixed content warnings
- [ ] Services survive droplet reboot (systemd enabled)

---

## Cost

| Item | Cost |
|---|---|
| Cloudflare Pages | Free |
| DigitalOcean droplet | $12-24/mo (existing) |
| Domain renewal | ~$10/yr |
| SSL certificates | Free (Let's Encrypt) |
| **Total** | **~$12-24/mo** |
