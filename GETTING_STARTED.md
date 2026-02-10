# Getting Started with InvestIQ

Quick start guide for InvestIQ stock analysis platform after Week 1 production improvements.

## 🚀 Quick Start (5 Minutes)

### Option 1: Docker (Recommended)

1. **Get API keys:**
   - Sign up for [Polygon.io](https://polygon.io) (free tier available)
   - Get your API key from the dashboard

2. **Configure environment:**
   ```bash
   cp .env.example .env
   ```

3. **Edit `.env` file:**
   ```env
   # Required: Polygon API key
   POLYGON_API_KEY=your_polygon_key_here

   # Required: API authentication keys (generate with openssl)
   API_KEYS=your_generated_key_1,your_generated_key_2
   ```

4. **Generate secure API keys:**
   ```bash
   # Generate two random API keys
   openssl rand -hex 32
   openssl rand -hex 32
   # Copy the output to API_KEYS in .env
   ```

5. **Start services:**
   ```bash
   docker-compose up -d
   ```

6. **Test it:**
   ```bash
   # Health check (no auth required)
   curl http://localhost:3000/health

   # Analyze a stock (requires your API key)
   curl -H "X-API-Key: your_key_from_env" \
        http://localhost:3000/api/analyze/AAPL
   ```

### Option 2: Native Build

1. **Prerequisites:**
   - Rust 1.70+ ([rustup.rs](https://rustup.rs/))
   - Redis (optional, for caching)

2. **Setup:**
   ```bash
   cp .env.example .env
   # Edit .env with your API keys (see Docker option above)
   ```

3. **Build:**
   ```bash
   cargo build --release
   ```

4. **Run:**
   ```bash
   # Terminal 1: Start Redis (optional)
   redis-server

   # Terminal 2: Start API server
   cargo run --release --bin api-server

   # Terminal 3 (optional): Start Discord bot
   cargo run --release --bin discord-bot
   ```

## 📖 Using the API

### Authentication

All API endpoints (except `/health`) require authentication using an API key.

**Three ways to authenticate:**

1. **X-API-Key header (recommended):**
   ```bash
   curl -H "X-API-Key: your_api_key" \
        http://localhost:3000/api/analyze/AAPL
   ```

2. **Authorization: Bearer token:**
   ```bash
   curl -H "Authorization: Bearer your_api_key" \
        http://localhost:3000/api/analyze/AAPL
   ```

3. **Query parameter (not recommended for production):**
   ```bash
   curl "http://localhost:3000/api/analyze/AAPL?api_key=your_api_key"
   ```

### Common Endpoints

**Health Check:**
```bash
curl http://localhost:3000/health
```

**Analyze a Stock:**
```bash
curl -H "X-API-Key: YOUR_KEY" \
     http://localhost:3000/api/analyze/AAPL
```

**Get Stock Suggestions:**
```bash
curl -H "X-API-Key: YOUR_KEY" \
     "http://localhost:3000/api/suggest?universe=tech&limit=10"
```

**Get Historical Data:**
```bash
curl -H "X-API-Key: YOUR_KEY" \
     "http://localhost:3000/api/bars/AAPL?timeframe=1d&days=90"
```

**Run Backtest:**
```bash
curl -H "X-API-Key: YOUR_KEY" \
     "http://localhost:3000/api/backtest/AAPL?days=365"
```

### Response Format

All successful responses follow this format:

```json
{
  "success": true,
  "data": {
    "symbol": "AAPL",
    "overall_signal": "Buy",
    "overall_confidence": 0.75,
    "technical": { ... },
    "fundamental": { ... },
    "quantitative": { ... },
    "sentiment": { ... }
  }
}
```

Error responses:

```json
{
  "success": false,
  "error": "Error message here"
}
```

## 🎮 Discord Bot

If you want to use the Discord integration:

1. **Create a Discord bot:**
   - Go to [Discord Developer Portal](https://discord.com/developers/applications)
   - Create new application
   - Go to "Bot" tab → "Add Bot"
   - Copy the token

2. **Add to .env:**
   ```env
   DISCORD_BOT_TOKEN=your_discord_bot_token
   ```

3. **Start the bot:**
   ```bash
   # Docker
   docker-compose --profile discord up -d

   # Native
   cargo run --release --bin discord-bot
   ```

4. **Use in Discord:**
   ```
   !iq analyze AAPL
   !iq chart TSLA
   !iq help
   ```

## 🐳 Docker Commands

**Start all services:**
```bash
docker-compose up -d
```

**Start with Discord bot:**
```bash
docker-compose --profile discord up -d
```

**View logs:**
```bash
docker-compose logs -f api-server
docker-compose logs -f discord-bot
```

**Stop services:**
```bash
docker-compose down
```

**Rebuild after code changes:**
```bash
docker-compose build
docker-compose up -d
```

## 🔧 Configuration

### Environment Variables

**Required:**
- `POLYGON_API_KEY` - Your Polygon.io API key
- `API_KEYS` - Comma-separated list of valid API keys for authentication

**Optional:**
- `REDIS_URL` - Redis connection (default: redis://localhost:6379)
- `RUST_LOG` - Log level: trace, debug, info, warn, error (default: info)
- `RATE_LIMIT_PER_MINUTE` - Requests per minute per IP (default: 60)
- `ALLOWED_ORIGINS` - CORS allowed origins (default: http://localhost:3000,http://localhost:8050)
- `DISCORD_BOT_TOKEN` - Discord bot token (optional)
- `ALPHA_VANTAGE_API_KEY` - For validation features (optional)

### Security Settings

**Production CORS:**
```env
ALLOWED_ORIGINS=https://yourdomain.com,https://app.yourdomain.com
```

**Adjust rate limiting:**
```env
RATE_LIMIT_PER_MINUTE=100  # Allow 100 requests/minute
```

**Enable debug logging:**
```env
RUST_LOG=debug
```

## 🧪 Testing

**Run all tests:**
```bash
cargo test
```

**Run specific crate tests:**
```bash
cargo test -p technical-analysis
cargo test -p api-server
```

**Run with output:**
```bash
cargo test -- --nocapture
```

## 📚 Learn More

- **[README.md](./README.md)** - Full feature documentation
- **[PRODUCTION_DEPLOYMENT.md](./PRODUCTION_DEPLOYMENT.md)** - Production deployment guide
- **[WEEK1_CHANGES.md](./WEEK1_CHANGES.md)** - Recent security improvements
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System architecture details

## 🆘 Troubleshooting

### "Authentication required" error
Make sure you're including the `X-API-Key` header with a valid API key from your `.env` file.

### "Rate limit exceeded" (HTTP 429)
You've exceeded the rate limit. Wait 60 seconds or adjust `RATE_LIMIT_PER_MINUTE` in `.env`.

### CORS errors in browser
Add your frontend domain to `ALLOWED_ORIGINS` in `.env`.

### Redis connection failed
The system will fall back to in-memory cache. This is fine for development but consider running Redis for production.

### Docker build fails
Make sure Docker has enough memory (at least 4GB recommended for Rust builds).

## 🎯 Next Steps

1. **Try the API** - Use curl or Postman to explore endpoints
2. **Build a frontend** - Integrate with React, Vue, or any frontend framework
3. **Deploy to production** - See [PRODUCTION_DEPLOYMENT.md](./PRODUCTION_DEPLOYMENT.md)
4. **Contribute** - Check open issues and contribute improvements

## 💡 Example Use Cases

**Stock Analysis Dashboard:**
```javascript
// Frontend example (JavaScript)
const response = await fetch('http://localhost:3000/api/analyze/AAPL', {
  headers: { 'X-API-Key': 'your_api_key' }
});
const data = await response.json();
console.log(data.data.overall_signal); // "Buy", "Sell", etc.
```

**Automated Trading Bot:**
```python
# Python example
import requests

def get_analysis(symbol):
    response = requests.get(
        f'http://localhost:3000/api/analyze/{symbol}',
        headers={'X-API-Key': 'your_api_key'}
    )
    return response.json()

analysis = get_analysis('AAPL')
if analysis['data']['overall_signal'] == 'Buy':
    print(f"Strong buy signal with {analysis['data']['overall_confidence']*100}% confidence")
```

**Stock Screener:**
```bash
# Get top tech stocks to buy
curl -H "X-API-Key: YOUR_KEY" \
     "http://localhost:3000/api/suggest?universe=tech&min_signal=1&limit=5"
```

Happy analyzing! 🚀📈
