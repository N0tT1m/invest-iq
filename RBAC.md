# Role-Based Access Control (RBAC)

InvestIQ now implements a role-based access control system with three hierarchical roles.

## Roles

### 1. Viewer (Level 0)
- **Access**: All GET/read endpoints
- **Use case**: Read-only access to data, analyses, and reports
- **Cannot**: Execute trades, modify risk parameters, change settings

### 2. Trader (Level 1)
- **Access**: Everything Viewer can + trade execution and portfolio management
- **Can**:
  - Execute trades via broker write endpoints
  - Add/remove watchlist items
  - Run backtests
  - Manage paper trading portfolio
- **Cannot**: Modify system-wide risk parameters or halt trading

### 3. Admin (Level 2)
- **Access**: Full system access
- **Can**:
  - Everything Trader can
  - Update risk parameters (PUT `/api/risk/parameters`)
  - Manually halt/resume trading (POST `/api/risk/trading-halt`)
  - Modify system configuration

## API Key Format

Set the `API_KEYS` environment variable with comma-separated keys and optional role suffixes:

```bash
API_KEYS=key1:admin,key2:trader,key3:viewer,key4
```

- `key1:admin` - Admin role
- `key2:trader` - Trader role
- `key3:viewer` - Viewer role
- `key4` - No role specified, defaults to Admin (backwards compatible)

## Authentication

Include your API key in requests via:
- `X-API-Key` header (recommended): `X-API-Key: your-api-key`
- `Authorization` header: `Authorization: Bearer your-api-key`

## Examples

### Viewer API Key
```bash
# Can read data
curl -H "X-API-Key: viewer-key" http://localhost:3000/api/analyze/AAPL

# Cannot execute trades (403 Forbidden)
curl -H "X-API-Key: viewer-key" \
     -X POST \
     -H "Content-Type: application/json" \
     -d '{"symbol":"AAPL","side":"buy","quantity":10}' \
     http://localhost:3000/api/broker/execute
# Error: "Insufficient permissions. Required role: trader"
```

### Trader API Key
```bash
# Can execute trades
curl -H "X-API-Key: trader-key" \
     -X POST \
     -H "Content-Type: application/json" \
     -d '{"symbol":"AAPL","side":"buy","quantity":10}' \
     http://localhost:3000/api/broker/execute

# Cannot modify risk parameters (403 Forbidden)
curl -H "X-API-Key: trader-key" \
     -X PUT \
     -H "Content-Type: application/json" \
     -d '{"max_position_size_percent":0.15}' \
     http://localhost:3000/api/risk/parameters
# Error: "Insufficient permissions. Required role: admin"
```

### Admin API Key
```bash
# Can modify risk parameters
curl -H "X-API-Key: admin-key" \
     -X PUT \
     -H "Content-Type: application/json" \
     -d '{"max_position_size_percent":0.15}' \
     http://localhost:3000/api/risk/parameters

# Can halt trading
curl -H "X-API-Key: admin-key" \
     -X POST \
     -H "Content-Type: application/json" \
     -d '{"halted":true,"reason":"Market volatility"}' \
     http://localhost:3000/api/risk/trading-halt
```

## Development Mode

If `API_KEYS` is not set, authentication is **disabled** and all endpoints are accessible without an API key. This is intended for local development only.

## Implementation Details

### Protected Endpoints

**Trader-level protection:**
- All broker write routes (POST, DELETE): `/api/broker/execute`, `/api/broker/close`, `/api/broker/cancel`

**Admin-level protection:**
- PUT `/api/risk/parameters` - Update risk parameters
- POST `/api/risk/trading-halt` - Manually halt/resume trading

### Live Trading Protection

Broker write endpoints have dual protection:
1. **RBAC**: Requires Trader role or higher
2. **Live Trading Key**: Requires `X-Live-Trading-Key` header matching `LIVE_TRADING_KEY` env var (only when using live Alpaca API)

Both checks must pass for live trading operations.

## Migration

Existing deployments with `API_KEYS=key1,key2,key3` continue to work unchanged. All keys without role suffixes default to Admin role for backwards compatibility.

To implement RBAC:
1. Update your `API_KEYS` with role suffixes
2. Distribute keys according to user roles
3. Test with lower-privilege keys first
4. Audit access patterns before switching to production
