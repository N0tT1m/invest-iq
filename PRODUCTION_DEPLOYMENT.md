# Production Deployment Guide

This guide covers deploying InvestIQ to production with security best practices.

## 🔐 Security Features

### API Authentication

InvestIQ uses API key authentication to protect all endpoints (except health checks).

**Generating Secure API Keys:**

```bash
# Generate a secure random API key (recommended)
openssl rand -hex 32

# Or use this alternative
python3 -c "import secrets; print(secrets.token_hex(32))"
```

**Configuring API Keys:**

Add to your `.env` file:

```env
API_KEYS=key1_generated_above,key2_generated_above,key3_for_frontend
```

**Using API Keys:**

Clients can authenticate using any of these methods:

1. **X-API-Key header** (recommended):
   ```bash
   curl -H "X-API-Key: your_api_key" http://localhost:3000/api/analyze/AAPL
   ```

2. **Authorization: Bearer header**:
   ```bash
   curl -H "Authorization: Bearer your_api_key" http://localhost:3000/api/analyze/AAPL
   ```

3. **Query parameter** (not recommended for production):
   ```bash
   curl http://localhost:3000/api/analyze/AAPL?api_key=your_api_key
   ```

### Rate Limiting

Protects against API abuse with configurable per-IP rate limits.

**Configuration:**

```env
# Allow 60 requests per minute per IP (default)
RATE_LIMIT_PER_MINUTE=60
```

**Burst Handling:**
- Allows burst of 10 requests
- Returns HTTP 429 (Too Many Requests) when limit exceeded

### CORS Configuration

Restricts which domains can access your API.

**Configuration:**

```env
# Development (default)
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8050

# Production (update to your domains)
ALLOWED_ORIGINS=https://yourdomain.com,https://app.yourdomain.com
```

**Allowed Methods:** GET, POST, OPTIONS
**Allowed Headers:** Content-Type, Authorization, X-API-Key

## 🐳 Docker Deployment

### Quick Start

1. **Configure environment:**
   ```bash
   cp .env.example .env
   # Edit .env with your API keys and configuration
   ```

2. **Generate secure API keys:**
   ```bash
   echo "API_KEYS=$(openssl rand -hex 32),$(openssl rand -hex 32)" >> .env
   ```

3. **Build and start services:**
   ```bash
   # Start API server and Redis
   docker-compose up -d

   # Or start with Discord bot
   docker-compose --profile discord up -d
   ```

4. **Verify deployment:**
   ```bash
   # Check health
   curl http://localhost:3000/health

   # Test authenticated endpoint
   curl -H "X-API-Key: your_key_here" http://localhost:3000/api/analyze/AAPL
   ```

### Docker Compose Services

```yaml
services:
  redis:        # Cache layer (persistent storage)
  api-server:   # Main REST API
  discord-bot:  # Discord integration (optional, use --profile discord)
```

### Environment Variables

**Required:**
- `POLYGON_API_KEY` - Polygon.io API key
- `API_KEYS` - Comma-separated API keys for authentication

**Optional:**
- `REDIS_URL` - Redis connection URL (default: redis://redis:6379)
- `RUST_LOG` - Log level (default: info)
- `RATE_LIMIT_PER_MINUTE` - Rate limit (default: 60)
- `ALLOWED_ORIGINS` - CORS allowed origins
- `DISCORD_BOT_TOKEN` - Discord bot token (if using Discord bot)
- `ALPHA_VANTAGE_API_KEY` - For validation features

### Production Docker Build

**Build images:**

```bash
# API Server
docker build -f Dockerfile.api -t investiq-api:latest .

# Discord Bot
docker build -f Dockerfile.discord -t investiq-discord:latest .
```

**Tag for registry:**

```bash
docker tag investiq-api:latest your-registry.com/investiq-api:v1.0.0
docker tag investiq-discord:latest your-registry.com/investiq-discord:v1.0.0
```

**Push to registry:**

```bash
docker push your-registry.com/investiq-api:v1.0.0
docker push your-registry.com/investiq-discord:v1.0.0
```

## 🚀 Kubernetes Deployment

### Prerequisites

- Kubernetes cluster (v1.25+)
- kubectl configured
- Docker images pushed to registry

### Create Secrets

```bash
# Create namespace
kubectl create namespace investiq

# Create secrets
kubectl create secret generic investiq-secrets \
  --from-literal=polygon-api-key=YOUR_POLYGON_KEY \
  --from-literal=api-keys=KEY1,KEY2,KEY3 \
  -n investiq

# Optional: Discord bot token
kubectl create secret generic investiq-discord-secrets \
  --from-literal=discord-bot-token=YOUR_DISCORD_TOKEN \
  -n investiq
```

### Deploy Redis

```yaml
# redis.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: redis
  namespace: investiq
spec:
  replicas: 1
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        ports:
        - containerPort: 6379
        volumeMounts:
        - name: redis-data
          mountPath: /data
        command: ["redis-server", "--appendonly", "yes"]
      volumes:
      - name: redis-data
        persistentVolumeClaim:
          claimName: redis-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: redis
  namespace: investiq
spec:
  selector:
    app: redis
  ports:
  - port: 6379
    targetPort: 6379
```

### Deploy API Server

```yaml
# api-server.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api-server
  namespace: investiq
spec:
  replicas: 3  # Scale as needed
  selector:
    matchLabels:
      app: api-server
  template:
    metadata:
      labels:
        app: api-server
    spec:
      containers:
      - name: api-server
        image: your-registry.com/investiq-api:v1.0.0
        ports:
        - containerPort: 3000
        env:
        - name: POLYGON_API_KEY
          valueFrom:
            secretKeyRef:
              name: investiq-secrets
              key: polygon-api-key
        - name: API_KEYS
          valueFrom:
            secretKeyRef:
              name: investiq-secrets
              key: api-keys
        - name: REDIS_URL
          value: redis://redis:6379
        - name: RUST_LOG
          value: info
        - name: RATE_LIMIT_PER_MINUTE
          value: "60"
        - name: ALLOWED_ORIGINS
          value: https://yourdomain.com
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: api-server
  namespace: investiq
spec:
  selector:
    app: api-server
  ports:
  - port: 3000
    targetPort: 3000
  type: LoadBalancer  # Or ClusterIP with Ingress
```

### Apply Manifests

```bash
kubectl apply -f redis.yaml
kubectl apply -f api-server.yaml

# Check status
kubectl get pods -n investiq
kubectl get services -n investiq
```

## 🔒 Security Best Practices

### 1. API Key Management

- ✅ Generate cryptographically secure random keys (32+ bytes)
- ✅ Store keys in environment variables, never in code
- ✅ Use different keys for different clients/environments
- ✅ Rotate keys regularly (every 90 days recommended)
- ✅ Use secrets management (Kubernetes Secrets, AWS Secrets Manager, etc.)

### 2. Network Security

- ✅ Use HTTPS in production (configure reverse proxy like nginx)
- ✅ Restrict CORS to specific domains
- ✅ Use rate limiting to prevent abuse
- ✅ Deploy behind a firewall/security group
- ✅ Enable DDoS protection (CloudFlare, AWS Shield, etc.)

### 3. Container Security

- ✅ Run containers as non-root user (already configured)
- ✅ Use minimal base images (Debian slim)
- ✅ Scan images for vulnerabilities (GitHub Actions includes this)
- ✅ Keep dependencies updated
- ✅ Use read-only file systems where possible

### 4. Monitoring & Logging

- ✅ Configure RUST_LOG appropriately (info in prod, debug for troubleshooting)
- ✅ Set up log aggregation (ELK stack, CloudWatch, etc.)
- ✅ Monitor API metrics (implement Prometheus in Month 1)
- ✅ Set up alerting for errors and anomalies
- ✅ Track API usage per key

### 5. Data Protection

- ✅ Enable Redis persistence (AOF enabled in docker-compose)
- ✅ Regular backups of Redis data
- ✅ Encrypt sensitive data at rest
- ✅ Use TLS for Redis connections in production

## 📊 Monitoring

### Health Checks

```bash
# Basic health check
curl http://your-server:3000/health

# Expected response:
# {"success":true,"data":{"status":"healthy","service":"invest-iq-api"}}
```

### Logs

```bash
# Docker Compose
docker-compose logs -f api-server

# Kubernetes
kubectl logs -f deployment/api-server -n investiq
```

### Metrics (Future)

After implementing Prometheus (Month 1):
- Request rate and latency
- Error rates by endpoint
- Cache hit/miss ratio
- Rate limit hits
- Active connections

## 🔄 CI/CD Pipeline

GitHub Actions workflow automatically:
1. Runs tests on every push/PR
2. Checks code formatting and linting
3. Runs security audit (cargo audit)
4. Builds Docker images
5. Pushes to Docker Hub (on main branch)

**Required GitHub Secrets:**
- `DOCKER_USERNAME` - Docker Hub username
- `DOCKER_PASSWORD` - Docker Hub password/token

## 🆘 Troubleshooting

### Authentication Issues

```bash
# Test with correct API key
curl -H "X-API-Key: your_key" http://localhost:3000/api/analyze/AAPL

# 401 Unauthorized - Missing API key
# 403 Forbidden - Invalid API key
# 200 OK - Success
```

### Rate Limiting

```bash
# Check if you're being rate limited (HTTP 429)
# Wait 60 seconds or adjust RATE_LIMIT_PER_MINUTE
```

### CORS Errors

```bash
# Add your domain to ALLOWED_ORIGINS
ALLOWED_ORIGINS=https://yourfrontend.com,https://yourdomain.com
```

### Redis Connection

```bash
# Test Redis connection
docker-compose exec redis redis-cli ping
# Should return: PONG

# Check API server logs for Redis connection status
docker-compose logs api-server | grep -i redis
```

## 📝 Maintenance

### Updating

```bash
# Pull latest changes
git pull

# Rebuild images
docker-compose build

# Restart services with zero downtime (if load balanced)
docker-compose up -d --no-deps --build api-server

# Or restart all
docker-compose down && docker-compose up -d
```

### Backup

```bash
# Backup Redis data
docker-compose exec redis redis-cli BGSAVE

# Copy backup
docker cp investiq-redis:/data/dump.rdb ./backups/dump-$(date +%Y%m%d).rdb
```

### Log Rotation

Configure Docker log rotation:

```json
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  }
}
```

## 🎯 Next Steps

After Week 1 deployment, proceed with:

**Month 1 Priorities:**
1. Add Prometheus metrics
2. Implement PostgreSQL persistence
3. Add retry logic and circuit breakers
4. Set up error aggregation (Sentry)
5. Implement backup strategy
6. Add integration tests
7. Create API documentation (OpenAPI)

See main README roadmap for complete production readiness timeline.
