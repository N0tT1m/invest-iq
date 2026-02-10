# InvestIQ Complete Setup Guide

This guide will walk you through setting up the entire InvestIQ stack: Rust backend, API server, Discord bot, and Dash frontend.

## 📋 Prerequisites

### Required
- **Rust** 1.70+ - [Install from rustup.rs](https://rustup.rs/)
- **Python** 3.8+ - [Download from python.org](https://www.python.org/downloads/)
- **Polygon.io API Key** - [Get free key](https://polygon.io/dashboard/signup)

### Optional
- **Docker** - For Redis caching
- **Discord Bot Token** - For Discord integration

## 🚀 Quick Start (5 Minutes)

### Step 1: Get API Keys

1. **Polygon.io** (Required)
   - Sign up at https://polygon.io/dashboard/signup
   - Get your free API key from the dashboard
   - Free tier: 5 API calls/minute

2. **Discord Bot** (Optional)
   - Go to https://discord.com/developers/applications
   - Create new application → Add Bot → Copy token
   - Enable "Message Content Intent" under Bot settings

### Step 2: Configure Environment

```bash
# Copy environment template
cp .env.example .env

# Edit .env and add your keys
# POLYGON_API_KEY=your_key_here
# DISCORD_BOT_TOKEN=your_token_here (optional)
```

### Step 3: Start Redis (Optional but Recommended)

```bash
# Start Redis using Docker
docker-compose up -d

# Verify Redis is running
docker ps
```

> **Note**: If you skip Redis, the system will use in-memory caching automatically.

### Step 4: Build and Start Backend

```bash
# Build in release mode for best performance
cargo build --release

# Start the API server
cargo run --release --bin api-server
```

You should see:
```
✅ Connected to Redis at redis://localhost:6379
🚀 API Server starting on 0.0.0.0:3000
```

### Step 5: Start Frontend

**Linux/Mac:**
```bash
cd frontend
./start.sh
```

**Windows:**
```bash
cd frontend
start.bat
```

**Manual:**
```bash
cd frontend
pip install -r requirements.txt
python app.py
```

### Step 6: Access the Dashboard

Open your browser to: **http://localhost:8050**

That's it! You're ready to analyze stocks! 🎉

## 📊 Testing the System

### Test the API

```bash
# Health check
curl http://localhost:3000/health

# Analyze Apple stock
curl http://localhost:3000/api/analyze/AAPL | jq

# Get historical data
curl "http://localhost:3000/api/bars/AAPL?timeframe=1d&days=90" | jq
```

### Test the Dashboard

1. Open http://localhost:8050
2. Enter symbol: `AAPL`
3. Select timeframe: `1d`
4. Set days: `90`
5. Click **Analyze**

You should see:
- Overall signal (Buy/Sell/Hold)
- Interactive candlestick chart
- Technical indicators (RSI, MACD)
- Fundamental metrics
- Quantitative analysis
- Sentiment breakdown

## 🤖 Discord Bot Setup (Optional)

### Step 1: Invite Bot to Server

1. Go to Discord Developer Portal
2. OAuth2 → URL Generator
3. Select scopes: `bot`
4. Select permissions: `Send Messages`, `Read Message History`
5. Copy generated URL and open in browser
6. Select your server and authorize

### Step 2: Start the Bot

```bash
cargo run --release --bin discord-bot
```

### Step 3: Use Commands

In Discord:
```
!iq analyze AAPL
!iq help
```

## 🔧 Advanced Configuration

### API Server Configuration

Edit `crates/api-server/src/main.rs` to change:

```rust
// Change port
let addr = "0.0.0.0:3000";  // Change to your port

// Change cache TTL default
fn default_cache_duration() -> u64 {
    300  // Change to your TTL in seconds
}
```

### Dashboard Configuration

Edit `frontend/app.py` to change:

```python
# API endpoint
API_BASE_URL = "http://localhost:3000"  # Change if API is on different host/port

# Dashboard port
app.run_server(debug=True, host='0.0.0.0', port=8050)  # Change port here
```

### Redis Configuration

Edit `docker-compose.yml` for Redis settings:

```yaml
services:
  redis:
    ports:
      - "6379:6379"  # Change external port if needed
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru
```

## 📈 Performance Tuning

### For Production

1. **Use Redis**
   - Enables distributed caching
   - Faster than in-memory for multiple instances
   - Persists across restarts

2. **Increase Cache TTL**
   ```bash
   # In API requests
   curl "http://localhost:3000/api/analyze/AAPL?cache_ttl=600"
   ```

3. **Run Multiple API Instances**
   ```bash
   # Terminal 1
   API_PORT=3000 cargo run --release --bin api-server

   # Terminal 2
   API_PORT=3001 cargo run --release --bin api-server

   # Use nginx/HAProxy to load balance
   ```

4. **Optimize Build**
   ```bash
   # Enable LTO and codegen optimizations
   RUSTFLAGS="-C target-cpu=native" cargo build --release
   ```

### For Development

1. **Use Debug Mode**
   ```bash
   # Faster compilation
   cargo run --bin api-server
   ```

2. **Enable Logging**
   ```bash
   RUST_LOG=debug cargo run --bin api-server
   ```

3. **Hot Reload Dashboard**
   ```python
   # In app.py, debug=True enables hot reload
   app.run_server(debug=True, ...)
   ```

## 🐛 Troubleshooting

### API Server Issues

**"POLYGON_API_KEY must be set"**
```bash
# Make sure .env file exists and has the key
cat .env | grep POLYGON_API_KEY
```

**"Address already in use"**
```bash
# Find process using port 3000
lsof -i :3000  # Mac/Linux
netstat -ano | findstr :3000  # Windows

# Kill the process or change the port
```

**"Failed to connect to Redis"**
```bash
# System will fall back to in-memory cache
# To fix, start Redis:
docker-compose up -d
```

### Dashboard Issues

**"Module not found" errors**
```bash
cd frontend
pip install -r requirements.txt
```

**"Connection refused" when analyzing**
```bash
# Make sure API server is running
curl http://localhost:3000/health
```

**Charts not displaying**
```bash
# Clear browser cache
# Check browser console for errors
# Try different browser
```

**Slow analysis**
```bash
# Reduce lookback period (use 30-90 days)
# Enable Redis caching
# Use higher timeframes (1d instead of 1m)
```

### Discord Bot Issues

**Bot not responding**
```bash
# Check Message Content Intent is enabled
# Verify bot has permissions in server
# Check bot is online in Discord
```

**"DISCORD_BOT_TOKEN must be set"**
```bash
# Add token to .env file
echo "DISCORD_BOT_TOKEN=your_token" >> .env
```

### Polygon API Issues

**Rate limiting (429 errors)**
```bash
# Free tier: 5 calls/minute
# Solution 1: Increase cache TTL
# Solution 2: Upgrade Polygon plan
# Solution 3: Reduce analysis frequency
```

**"Insufficient data" errors**
```bash
# Try more liquid stocks (AAPL, MSFT, GOOGL)
# Reduce lookback period
# Use daily timeframe instead of intraday
```

## 🔐 Security Best Practices

1. **Never commit .env file**
   - Already in .gitignore
   - Use separate keys for dev/prod

2. **Use environment variables**
   ```bash
   export POLYGON_API_KEY=xxx
   export DISCORD_BOT_TOKEN=xxx
   cargo run --release --bin api-server
   ```

3. **Restrict CORS in production**
   ```rust
   // In api-server/src/main.rs
   let cors = CorsLayer::new()
       .allow_origin("https://yourdomain.com".parse::<HeaderValue>().unwrap())
       // instead of .allow_origin(Any)
   ```

4. **Use HTTPS in production**
   - Put behind nginx/Caddy
   - Use Let's Encrypt for SSL

## 📦 Deployment

### Deploy to Cloud (AWS/GCP/Azure)

1. **Build Docker Images**
   ```dockerfile
   # Dockerfile for API
   FROM rust:1.70 as builder
   WORKDIR /app
   COPY . .
   RUN cargo build --release --bin api-server

   FROM debian:bookworm-slim
   COPY --from=builder /app/target/release/api-server /usr/local/bin/
   CMD ["api-server"]
   ```

2. **Use Managed Redis**
   - AWS ElastiCache
   - GCP Cloud Memorystore
   - Azure Cache for Redis

3. **Use Container Orchestration**
   - Kubernetes
   - AWS ECS
   - Google Cloud Run

### Deploy to VPS

```bash
# On your server
git clone your-repo
cd invest-iq

# Setup
cp .env.example .env
# Edit .env with your keys

# Build
cargo build --release

# Run with systemd
sudo cp systemd/investiq-api.service /etc/systemd/system/
sudo systemctl enable investiq-api
sudo systemctl start investiq-api
```

## 📚 Next Steps

### Customize Analysis
- Add custom technical indicators
- Adjust analysis weights
- Create custom strategies

### Extend Dashboard
- Add more chart types
- Implement stock screener
- Add portfolio tracking
- Create alerts system

### Scale System
- Add WebSocket for real-time data
- Implement database for historical analysis
- Add user authentication
- Create mobile app

## 🆘 Getting Help

1. **Check Documentation**
   - README.md - Project overview
   - ARCHITECTURE.md - System design
   - QUICKSTART.md - Fast setup

2. **Common Issues**
   - GitHub Issues
   - Error messages in logs

3. **Community**
   - GitHub Discussions
   - Stack Overflow

## 📝 Summary Checklist

- [ ] Install Rust, Python, Docker
- [ ] Get Polygon API key
- [ ] Clone repository
- [ ] Copy and edit .env file
- [ ] Start Redis (optional)
- [ ] Build Rust backend
- [ ] Start API server (port 3000)
- [ ] Install Python dependencies
- [ ] Start Dash frontend (port 8050)
- [ ] Test with AAPL
- [ ] (Optional) Setup Discord bot

## 🎉 Success Criteria

You've successfully set up InvestIQ when:

✅ API health check returns 200
✅ Dashboard loads at localhost:8050
✅ Can analyze AAPL and see results
✅ Charts display with data
✅ All 4 analysis types show results
✅ No errors in console/logs

---

**Need help?** Check the troubleshooting section or open an issue on GitHub!

Built with ❤️ in Rust & Python
