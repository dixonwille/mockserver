# Deployment Guide

Deploying the mock server in various environments.

## Docker

**Note:** The following Docker files are examples to copy into your project.

### Example Dockerfile

```dockerfile
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/mockserver /mockserver
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
VOLUME ["/mocks", "/data"]
EXPOSE 3000 3001
ENTRYPOINT ["/mockserver", "serve"]
```

### Example docker-compose.yml

```yaml
services:
  mockserver:
    build: .
    ports:
      - "3000:3000"
      - "3001:3001"
    volumes:
      - ./mocks:/mocks
      - mockserver-data:/data
    environment:
      - MOCKSERVER_DIR=/mocks
      - MOCKSERVER_DATA_DIR=/data
      - MOCKSERVER_HOST=0.0.0.0

volumes:
  mockserver-data:
```

### Running

```bash
docker-compose up -d
docker-compose logs -f mockserver
docker-compose down
```

## Single Binary

### Download

```bash
curl -L https://github.com/user/mockserver/releases/latest/download/mockserver-$(uname -s)-$(uname -m) -o mockserver
chmod +x mockserver
./mockserver init
./mockserver serve
```

### Build from Source

```bash
cargo install mockserver
mockserver init
mockserver serve
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MOCKSERVER_PORT` | 3000 | Mock server port |
| `MOCKSERVER_API_PORT` | 3001 | Admin API port |
| `MOCKSERVER_DIR` | `./mocks` | Lua scripts directory |
| `MOCKSERVER_DATA_DIR` | `./data` | SQLite database directory |
| `MOCKSERVER_HOST` | `127.0.0.1` | Bind address |
| `MOCKSERVER_RETENTION` | 7 | Days to keep request history |
| `MOCKSERVER_SCRIPT_TIMEOUT` | 30 | Lua script timeout (seconds) |
| `MOCKSERVER_IDLE_TIMEOUT` | 30 | Flush idle domains (minutes) |
| `MOCKSERVER_LUA_MEMORY` | 64 | Memory limit per Lua state (MB) |
| `RUST_LOG` | `info` | Log level |

## Reverse Proxy (nginx)

```nginx
upstream mockserver {
    server localhost:3000;
}

upstream mockserver_api {
    server localhost:3001;
}

server {
    listen 443 ssl;
    server_name mock.example.com;

    ssl_certificate /etc/nginx/ssl/cert.pem;
    ssl_certificate_key /etc/nginx/ssl/key.pem;

    location / {
        proxy_pass http://mockserver;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Host $host;
    }
}

server {
    listen 443 ssl;
    server_name mock-admin.example.com;

    ssl_certificate /etc/nginx/ssl/cert.pem;
    ssl_certificate_key /etc/nginx/ssl/key.pem;

    location / {
        proxy_pass http://mockserver_api;
        proxy_set_header Host $host;
    }
}
```

## Production Checklist

- [ ] Set `MOCKSERVER_HOST=0.0.0.0` in containers
- [ ] Configure `--retention` for storage needs
- [ ] Set `--max-body` for expected payloads
- [ ] Review `--idle-timeout` for memory vs latency
- [ ] Deploy behind reverse proxy for TLS
- [ ] Restrict network access to dev/test environments

## Related Documentation

- [CLI](./CLI.md) - Command-line options
- [Operations](./OPERATIONS.md) - Monitoring and maintenance
