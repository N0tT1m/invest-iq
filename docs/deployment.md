# InvestIQ Deployment Guide

## Quick Start (Docker)

```bash
# Copy and configure environment
cp .env.example .env
# Edit .env with your API keys

# Start core services (API + Redis + ML)
docker compose up -d

# Include trading agent
docker compose --profile agent up -d

# Include database backups
docker compose --profile backup up -d
```

## Environment Variables

### Required
| Variable | Description |
|----------|-------------|
| `POLYGON_API_KEY` | Polygon.io API key (required) |

### Authentication
| Variable | Default | Description |
|----------|---------|-------------|
| `API_KEYS` | _(empty)_ | Comma-separated API keys. If empty, auth is disabled |
| `REQUIRE_AUTH` | `false` | Set `true` to require API_KEYS in production |
| `LIVE_TRADING_KEY` | _(empty)_ | Extra header key for broker write endpoints |

### Trading (Alpaca)
| Variable | Default | Description |
|----------|---------|-------------|
| `ALPACA_API_KEY` | _(empty)_ | Alpaca API key |
| `ALPACA_SECRET_KEY` | _(empty)_ | Alpaca secret key |
| `ALPACA_BASE_URL` | `https://paper-api.alpaca.markets` | Paper or live URL |
| `LIVE_TRADING_APPROVED` | _(empty)_ | Must be `yes` for live trading |

### Infrastructure
| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:portfolio.db` | SQLite database path |
| `REDIS_URL` | _(empty)_ | Redis URL (falls back to in-memory cache) |
| `RATE_LIMIT_PER_MINUTE` | `60` | API rate limit per IP |
| `ALLOWED_ORIGINS` | localhost:3000,8050,8051,8052 | CORS origins |
| `RUST_LOG` | `info` | Log level filter |
| `RUST_LOG_FORMAT` | _(empty)_ | Set `json` for structured JSON logs |

### Optional Services
| Variable | Description |
|----------|-------------|
| `FINNHUB_API_KEY` | Finnhub news (additive to Polygon) |
| `ALPHA_VANTAGE_API_KEY` | Validation engine |
| `DISCORD_WEBHOOK_URL` | Trading agent Discord notifications |

## TLS/HTTPS Setup

InvestIQ does not terminate TLS directly. Use a reverse proxy.

### Option 1: Nginx + Let's Encrypt (Recommended)

```nginx
server {
    listen 443 ssl http2;
    server_name investiq.example.com;

    ssl_certificate /etc/letsencrypt/live/investiq.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/investiq.example.com/privkey.pem;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;

    # API backend
    location /api/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-Ip $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # Health check (no auth)
    location /health {
        proxy_pass http://127.0.0.1:3000;
    }

    # Frontend
    location / {
        proxy_pass http://127.0.0.1:8050;
        proxy_set_header Host $host;
        proxy_set_header X-Real-Ip $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}

server {
    listen 80;
    server_name investiq.example.com;
    return 301 https://$host$request_uri;
}
```

Install certbot and get certificates:
```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d investiq.example.com
```

### Option 2: Caddy (Auto TLS)

```Caddyfile
investiq.example.com {
    handle /api/* {
        reverse_proxy localhost:3000
    }
    handle /health {
        reverse_proxy localhost:3000
    }
    handle {
        reverse_proxy localhost:8050
    }
}
```

Caddy automatically obtains and renews TLS certificates.

### Option 3: Docker with Traefik

Add to `docker-compose.yml`:
```yaml
services:
  traefik:
    image: traefik:v3.0
    command:
      - "--providers.docker=true"
      - "--entrypoints.websecure.address=:443"
      - "--certificatesresolvers.letsencrypt.acme.tlschallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.email=you@example.com"
      - "--certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json"
    ports:
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - letsencrypt:/letsencrypt
```

Then add labels to `api-server`:
```yaml
labels:
  - "traefik.http.routers.api.rule=Host(`investiq.example.com`) && PathPrefix(`/api`)"
  - "traefik.http.routers.api.tls.certresolver=letsencrypt"
```

## Log Rotation

### Docker
Already configured in `docker-compose.yml` via the `json-file` driver:
- API server: 50MB x 5 files
- ML service: 20MB x 3 files

### Bare Metal
```bash
sudo cp scripts/investiq-logrotate.conf /etc/logrotate.d/investiq
```

For JSON log collection with external tools:
```bash
RUST_LOG_FORMAT=json ./api-server 2>&1 | tee -a /var/log/investiq/api.log
```

## Database Backup

### Docker
Enable the backup profile:
```bash
docker compose --profile backup up -d
```
Backups are stored in the `db_backups` volume, rotated daily (7-day retention).

### Bare Metal
```bash
# One-time backup
./scripts/backup-db.sh ./backups

# Cron (daily at 2am)
echo "0 2 * * * /path/to/investiq/scripts/backup-db.sh /path/to/backups" | crontab -
```

### Restore
```bash
# Stop the API server first
cp backups/portfolio_20260210_020000.db portfolio.db
```

## Health Checks

- `GET /health` — Returns dependency status (DB, Polygon, Redis, Alpaca, ML)
- `GET /metrics` — Request count, error count, latency histogram
- ML service: `GET http://localhost:8004/health`
