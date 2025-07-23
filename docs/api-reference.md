# ICM Server API Reference

Complete API documentation for the ICM trading system.

## 🌐 Base URL

```
http://localhost:3000
```

## 📋 Response Format

All API responses follow this format:

```json
{
  "status": "success|error",
  "message": "Human readable message",
  "data": {
    /* Response data */
  },
  "timestamp": "2025-07-23T11:43:54.893Z"
}
```

## 🔍 Health & Monitoring

### `GET /ping`

Check if the server is running and healthy.

**Response:**

```json
{
  "status": "pong"
}
```

**Example:**

```bash
curl http://localhost:3000/ping
```

```javascript
const response = await fetch("http://localhost:3000/ping");
const data = await response.json();
console.log(data.status); // "pong"
```

---

## 🤖 Trading Agent Endpoints

### `GET /api/v1/agent/status`

Get the current status and basic metrics of the trading agent.

**Response:**

```json
{
  "status": "active|inactive",
  "is_running": true,
  "stats": {
    "is_running": true,
    "is_active": true,
    "uptime_seconds": 3600,
    "data_fetcher": {
      "is_running": true,
      "cached_quotes": 10,
      "cached_prices": 5,
      "configured_pairs": 1,
      "fetch_interval_ms": 5000
    },
    "planner": {
      "is_active": true,
      "active_strategies": 1,
      "current_positions": 0,
      "market_conditions": {
        "volatility_24h": 0.05,
        "volume_24h": 1000000.0,
        "price_trend": "Sideways",
        "liquidity_score": 0.5
      }
    },
    "executor": {
      "is_active": true,
      "available_permits": 5,
      "total_executions": 42,
      "success_rate": 0.95,
      "avg_execution_time_ms": 150
    },
    "observer": {
      "is_active": true,
      "total_executions_monitored": 42,
      "active_positions": 2,
      "execution_history_size": 100,
      "last_update": "2025-07-23T11:43:54.893Z"
    },
    "performance": {
      "total_trades": 42,
      "successful_trades": 40,
      "total_pnl": "125.50",
      "win_rate": 0.85,
      "avg_slippage_bps": 25.5,
      "avg_execution_time_ms": 150,
      "max_drawdown": 5.2,
      "sharpe_ratio": 1.8,
      "last_updated": "2025-07-23T11:43:54.893Z"
    },
    "active_positions": 2,
    "current_strategy": "Arbitrage"
  },
  "message": "Trading agent is operational"
}
```

**Example:**

```bash
curl http://localhost:3000/api/v1/agent/status
```

```javascript
const response = await fetch("http://localhost:3000/api/v1/agent/status");
const data = await response.json();
console.log("Agent is running:", data.is_running);
console.log("Total trades:", data.stats.performance.total_trades);
```

### `GET /api/v1/agent/state`

Get detailed agent state including configuration and positions.

**Response:**

```json
{
  "is_active": true,
  "current_positions": {
    "SOL/USDC": {
      "token_a": "So11111111111111111111111111111111111111112",
      "token_b": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "amount_a": "1000000000",
      "amount_b": "50000000",
      "entry_price": 50.0,
      "current_price": 52.5,
      "unrealized_pnl": "125.50",
      "position_age_hours": 24
    }
  },
  "performance": {
    "total_trades": 42,
    "successful_trades": 40,
    "total_pnl": "125.50",
    "win_rate": 0.85,
    "avg_slippage_bps": 25.5,
    "avg_execution_time_ms": 150,
    "max_drawdown": 5.2,
    "sharpe_ratio": 1.8,
    "last_updated": "2025-07-23T11:43:54.893Z"
  },
  "strategy_config": {
    "strategy_type": "Arbitrage",
    "parameters": {
      "min_spread_bps": 50,
      "max_slippage_bps": 100,
      "position_size_usd": 100.0,
      "rebalance_threshold_pct": 0.05,
      "lookback_periods": 24,
      "custom_params": {}
    },
    "risk_limits": {
      "max_position_size_usd": 1000.0,
      "max_daily_loss_pct": 5.0,
      "max_drawdown_pct": 15.0,
      "stop_loss_pct": 3.0,
      "take_profit_pct": 10.0
    },
    "execution_settings": {
      "priority_fee_percentile": 50,
      "max_priority_fee_lamports": 10000,
      "transaction_timeout_ms": 30000,
      "retry_attempts": 3,
      "jito_tip_lamports": 10000
    }
  },
  "learning_parameters": {
    "learning_rate": 0.01,
    "adaptation_window_hours": 24,
    "performance_threshold": 0.7,
    "parameter_bounds": {
      "position_size_multiplier": [0.1, 2.0],
      "priority_fee_percentile": [50.0, 99.0],
      "max_slippage_bps": [10.0, 500.0]
    }
  },
  "last_market_data": {
    "SOL/USDC": {
      "price": 52.5,
      "volume_24h": 1000000.0,
      "volatility": 0.05,
      "last_update": "2025-07-23T11:43:54.893Z"
    }
  }
}
```

### `POST /api/v1/agent/start`

Start the trading agent with specified configuration.

**Request Body:**

```json
{
  "openai_api_key": "sk-proj-...",
  "token_pairs": [
    [
      "So11111111111111111111111111111111111111112",
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    ]
  ],
  "strategies": [
    {
      "strategy_type": "Arbitrage",
      "min_spread_bps": 50,
      "max_slippage_bps": 100,
      "position_size_usd": 100.0,
      "max_position_size_usd": 1000.0,
      "priority_fee_percentile": 50,
      "max_priority_fee_lamports": 10000
    }
  ],
  "data_fetch_interval_ms": 5000,
  "learning_enabled": true
}
```

**Response:**

```json
{
  "status": "active",
  "is_running": true,
  "stats": {
    /* Full stats object */
  },
  "message": "Trading agent started successfully"
}
```

**Strategy Types:**

- `"Arbitrage"` - Price differences across DEXs
- `"DCA"` - Dollar Cost Averaging
- `"GridTrading"` - Range-bound trading
- `"Momentum"` - Trend following

**Example:**

```bash
curl -X POST http://localhost:3000/api/v1/agent/start \
  -H "Content-Type: application/json" \
  -d '{
    "openai_api_key": "your-key-here",
    "token_pairs": [["SOL", "USDC"]],
    "strategies": [{
      "strategy_type": "Arbitrage",
      "min_spread_bps": 50,
      "position_size_usd": 100.0
    }],
    "learning_enabled": true
  }'
```

```javascript
const config = {
  openai_api_key: "your-key-here",
  token_pairs: [["SOL", "USDC"]],
  strategies: [
    {
      strategy_type: "Arbitrage",
      min_spread_bps: 50,
      max_slippage_bps: 100,
      position_size_usd: 100.0,
    },
  ],
  data_fetch_interval_ms: 5000,
  learning_enabled: true,
};

const response = await fetch("http://localhost:3000/api/v1/agent/start", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(config),
});

const data = await response.json();
console.log("Agent started:", data.status === "active");
```

### `POST /api/v1/agent/stop`

Stop the trading agent.

**Request Body:**

```json
{}
```

**Response:**

```json
{
  "status": "stopped",
  "is_running": false,
  "stats": null,
  "message": "Trading agent stopped successfully"
}
```

**Example:**

```bash
curl -X POST http://localhost:3000/api/v1/agent/stop \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `POST /api/v1/agent/rebalance`

Force a portfolio rebalance.

**Request Body:**

```json
{}
```

**Response:**

```json
{
  "status": "rebalanced",
  "is_running": true,
  "stats": null,
  "message": "Portfolio rebalanced successfully"
}
```

**Example:**

```bash
curl -X POST http://localhost:3000/api/v1/agent/rebalance \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `POST /api/v1/agent/strategy`

Update the current trading strategy configuration.

**Request Body:**

```json
{
  "strategy_config": {
    "strategy_type": "Arbitrage",
    "min_spread_bps": 75,
    "max_slippage_bps": 150,
    "position_size_usd": 200.0,
    "max_position_size_usd": 2000.0,
    "priority_fee_percentile": 75,
    "max_priority_fee_lamports": 15000
  }
}
```

**Response:**

```json
{
  "status": "updated",
  "message": "Strategy configuration updated successfully",
  "new_config": {
    /* Updated strategy config */
  }
}
```

### `POST /api/v1/agent/emergency-stop`

Emergency stop all trading activities immediately.

**Request Body:**

```json
{}
```

**Response:**

```json
{
  "status": "emergency_stopped",
  "is_running": false,
  "message": "Emergency stop executed - all trading halted"
}
```

---

## 💰 ICM Program Endpoints

### `POST /api/v1/bucket`

Create a new trading bucket.

**Request Body:**

```json
{
  "creator_pubkey": "...",
  "initial_deposit_lamports": 1000000000,
  "strategy_type": "Arbitrage"
}
```

**Response:**

```json
{
  "bucket_pubkey": "...",
  "transaction_signature": "...",
  "status": "created"
}
```

### `GET /api/v1/bucket`

Get bucket information.

**Query Parameters:**

- `bucket_pubkey` (required): The bucket public key

**Response:**

```json
{
  "bucket_pubkey": "...",
  "creator": "...",
  "balance_lamports": 1000000000,
  "strategy_type": "Arbitrage",
  "is_active": true,
  "created_at": "2025-07-23T11:43:54.893Z"
}
```

### `POST /api/v1/bucket/contribute`

Contribute funds to a bucket.

**Request Body:**

```json
{
  "bucket_pubkey": "...",
  "contributor_pubkey": "...",
  "amount_lamports": 500000000
}
```

### `POST /api/v1/bucket/start-trading`

Start trading for a bucket.

**Request Body:**

```json
{
  "bucket_pubkey": "...",
  "creator_pubkey": "..."
}
```

### `POST /api/v1/bucket/swap`

Execute a token swap.

**Request Body:**

```json
{
  "creator_pubkey": "...",
  "bucket_pubkey": "...",
  "input_mint": "So11111111111111111111111111111111111111112",
  "output_mint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "route_plan": [
    /* Jupiter route plan */
  ],
  "in_amount": 1000000000,
  "quoted_out_amount": 50000000,
  "slippage_bps": 100,
  "platform_fee_bps": 0
}
```

**Response:**

```json
{
  "transaction": "base64_encoded_transaction",
  "message": "Swap transaction created successfully"
}
```

### `POST /api/v1/bucket/claim-rewards`

Claim rewards from a bucket.

**Request Body:**

```json
{
  "bucket_pubkey": "...",
  "contributor_pubkey": "..."
}
```

### `POST /api/v1/bucket/close`

Close a trading bucket.

**Request Body:**

```json
{
  "bucket_pubkey": "...",
  "creator_pubkey": "..."
}
```

---

## 🚨 Error Responses

### Standard Error Format

```json
{
  "error": {
    "code": "AGENT_NOT_INITIALIZED",
    "message": "Trading agent is not initialized",
    "details": "Call /api/v1/agent/start first"
  },
  "status": "error",
  "timestamp": "2025-07-23T11:43:54.893Z"
}
```

### Common Error Codes

| Code                      | HTTP Status | Description                 |
| ------------------------- | ----------- | --------------------------- |
| `AGENT_NOT_INITIALIZED`   | 400         | Agent not started           |
| `AGENT_ALREADY_RUNNING`   | 409         | Agent already active        |
| `INVALID_STRATEGY_CONFIG` | 400         | Invalid strategy parameters |
| `INSUFFICIENT_FUNDS`      | 400         | Not enough balance          |
| `NETWORK_ERROR`           | 502         | Solana network issues       |
| `JUPITER_API_ERROR`       | 502         | Jupiter DEX API error       |
| `OPENAI_API_ERROR`        | 502         | OpenAI API error            |
| `INTERNAL_SERVER_ERROR`   | 500         | Server error                |

### Error Handling Examples

```javascript
async function handleApiCall(url, options) {
  try {
    const response = await fetch(url, options);

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(`${errorData.error.code}: ${errorData.error.message}`);
    }

    return await response.json();
  } catch (error) {
    console.error("API Error:", error.message);

    // Handle specific errors
    if (error.message.includes("AGENT_NOT_INITIALIZED")) {
      alert("Please start the trading agent first");
    } else if (error.message.includes("INSUFFICIENT_FUNDS")) {
      alert("Not enough balance for this operation");
    } else {
      alert("An unexpected error occurred");
    }

    throw error;
  }
}
```

---

## 📊 WebSocket Events (Future)

### Connection

```javascript
const ws = new WebSocket("ws://localhost:3000/ws");
```

### Event Types

#### Trade Update

```json
{
  "type": "trade_update",
  "data": {
    "trade_id": "...",
    "strategy": "Arbitrage",
    "token_pair": "SOL/USDC",
    "amount_in": "1000000000",
    "amount_out": "50000000",
    "status": "completed",
    "timestamp": "2025-07-23T11:43:54.893Z"
  }
}
```

#### Metrics Update

```json
{
  "type": "metrics_update",
  "data": {
    "total_trades": 43,
    "win_rate": 0.86,
    "total_pnl": "130.25",
    "timestamp": "2025-07-23T11:43:54.893Z"
  }
}
```

#### Alert

```json
{
  "type": "alert",
  "data": {
    "level": "warning",
    "message": "High slippage detected on SOL/USDC",
    "timestamp": "2025-07-23T11:43:54.893Z"
  }
}
```

---

## 🔒 Authentication (Future)

### JWT Token

```json
{
  "Authorization": "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

### Login Endpoint

```bash
POST /api/v1/auth/login
{
  "username": "trader",
  "password": "secure_password"
}
```

---

## 📈 Rate Limits

| Endpoint         | Limit   | Window   |
| ---------------- | ------- | -------- |
| `/ping`          | 100/min | 1 minute |
| Agent endpoints  | 60/min  | 1 minute |
| Bucket endpoints | 30/min  | 1 minute |

### Rate Limit Headers

```
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 59
X-RateLimit-Reset: 1642694400
```

This completes the comprehensive API reference for your ICM Server trading system! 🚀
