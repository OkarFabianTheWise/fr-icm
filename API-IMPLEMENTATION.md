# ICM Trading Platform API Documentation

## Overview

The ICM (Intelligent Content Management) Trading Platform API provides comprehensive endpoints for managing decentralized investment pools, AI-powered trading agents, authentication, and Solana blockchain interactions. Built with Rust, Axum, and integrated with Jupiter DEX for Solana token swaps.

**Base URL**: `https://your-api-domain.com`  
**Environment**: Solana Devnet  
**Authentication**: JWT-based with HttpOnly cookies

---

## 🔐 Authentication

All protected endpoints require authentication via JWT tokens stored in HttpOnly cookies.

### Cookie Configuration

- **Name**: `access_token`
- **HttpOnly**: `true`
- **Secure**: `true` (HTTPS only)
- **SameSite**: `None` (for cross-origin support)
- **Path**: `/`

### Register User

```http
POST /api/auth/register
```

**Request Body:**

```json
{
  "email": "user@example.com",
  "password": "securePassword123"
}
```

**Response:**

```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com"
  },
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "expires_at": 1672531200
}
```

**Status Codes:**

- `200` - Success
- `409` - Email already registered
- `500` - Server error

### Login User

```http
POST /api/auth/login
```

**Request Body:**

```json
{
  "email": "user@example.com",
  "password": "securePassword123"
}
```

**Response:**

```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com"
  },
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "wallet_balances": {
    "sol_balance": 1.5,
    "usdc_balance": 250.0
  },
  "expires_at": 1672531200,
  "pools": [...]
}
```

### Get Current User

```http
GET /api/auth/me
```

**Headers:**

```
Cookie: access_token=your_jwt_token
```

**Response:**

```json
{
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com"
  },
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "wallet_balances": {
    "sol_balance": 1.5,
    "usdc_balance": 250.0
  },
  "pools": [...]
}
```

### Logout

```http
POST /api/auth/logout
```

**Response:** `204 No Content`

---

## 🏥 Health Check

### Ping Server

```http
GET /ping
```

**Response:**

```json
{
  "status": "ok",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## 🏦 ICM Program Operations

All ICM endpoints require authentication and return unsigned transactions for client-side signing.

### Check Program Status

```http
GET /api/v1/program/status?usdc_mint=2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg
```

**Response:**

```json
{
  "success": true,
  "data": {
    "is_initialized": true,
    "program_id": "ICMProgram111111111111111111111111111111111"
  },
  "error": null
}
```

### Initialize Program

```http
POST /api/v1/program/initialize
```

**Request Body:**

```json
{
  "usdc_mint": "2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg"
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "transaction": "base64_encoded_transaction",
    "message": "Program initialization transaction created"
  },
  "error": null
}
```

### Create User Profile

```http
POST /api/v1/profile/create
```

**Response:**

```json
{
  "success": true,
  "data": {
    "transaction": "base64_encoded_transaction",
    "message": "Profile creation transaction created"
  },
  "error": null
}
```

---

## 🪣 Investment Pool (Bucket) Management

### Create Investment Pool

```http
POST /api/v1/bucket/create
```

**Request Body:**

```json
{
  "name": "AI Growth Fund",
  "token_mints": [
    "So11111111111111111111111111111111111111112",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  ],
  "contribution_window_minutes": 10080,
  "trading_window_minutes": 43200,
  "creator_fee_percent": 200,
  "target_amount": 10000.0,
  "min_contribution": 10.0,
  "max_contribution": 1000.0,
  "management_fee": 100,
  "strategy": "Arbitrage"
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "transaction": "base64_encoded_transaction",
    "message": "Bucket creation transaction created"
  },
  "error": null
}
```

### Contribute to Pool

```http
POST /api/v1/bucket/contribute
```

**Request Body:**

```json
{
  "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "bucket_name": "AI Growth Fund",
  "contribution_amount": 100.0
}
```

### Start Trading

```http
POST /api/v1/bucket/start-trading
```

**Request Body:**

```json
{
  "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "bucket_name": "AI Growth Fund",
  "token_bucket": [
    "So11111111111111111111111111111111111111112",
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  ],
  "trading_end_time": "2024-01-15T10:30:00Z",
  "strategy": "Arbitrage",
  "management_fee": 100
}
```

### Get All Pools

```http
GET /api/v1/bucket/all
```

**Response:**

```json
{
  "success": true,
  "data": [
    {
      "public_key": "PoolPubkey111111111111111111111111111111111",
      "account": {
        "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "name": "AI Growth Fund",
        "token_mints": ["So11111111111111111111111111111111111111112"],
        "contribution_deadline": "1672531200",
        "trading_deadline": "1675209600",
        "creator_fee_percent": 200,
        "status": "fundraising",
        "raised_amount": 2500.0,
        "contributor_count": 25,
        "strategy": "Arbitrage",
        "time_remaining": "2d 5h 30m"
      }
    }
  ],
  "error": null
}
```

### Get Trading Pool Info

```http
POST /api/v1/bucket/trading_pools
```

**Query Parameters:**

```
?creator_pubkey=9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM&bucket_name=AI%20Growth%20Fund
```

### Swap Tokens

```http
POST /api/v1/bucket/swap
```

**Request Body:**

```json
{
  "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "bucket_name": "AI Growth Fund",
  "input_mint": "So11111111111111111111111111111111111111112",
  "output_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "amount": 1.0,
  "slippage_bps": 50
}
```

### Claim Rewards

```http
POST /api/v1/bucket/claim-rewards
```

**Request Body:**

```json
{
  "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "bucket_name": "AI Growth Fund"
}
```

### Close Pool

```http
POST /api/v1/bucket/close
```

**Request Body:**

```json
{
  "creator": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "bucket_name": "AI Growth Fund"
}
```

---

## 🤖 AI Trading Agent

### Get Agent Status

```http
GET /api/v1/agent/status
```

**Response:**

```json
{
  "status": "active",
  "is_running": true,
  "stats": {
    "total_trades": 125,
    "successful_trades": 98,
    "total_profit_usd": 2450.75,
    "current_positions": 3,
    "uptime_seconds": 86400,
    "is_running": true
  },
  "message": "Trading agent is operational"
}
```

### Start Trading Agent

```http
POST /api/v1/agent/start
```

**Request Body:**

```json
{
  "openai_api_key": "sk-your-openai-api-key",
  "token_pairs": [
    [
      "So11111111111111111111111111111111111111112",
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    ]
  ],
  "strategies": [
    {
      "strategy_type": "Arbitrage",
      "min_spread_bps": 10,
      "max_slippage_bps": 50,
      "position_size_usd": 100.0,
      "max_position_size_usd": 1000.0,
      "priority_fee_percentile": 75,
      "max_priority_fee_lamports": 100000
    }
  ],
  "data_fetch_interval_ms": 5000,
  "learning_enabled": true,
  "portfolio_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

### Stop Trading Agent

```http
POST /api/v1/agent/stop
```

### Get Agent State

```http
GET /api/v1/agent/state
```

**Response:**

```json
{
  "agent_state": "Trading",
  "current_strategy": "Arbitrage",
  "active_positions": [
    {
      "token_pair": "SOL/USDC",
      "position_size_usd": 150.0,
      "entry_price": 98.5,
      "current_pnl": 12.75
    }
  ],
  "portfolio_value_usd": 5250.0
}
```

### Update Strategy

```http
POST /api/v1/agent/strategy
```

**Request Body:**

```json
{
  "strategy_config": {
    "strategy_type": "GridTrading",
    "min_spread_bps": 15,
    "max_slippage_bps": 30,
    "position_size_usd": 200.0,
    "max_position_size_usd": 2000.0
  }
}
```

### Force Rebalance

```http
POST /api/v1/agent/rebalance
```

### Emergency Stop

```http
POST /api/v1/agent/emergency-stop
```

---

## 💰 Wallet Operations

### Get Wallet Balance

```http
GET /api/wallet/balance?public_key=9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
```

**Response:**

```json
{
  "sol_balance": 1.5,
  "usdc_balance": 250.0,
  "public_key": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
}
```

---

## 🚿 Faucet

### Claim Test Tokens

```http
POST /api/v1/faucet/claim
```

**Request Body:**

```json
{
  "amount": 100.0
}
```

**Response:**

```json
{
  "success": true,
  "message": "Successfully claimed 100 USDC from faucet",
  "tx_signature": "3Bxs7uxE2KgooRX..."
}
```

**Rate Limits:**

- Max amount: 100 USDC per claim
- Cooldown: 3 hours between claims

---

## 📊 Data Models

### TradingPool

```json
{
  "pool_id": "string",
  "pool_bump": 8,
  "creator": "string (base58 pubkey)",
  "token_bucket": ["string"],
  "target_amount": "string (USDC amount)",
  "min_contribution": "string",
  "max_contribution": "string",
  "trading_duration": "string",
  "created_at": "string (ISO timestamp)",
  "fundraising_deadline": "string (ISO timestamp)",
  "trading_start_time": "string (ISO timestamp) | null",
  "trading_end_time": "string (ISO timestamp) | null",
  "phase": "fundraising | trading | completed | closed",
  "management_fee": 16,
  "raised_amount": "string | null",
  "contribution_percent": "number | null",
  "strategy": "string | null",
  "time_remaining": "string | null"
}
```

### AgentStats

```json
{
  "total_trades": "number",
  "successful_trades": "number",
  "total_profit_usd": "number",
  "current_positions": "number",
  "uptime_seconds": "number",
  "is_running": "boolean"
}
```

### WalletBalance

```json
{
  "sol_balance": "number (SOL units)",
  "usdc_balance": "number (USDC units)",
  "public_key": "string (base58)"
}
```

---

## 🔒 Authentication Flow

### Frontend Implementation

```javascript
// Login
const response = await fetch("/api/auth/login", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  credentials: "include", // Important for cookies
  body: JSON.stringify({ email, password }),
});

// Authenticated requests
const poolsResponse = await fetch("/api/v1/bucket/all", {
  method: "GET",
  credentials: "include", // Include cookies
});
```

### Cookie Management

- Cookies are automatically managed by the browser
- No manual token storage required
- Cross-origin requests supported with `credentials: 'include'`

---

## ⚡ Real-time Features

### WebSocket Endpoints (Future)

- `/ws/agent-updates` - Live trading agent status
- `/ws/pool-updates` - Pool status changes
- `/ws/price-feeds` - Real-time price data

---

## 🚨 Error Handling

### Standard Error Response

```json
{
  "success": false,
  "data": null,
  "error": "Error message description"
}
```

### Common Status Codes

- `200` - Success
- `400` - Bad Request (invalid parameters)
- `401` - Unauthorized (missing/invalid JWT)
- `403` - Forbidden (insufficient permissions)
- `404` - Not Found
- `409` - Conflict (duplicate resource)
- `500` - Internal Server Error

### Rate Limiting

- Faucet: 1 claim per 3 hours
- Trading operations: Based on Solana network limits
- API calls: 1000 requests per minute per IP

---

## 🔧 Environment Configuration

### Required Environment Variables

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost/icm_db

# Authentication
JWT_SECRET=your-secret-key

# Solana
SOLANA_RPC_URL=https://api.devnet.solana.com

# AI Trading
OPENAI_API_KEY=sk-your-openai-key

# Server
PORT=3000
```

### CORS Configuration

```rust
// Allowed origins
"https://fr-icm-ui.vercel.app"
"http://localhost:3001"

// Allowed methods
GET, POST, OPTIONS

// Allowed headers
Origin, Content-Type, Accept, Authorization
```

---

## 📈 Integration Examples

### Complete Pool Creation Flow

```javascript
// 1. Create pool
const createResponse = await fetch("/api/v1/bucket/create", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  credentials: "include",
  body: JSON.stringify({
    name: "DeFi Growth Pool",
    token_mints: ["So11111111111111111111111111111111111111112"],
    contribution_window_minutes: 10080,
    trading_window_minutes: 43200,
    creator_fee_percent: 200,
    target_amount: 5000.0,
    min_contribution: 50.0,
    max_contribution: 500.0,
    management_fee: 100,
    strategy: "Arbitrage",
  }),
});

// 2. Sign and submit transaction
const { transaction } = await createResponse.json();
// ... sign with wallet and submit to Solana network

// 3. Start trading when ready
const startTradingResponse = await fetch("/api/v1/bucket/start-trading", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  credentials: "include",
  body: JSON.stringify({
    creator: userWalletAddress,
    bucket_name: "DeFi Growth Pool",
    token_bucket: ["So11111111111111111111111111111111111111112"],
    trading_end_time: new Date(
      Date.now() + 30 * 24 * 60 * 60 * 1000
    ).toISOString(),
    strategy: "Arbitrage",
    management_fee: 100,
  }),
});
```

### AI Trading Agent Setup

```javascript
// Start AI agent
const agentResponse = await fetch("/api/v1/agent/start", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  credentials: "include",
  body: JSON.stringify({
    openai_api_key: process.env.OPENAI_API_KEY,
    token_pairs: [
      [
        "So11111111111111111111111111111111111111112",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      ],
    ],
    strategies: [
      {
        strategy_type: "Arbitrage",
        min_spread_bps: 10,
        position_size_usd: 100.0,
        max_position_size_usd: 1000.0,
      },
    ],
    portfolio_id: userPortfolioId,
  }),
});

// Monitor agent status
const statusResponse = await fetch("/api/v1/agent/status", {
  credentials: "include",
});
```

---

## 🛡️ Security Considerations

### Authentication Security

- JWT tokens stored in HttpOnly cookies (XSS protection)
- HTTPS required in production
- Cross-site cookie protection with SameSite=None

### Transaction Security

- All transactions returned unsigned for client-side signing
- Private keys never transmitted to backend
- Message signing for wallet verification

### Rate Limiting

- Faucet cooldowns prevent abuse
- API rate limiting per IP address
- Solana network built-in transaction limits

---

## 📚 SDK Usage (Coming Soon)

```javascript
import { ICMClient } from "@icm/sdk";

const client = new ICMClient({
  apiUrl: "https://api.icm-trading.com",
  environment: "devnet",
});

// Authenticate
await client.auth.login(email, password);

// Create pool
const pool = await client.pools.create({
  name: "My Trading Pool",
  strategy: "Arbitrage",
  targetAmount: 10000,
});

// Start AI trading
await client.agent.start({
  strategies: ["Arbitrage"],
  portfolioId: pool.id,
});
```

This API documentation provides complete integration guidance for building DeFi applications on top of the ICM Trading Platform. For additional support or questions, please refer to our technical documentation or contact the development team.
