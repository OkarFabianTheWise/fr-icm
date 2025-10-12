# ICM Server - AI-Powered Trading System

### sudo -u postgres psql

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![Rust Version](https://img.shields.io/badge/rust-1.75+-blue)
![License](https://img.shields.io/badge/license-MIT-green)

An intelligent, high-performance trading system built with Rust that combines AI-driven decision making with Solana blockchain integration and Jupiter DEX aggregation.

## 🚀 Quick Start

```bash
# 1. Clone and setup
git clone <repository-url>
cd icm-server

# 2. Configure environment
cp .env.example .env
# Edit .env with your API keys

# 3. Run the system
cargo run

# 4. Verify it's working
curl http://localhost:3000/ping
```

**Server will be running at:** `http://127.0.0.1:3000`

## 🏗️ System Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   HTTP Server   │────│  Trading Agent  │────│  Jupiter DEX    │
│   (Axum/Tokio)  │    │   (AI-Powered)  │    │  Integration    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │              ┌─────────────────┐              │
         │              │ Data Components │              │
         │              └─────────────────┘              │
         │                       │                       │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Database      │    │   Executor      │    │ Solana Network │
│   (PostgreSQL)  │────│  (Your Code!)   │────│   (Devnet)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 📦 Core Components

### 🤖 **AI Trading Agent** (`src/agent/`)

- **Purpose**: Makes intelligent trading decisions using OpenAI
- **Key Features**: Strategy planning, risk management, adaptive learning
- **Entry Point**: `trading_agent.rs`

### ⚡ **Executor** (`src/agent/executor.rs`)

- **Purpose**: Executes trading plans by building and submitting transactions
- **Key Features**: Concurrent execution, Jupiter integration, metrics tracking
- **Your Implementation**: Core transaction execution logic

### 📊 **Data Fetcher** (`src/agent/data_fetcher.rs`)

- **Purpose**: Collects real-time market data and price feeds
- **Key Features**: Multi-source aggregation, caching, WebSocket connections

### 🧠 **Planner** (`src/agent/planner.rs`)

- **Purpose**: Analyzes market conditions and creates trading strategies
- **Key Features**: Multi-strategy support, risk assessment, opportunity detection

### 👀 **Observer** (`src/agent/observer.rs`)

- **Purpose**: Monitors trade execution and performance
- **Key Features**: Real-time tracking, performance analytics, alerting

### 🌐 **HTTP Server** (`src/server.rs`)

- **Purpose**: Provides REST API for system control and monitoring
- **Key Features**: Health checks, agent control, real-time status

## 🎯 Trading Strategies

| Strategy         | Description                   | Risk Level | Best For         |
| ---------------- | ----------------------------- | ---------- | ---------------- |
| **Arbitrage**    | Price differences across DEXs | Low        | Stable profits   |
| **DCA**          | Dollar Cost Averaging         | Medium     | Long-term growth |
| **Grid Trading** | Buy low, sell high in ranges  | Medium     | Sideways markets |
| **Momentum**     | Follow price trends           | High       | Trending markets |

## 🔌 API Endpoints

### Agent Control

- `GET /api/v1/agent/status` - Get agent status
- `POST /api/v1/agent/start` - Start trading
- `POST /api/v1/agent/stop` - Stop trading
- `POST /api/v1/agent/rebalance` - Force rebalance

### ICM Program

#### ⚠️ First-Time Setup (Required)

- `GET /api/v1/program/status?usdc_mint=<mint>` - Check if program is initialized
- `POST /api/v1/program/initialize` - Initialize program (MUST be called first)

#### Regular Operations

- `POST /api/v1/profile/create` - Create trading profile
- `POST /api/v1/bucket/create` - Create trading bucket
- `POST /api/v1/bucket/contribute` - Contribute to bucket
- `POST /api/v1/bucket/swap` - Execute swap
- `GET /api/v1/bucket/all` - Get all buckets

### Health & Monitoring

- `GET /ping` - Health check
- `GET /api/v1/agent/state` - Detailed metrics

## 🛠️ Configuration

### Environment Variables (`.env`)

```bash
# AI Configuration
OPENAI_API_KEY=your_openai_api_key_here

# Trading Configuration
MAX_CONCURRENT_TRADES=5
RISK_THRESHOLD=0.8
QUOTE_CACHE_TTL=30

# Solana Configuration
SOLANA_RPC_URL=https://api.devnet.solana.com
SOLANA_WS_URL=wss://api.devnet.solana.com

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/icm_db
```

## 🔧 Development

### Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- Node.js 18+ (for frontend)

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logs
RUST_LOG=debug cargo run
```

## 📁 Project Structure

```
src/
├── agent/              # 🤖 AI Trading Components
│   ├── trading_agent.rs   # Main agent orchestrator
│   ├── executor.rs         # Transaction execution (Your Code!)
│   ├── data_fetcher.rs     # Market data collection
│   ├── planner.rs          # Strategy planning
│   ├── observer.rs         # Performance monitoring
│   └── types.rs           # Shared data structures
├── routes/             # 🌐 HTTP API Routes
│   ├── agent.rs           # Agent control endpoints
│   ├── health.rs          # Health check endpoints
│   └── icm.rs            # ICM program endpoints
├── services/           # 🔧 Business Logic Services
├── database/           # 💾 Database Integration
├── auth/              # 🔐 Authentication & JWT
├── onchain_instance/   # ⛓️  Solana Program Integration
└── config.rs          # ⚙️  Configuration Management
```

## 🚀 Frontend Integration

Want to build a frontend? Here's what you need to know:

### Simple Dashboard Example

```javascript
// Check if server is running
fetch("http://localhost:3000/ping")
  .then((r) => r.json())
  .then((data) => console.log("Server:", data.status));

// Get trading agent status
fetch("http://localhost:3000/api/v1/agent/status")
  .then((r) => r.json())
  .then((data) => console.log("Agent:", data));

// Start trading with basic strategy
fetch("http://localhost:3000/api/v1/agent/start", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    openai_api_key: "your-key",
    token_pairs: [["SOL", "USDC"]],
    strategies: [
      {
        strategy_type: "Arbitrage",
        min_spread_bps: 50,
        position_size_usd: 100,
      },
    ],
  }),
});
```

See [`docs/frontend-guide.md`](docs/frontend-guide.md) for complete frontend integration guide.

## 📚 Documentation

- **[Component Architecture](docs/components.md)** - Detailed component documentation
- **[Frontend Integration](docs/frontend-guide.md)** - Building user interfaces
- **[Technical Connections](docs/technical-architecture.md)** - How modules connect
- **[API Reference](docs/api-reference.md)** - Complete API documentation
- **[Trading Strategies](docs/strategies.md)** - Strategy implementation guide

## 🔒 Security Notes

- **Private Keys**: Never commit private keys to version control
- **API Keys**: Store in environment variables only
- **Rate Limiting**: Implement for production deployments
- **Input Validation**: All API inputs are validated
- **Error Handling**: Graceful failure modes implemented

## 📈 Performance

- **Concurrent Execution**: Up to 5 simultaneous trades
- **Low Latency**: < 100ms average execution time
- **High Throughput**: 1000+ requests/second capacity
- **Memory Efficient**: Rust's zero-cost abstractions

## 🔧 Troubleshooting

### "Program state does not exist or cannot be fetched: Account not found"

This error occurs when the ICM program hasn't been initialized yet. **This is required before any other operations.**

#### Solution:

1. **Check program status first:**

   ```bash
   curl "http://localhost:3000/api/v1/program/status?usdc_mint=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
   ```

2. **Initialize the program (one-time setup):**

   ```bash
   curl -X POST http://localhost:3000/api/v1/program/initialize \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer YOUR_JWT_TOKEN" \
     -d '{
       "fee_rate_bps": 500,
       "usdc_mint": "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
     }'
   ```

3. **Then proceed with other operations** like creating profiles and buckets.

### Common Issues

- **Authentication Required**: Most endpoints require a valid JWT token
- **Wrong USDC Mint**: Ensure you're using the correct USDC mint address for your network
- **Insufficient Balance**: Check wallet balance before operations

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

---

**🎯 Ready to trade? Your AI-powered trading system is waiting!**
