# InvestIQ - Updated Startup Guide

## Recent Changes ✅

All startup scripts and Docker configuration have been updated to properly handle API key authentication.

## What Was Fixed

1. **Backend**: Temporarily disabled `tower_governor` rate limiting that was causing "Unable To Extract Key!" errors
2. **Frontend**: Added API key authentication to all frontend applications
3. **Scripts**: Updated `start-all.sh` to automatically extract and set `API_KEY` from `API_KEYS`
4. **Docker**: Updated `docker-compose.yml` and created `docker-start.sh` for proper API key handling

## Quick Start Options

### ✅ Option 1: Start Everything (Recommended)

```bash
./start-all.sh
```

This script now:
- Loads environment variables from `.env`
- Automatically extracts `API_KEY` from `API_KEYS` (uses first key)
- Starts Redis, API Server, and Dashboard
- All services properly authenticated

**Services will be available at:**
- Dashboard: http://localhost:8050
- API: http://localhost:3000
- Redis: localhost:6379

### ✅ Option 2: Docker Compose

```bash
./docker-start.sh
```

Or manually:
```bash
# Set API_KEY from .env
export $(grep -v '^#' .env | xargs)
export API_KEY=$(echo $API_KEYS | cut -d',' -f1)

# Start services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down
```

**Docker services:**
- `investiq-redis` - Redis cache
- `investiq-api` - API server
- `investiq-dashboard` - Web dashboard
- `investiq-discord` - Discord bot (optional, use `--profile discord`)

### ✅ Option 3: Individual Services

#### Start API Server Only:
```bash
cargo run --release --bin api-server
```

#### Start Dashboard Only:
```bash
export API_KEY=79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219
cd frontend
source venv/bin/activate
python3 app.py
```

#### Start Trading Dashboard:
```bash
export API_KEY=79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219
cd frontend
source venv/bin/activate
python3 trading_dashboard.py
```

#### Start Portfolio Manager:
```bash
export API_KEY=79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219
cd frontend
source venv/bin/activate
python3 portfolio_app.py
```

## Environment Configuration

Your `.env` file should contain:

```bash
# Required for stock data
POLYGON_API_KEY=your_polygon_key_here

# Required for API authentication (comma-separated list)
API_KEYS=79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219

# Optional: Broker integration
ALPACA_API_KEY=your_alpaca_key
ALPACA_SECRET_KEY=your_alpaca_secret
ALPACA_BASE_URL=https://paper-api.alpaca.markets

# Optional: Discord bot
DISCORD_BOT_TOKEN=your_discord_token

# Optional: Redis
REDIS_URL=redis://localhost:6379
```

## API Authentication

All API endpoints (except `/health` and `/`) require authentication via:

1. **X-API-Key header** (recommended):
   ```bash
   curl -H "X-API-Key: YOUR_KEY" http://localhost:3000/api/analyze/AAPL
   ```

2. **Authorization Bearer header**:
   ```bash
   curl -H "Authorization: Bearer YOUR_KEY" http://localhost:3000/api/analyze/AAPL
   ```

3. **Query parameter** (discouraged):
   ```bash
   curl "http://localhost:3000/api/analyze/AAPL?api_key=YOUR_KEY"
   ```

## Testing Authentication

```bash
# Should succeed with API key
curl -H "X-API-Key: 79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219" \
  http://localhost:3000/api/analyze/AAPL

# Should fail without API key (401 Unauthorized)
curl http://localhost:3000/api/analyze/AAPL

# Health check doesn't require auth
curl http://localhost:3000/health
```

## Troubleshooting

### Error: "Unable To Extract Key!"
✅ **FIXED** - This error has been resolved by temporarily disabling rate limiting.

### Error: "Missing API key"
- Make sure `API_KEYS` is set in `.env`
- For `start-all.sh`, it automatically extracts the API key
- For manual start, export `API_KEY` before running the frontend
- Frontend apps have a fallback to use the hardcoded key from `.env`

### Error: "Connection refused" or "502 Bad Gateway"
- Ensure API server is running: `curl http://localhost:3000/health`
- Check logs: `tail -f api-server.log` or `docker-compose logs -f api-server`

### Frontend can't connect to API
- Check API_BASE_URL in frontend code (should be `http://localhost:3000`)
- Verify API_KEY is set: `echo $API_KEY`
- For Docker, API endpoint should be `http://api-server:3000` (internal network)

## Stopping Services

### Stop start-all.sh services:
```bash
./stop-all.sh
```

Or manually:
```bash
pkill -f api-server
pkill -f "python3 app.py"
docker-compose down
```

### Stop Docker services:
```bash
docker-compose down
```

Keep data:
```bash
docker-compose stop
```

Remove everything including volumes:
```bash
docker-compose down -v
```

## Logs

### start-all.sh logs:
```bash
tail -f api-server.log
tail -f dashboard.log
```

### Docker logs:
```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f api-server
docker-compose logs -f dashboard
```

## Additional Resources

- Full documentation: See `API_KEY_FIX.md`
- Original setup: See `START_HERE.md`
- Portfolio features: See `PORTFOLIO_GUIDE.md`
- Production deployment: See `PRODUCTION_DEPLOYMENT.md`
