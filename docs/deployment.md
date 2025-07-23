# ICM Server Deployment Guide

Comprehensive guide for deploying the ICM trading system in various environments.

## 🚀 Quick Start

### Prerequisites

- **Rust**: Version 1.70 or higher
- **PostgreSQL**: Version 13 or higher
- **Solana CLI**: For wallet management
- **Git**: For cloning the repository

### Environment Setup

1. **Clone the repository:**

```bash
git clone <repository-url>
cd icm-server
```

2. **Install Rust dependencies:**

```bash
cargo build --release
```

3. **Set up PostgreSQL:**

```bash
# Create database
createdb icm_trading

# Run migrations
cargo run --bin migrations
```

4. **Configure environment:**

```bash
cp .env.example .env
# Edit .env with your configuration
```

---

## 🛠️ Development Environment

### Local Setup

1. **Environment Variables (.env):**

```env
# Database
DATABASE_URL=postgresql://username:password@localhost/icm_trading

# Solana
SOLANA_RPC_URL=https://api.devnet.solana.com
PRIVATE_KEY_PATH=./wallet.json

# API Keys
OPENAI_API_KEY=sk-proj-your-key-here
JUPITER_API_BASE_URL=https://quote-api.jup.ag/v6

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=3000
LOG_LEVEL=info

# ICM Program
ICM_PROGRAM_ID=your-program-id-here
```

2. **Create Solana Wallet:**

```bash
# Generate new wallet
solana-keygen new --outfile wallet.json

# Get public key
solana-keygen pubkey wallet.json

# Request airdrop (devnet)
solana airdrop 2 <your-public-key> --url devnet
```

3. **Start Development Server:**

```bash
cargo run
```

4. **Verify Setup:**

```bash
curl http://localhost:3000/ping
# Should return: {"status":"pong"}
```

### Docker Development

1. **Create docker-compose.yml:**

```yaml
version: "3.8"

services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: icm_trading
      POSTGRES_USER: icm_user
      POSTGRES_PASSWORD: icm_password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data

  icm-server:
    build: .
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgresql://icm_user:icm_password@postgres:5432/icm_trading
      SOLANA_RPC_URL: https://api.devnet.solana.com
    volumes:
      - ./wallet.json:/app/wallet.json
      - ./.env:/app/.env
    depends_on:
      - postgres

volumes:
  postgres_data:
```

2. **Create Dockerfile:**

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/icm-server ./
COPY --from=builder /app/migrations ./migrations

EXPOSE 3000
CMD ["./icm-server"]
```

3. **Start with Docker:**

```bash
docker-compose up -d
```

---

## ☁️ Production Deployment

### VPS/Cloud Server

#### 1. Server Requirements

| Component | Minimum  | Recommended |
| --------- | -------- | ----------- |
| CPU       | 2 cores  | 4+ cores    |
| RAM       | 4GB      | 8GB+        |
| Storage   | 50GB SSD | 100GB+ SSD  |
| Network   | 100 Mbps | 1 Gbps      |

#### 2. System Setup (Ubuntu 22.04)

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y curl build-essential pkg-config libssl-dev libpq-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.bashrc

# Install PostgreSQL
sudo apt install -y postgresql postgresql-contrib
sudo systemctl start postgresql
sudo systemctl enable postgresql

# Create database user
sudo -u postgres createuser --interactive icm_user
sudo -u postgres createdb icm_trading
sudo -u postgres psql -c "ALTER USER icm_user PASSWORD 'secure_password';"
```

#### 3. Application Deployment

```bash
# Clone and build
git clone <repository-url> /opt/icm-server
cd /opt/icm-server
cargo build --release

# Create service user
sudo useradd --system --shell /bin/false icm-server

# Set permissions
sudo chown -R icm-server:icm-server /opt/icm-server
sudo chmod +x /opt/icm-server/target/release/icm-server

# Create systemd service
sudo tee /etc/systemd/system/icm-server.service > /dev/null <<EOF
[Unit]
Description=ICM Trading Server
After=network.target postgresql.service

[Service]
Type=simple
User=icm-server
Group=icm-server
WorkingDirectory=/opt/icm-server
ExecStart=/opt/icm-server/target/release/icm-server
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

# Start service
sudo systemctl daemon-reload
sudo systemctl enable icm-server
sudo systemctl start icm-server
```

#### 4. Reverse Proxy (Nginx)

```bash
# Install Nginx
sudo apt install -y nginx

# Create configuration
sudo tee /etc/nginx/sites-available/icm-server > /dev/null <<EOF
server {
    listen 80;
    server_name your-domain.com;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_cache_bypass \$http_upgrade;
    }
}
EOF

# Enable site
sudo ln -s /etc/nginx/sites-available/icm-server /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

#### 5. SSL Certificate

```bash
# Install Certbot
sudo apt install -y certbot python3-certbot-nginx

# Get certificate
sudo certbot --nginx -d your-domain.com

# Auto-renewal
sudo systemctl enable certbot.timer
```

### AWS Deployment

#### 1. EC2 Instance

1. **Launch Instance:**

   - AMI: Ubuntu Server 22.04 LTS
   - Instance Type: t3.medium (minimum)
   - Security Group: HTTP (80), HTTPS (443), Custom TCP (3000)

2. **Setup Script:**

```bash
#!/bin/bash
# User data script for EC2 instance

# Update system
apt update && apt upgrade -y

# Install dependencies
apt install -y curl build-essential pkg-config libssl-dev libpq-dev nginx

# Install Rust
su - ubuntu -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'

# Install PostgreSQL
apt install -y postgresql postgresql-contrib
systemctl start postgresql
systemctl enable postgresql

# Clone and deploy application
cd /opt
git clone <repository-url> icm-server
chown -R ubuntu:ubuntu icm-server

# Build application
su - ubuntu -c 'cd /opt/icm-server && ~/.cargo/bin/cargo build --release'

# Setup systemd service (use previous configuration)
```

#### 2. RDS Database

1. **Create RDS Instance:**

   - Engine: PostgreSQL 15
   - Instance Class: db.t3.micro (dev) / db.t3.small (prod)
   - Storage: 20GB GP2 (minimum)

2. **Update Environment:**

```env
DATABASE_URL=postgresql://username:password@rds-endpoint:5432/icm_trading
```

#### 3. Application Load Balancer

```bash
# Create target group
aws elbv2 create-target-group \
    --name icm-server-targets \
    --protocol HTTP \
    --port 3000 \
    --vpc-id vpc-xxxxxxxxx

# Create load balancer
aws elbv2 create-load-balancer \
    --name icm-server-alb \
    --subnets subnet-xxxxxxxx subnet-yyyyyyyy
```

### Google Cloud Platform

#### 1. Compute Engine

```bash
# Create VM
gcloud compute instances create icm-server \
    --image-family=ubuntu-2204-lts \
    --image-project=ubuntu-os-cloud \
    --machine-type=e2-medium \
    --boot-disk-size=50GB \
    --boot-disk-type=pd-ssd
```

#### 2. Cloud SQL

```bash
# Create PostgreSQL instance
gcloud sql instances create icm-postgres \
    --database-version=POSTGRES_15 \
    --tier=db-f1-micro \
    --region=us-central1
```

### DigitalOcean Deployment

#### 1. Droplet Creation

```bash
# Using doctl CLI
doctl compute droplet create icm-server \
    --image ubuntu-22-04-x64 \
    --size s-2vcpu-4gb \
    --region nyc3 \
    --ssh-keys <ssh-key-id>
```

#### 2. App Platform

```yaml
# .do/app.yaml
name: icm-server
services:
  - name: api
    source_dir: /
    github:
      repo: your-username/icm-server
      branch: main
    run_command: ./target/release/icm-server
    environment_slug: rust
    instance_count: 1
    instance_size_slug: basic-xxs
    http_port: 3000
    envs:
      - key: DATABASE_URL
        scope: RUN_TIME
        value: ${db.DATABASE_URL}
      - key: SOLANA_RPC_URL
        scope: RUN_TIME
        value: https://api.devnet.solana.com

databases:
  - name: db
    engine: PG
    version: "15"
    size: basic-xs
```

---

## 📊 Monitoring & Observability

### Health Checks

```bash
# Create health check script
sudo tee /opt/icm-server/health-check.sh > /dev/null <<EOF
#!/bin/bash
response=\$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/ping)
if [ \$response -eq 200 ]; then
    exit 0
else
    exit 1
fi
EOF

sudo chmod +x /opt/icm-server/health-check.sh
```

### Log Management

```bash
# Configure logrotate
sudo tee /etc/logrotate.d/icm-server > /dev/null <<EOF
/var/log/icm-server/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 644 icm-server icm-server
    postrotate
        systemctl reload icm-server
    endscript
}
EOF
```

### Process Monitoring

```bash
# Install htop and monitoring tools
sudo apt install -y htop iotop nethogs

# Monitor ICM server process
htop -p $(pgrep -f icm-server)

# Check resource usage
sudo netstat -tlnp | grep :3000
sudo ss -tlnp | grep :3000
```

### Database Monitoring

```sql
-- PostgreSQL monitoring queries

-- Check connection count
SELECT count(*) FROM pg_stat_activity WHERE datname = 'icm_trading';

-- Check database size
SELECT pg_size_pretty(pg_database_size('icm_trading'));

-- Monitor query performance
SELECT query, mean_exec_time, calls
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;
```

---

## 🔒 Security Hardening

### Firewall Configuration

```bash
# Install and configure UFW
sudo ufw enable
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow from <your-ip> to any port 3000

# Check status
sudo ufw status verbose
```

### SSL/TLS Configuration

```nginx
# Enhanced Nginx SSL configuration
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;

    add_header Strict-Transport-Security "max-age=63072000" always;
    add_header X-Content-Type-Options nosniff;
    add_header X-Frame-Options DENY;
    add_header X-XSS-Protection "1; mode=block";

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Environment Security

```bash
# Secure .env file
sudo chmod 600 /opt/icm-server/.env
sudo chown icm-server:icm-server /opt/icm-server/.env

# Secure wallet file
sudo chmod 600 /opt/icm-server/wallet.json
sudo chown icm-server:icm-server /opt/icm-server/wallet.json

# Remove sensitive data from logs
export RUST_LOG=icm_server=info,warn,error
```

---

## 🔄 Backup & Recovery

### Database Backup

```bash
# Create backup script
sudo tee /opt/icm-server/backup.sh > /dev/null <<EOF
#!/bin/bash
BACKUP_DIR="/opt/backups/icm-server"
DATE=\$(date +%Y%m%d_%H%M%S)

mkdir -p \$BACKUP_DIR

# Database backup
pg_dump icm_trading > \$BACKUP_DIR/database_\$DATE.sql

# Application backup
tar -czf \$BACKUP_DIR/app_\$DATE.tar.gz /opt/icm-server

# Keep only last 7 days
find \$BACKUP_DIR -name "*.sql" -mtime +7 -delete
find \$BACKUP_DIR -name "*.tar.gz" -mtime +7 -delete
EOF

sudo chmod +x /opt/icm-server/backup.sh

# Add to crontab
echo "0 2 * * * /opt/icm-server/backup.sh" | sudo crontab -
```

### Recovery Procedure

```bash
# Stop service
sudo systemctl stop icm-server

# Restore database
sudo -u postgres dropdb icm_trading
sudo -u postgres createdb icm_trading
psql -U icm_user -d icm_trading < /opt/backups/icm-server/database_YYYYMMDD_HHMMSS.sql

# Restore application
cd /opt
sudo tar -xzf /opt/backups/icm-server/app_YYYYMMDD_HHMMSS.tar.gz

# Start service
sudo systemctl start icm-server
```

---

## 📈 Performance Optimization

### Rust Optimization

```toml
# Cargo.toml - Release optimizations
[profile.release]
codegen-units = 1
lto = true
opt-level = 3
panic = "abort"
```

### Database Optimization

```sql
-- PostgreSQL optimization
-- postgresql.conf settings
shared_buffers = 256MB
effective_cache_size = 1GB
random_page_cost = 1.1
checkpoint_completion_target = 0.9
wal_buffers = 16MB
default_statistics_target = 100
```

### System Optimization

```bash
# System limits
echo "icm-server soft nofile 65536" | sudo tee -a /etc/security/limits.conf
echo "icm-server hard nofile 65536" | sudo tee -a /etc/security/limits.conf

# Kernel parameters
echo "net.core.somaxconn = 65535" | sudo tee -a /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog = 65535" | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

This deployment guide covers everything from local development to production deployment across multiple cloud providers! 🚀
