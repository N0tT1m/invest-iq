# Docker Secrets Directory

This directory contains secret files for Docker Compose secrets support.

## Usage

Create the following files (one value per file, no trailing newlines):

```bash
# Example: Create secret files
echo -n "your_polygon_api_key_here" > polygon_api_key.txt
echo -n "your_alpaca_api_key_here" > alpaca_api_key.txt
echo -n "your_alpaca_secret_key_here" > alpaca_secret_key.txt
echo -n "key1,key2,key3" > api_keys.txt
```

## Required Files

- `polygon_api_key.txt` - Polygon.io API key
- `alpaca_api_key.txt` - Alpaca API key
- `alpaca_secret_key.txt` - Alpaca secret key
- `api_keys.txt` - Comma-separated list of valid API keys for InvestIQ

## Security Notes

1. **Never commit these files to git** - They are excluded via `.gitignore`
2. **Set proper file permissions** - Run `chmod 600 *.txt` to restrict access
3. **Use environment variables in development** - Secrets are optional; env vars take precedence
4. **Rotate secrets regularly** - See `docs/secrets-rotation.md` for rotation procedures

## Fallback Behavior

The API server checks secrets in this order:
1. Environment variable (e.g., `POLYGON_API_KEY`)
2. Docker secret file (e.g., `/run/secrets/polygon_api_key`)

If neither is found, the server will exit with an error for required secrets.
