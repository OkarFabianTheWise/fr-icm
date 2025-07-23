# Frontend Integration Guide

This guide explains how to build a frontend for the ICM Server trading system. **Perfect for beginners!**

## 🎯 What You'll Build

A simple web dashboard that can:

- ✅ Check if the trading server is running
- ✅ Start and stop the AI trading agent
- ✅ Monitor trading performance in real-time
- ✅ View trading history and metrics
- ✅ Configure trading strategies

## 🚀 Quick Start (5 Minutes)

### Step 1: Create a Simple HTML Page

Create `index.html`:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ICM Trading Dashboard</title>
    <style>
      body {
        font-family: Arial, sans-serif;
        max-width: 1200px;
        margin: 0 auto;
        padding: 20px;
      }
      .card {
        background: #f5f5f5;
        padding: 20px;
        margin: 10px 0;
        border-radius: 8px;
      }
      .status {
        padding: 10px;
        border-radius: 4px;
        margin: 10px 0;
      }
      .success {
        background: #d4edda;
        color: #155724;
      }
      .error {
        background: #f8d7da;
        color: #721c24;
      }
      .warning {
        background: #fff3cd;
        color: #856404;
      }
      button {
        padding: 10px 20px;
        margin: 5px;
        border: none;
        border-radius: 4px;
        cursor: pointer;
      }
      .btn-primary {
        background: #007bff;
        color: white;
      }
      .btn-success {
        background: #28a745;
        color: white;
      }
      .btn-danger {
        background: #dc3545;
        color: white;
      }
      .metrics {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 10px;
      }
      .metric {
        background: white;
        padding: 15px;
        border-radius: 4px;
        text-align: center;
      }
    </style>
  </head>
  <body>
    <h1>🤖 ICM Trading Dashboard</h1>

    <!-- Server Status -->
    <div class="card">
      <h2>Server Status</h2>
      <div id="server-status" class="status">Checking...</div>
      <button onclick="checkServerStatus()" class="btn-primary">
        Refresh Status
      </button>
    </div>

    <!-- Agent Control -->
    <div class="card">
      <h2>Trading Agent Control</h2>
      <div id="agent-status" class="status">Loading...</div>
      <button onclick="startAgent()" class="btn-success">Start Trading</button>
      <button onclick="stopAgent()" class="btn-danger">Stop Trading</button>
      <button onclick="rebalanceAgent()" class="btn-primary">Rebalance</button>
    </div>

    <!-- Performance Metrics -->
    <div class="card">
      <h2>Performance Metrics</h2>
      <div id="metrics" class="metrics">
        <div class="metric">
          <h3>Total Trades</h3>
          <div id="total-trades">-</div>
        </div>
        <div class="metric">
          <h3>Success Rate</h3>
          <div id="success-rate">-</div>
        </div>
        <div class="metric">
          <h3>Total P&L</h3>
          <div id="total-pnl">-</div>
        </div>
        <div class="metric">
          <h3>Win Rate</h3>
          <div id="win-rate">-</div>
        </div>
      </div>
    </div>

    <script src="dashboard.js"></script>
  </body>
</html>
```

### Step 2: Create JavaScript Functions

Create `dashboard.js`:

```javascript
// Base URL for your ICM Server
const API_BASE = "http://localhost:3000";

// Check if server is running
async function checkServerStatus() {
  const statusDiv = document.getElementById("server-status");

  try {
    const response = await fetch(`${API_BASE}/ping`);
    const data = await response.json();

    if (data.status === "pong") {
      statusDiv.className = "status success";
      statusDiv.textContent = "✅ Server is running and healthy";
    } else {
      statusDiv.className = "status warning";
      statusDiv.textContent = "⚠️ Server responded but status unclear";
    }
  } catch (error) {
    statusDiv.className = "status error";
    statusDiv.textContent = "❌ Server is not running or unreachable";
    console.error("Server check failed:", error);
  }
}

// Get agent status and update display
async function getAgentStatus() {
  const statusDiv = document.getElementById("agent-status");

  try {
    const response = await fetch(`${API_BASE}/api/v1/agent/status`);
    const data = await response.json();

    if (data.status === "active" && data.is_running) {
      statusDiv.className = "status success";
      statusDiv.textContent = "✅ Trading Agent is ACTIVE and RUNNING";
    } else if (data.status === "active") {
      statusDiv.className = "status warning";
      statusDiv.textContent = "⚠️ Trading Agent is ACTIVE but NOT RUNNING";
    } else {
      statusDiv.className = "status error";
      statusDiv.textContent = "❌ Trading Agent is INACTIVE";
    }

    // Update metrics if available
    if (data.stats && data.stats.performance) {
      updateMetrics(data.stats.performance);
    }
  } catch (error) {
    statusDiv.className = "status error";
    statusDiv.textContent = "❌ Could not get agent status";
    console.error("Agent status check failed:", error);
  }
}

// Start the trading agent
async function startAgent() {
  const statusDiv = document.getElementById("agent-status");
  statusDiv.textContent = "⏳ Starting trading agent...";

  // Simple configuration - you can make this more advanced later
  const config = {
    openai_api_key: "your-openai-key-here", // Replace with your key
    token_pairs: [
      [
        "So11111111111111111111111111111111111111112",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      ], // SOL/USDC
    ],
    strategies: [
      {
        strategy_type: "Arbitrage",
        min_spread_bps: 50,
        max_slippage_bps: 100,
        position_size_usd: 10.0, // Start small!
        max_position_size_usd: 100.0,
        priority_fee_percentile: 50,
        max_priority_fee_lamports: 10000,
      },
    ],
    data_fetch_interval_ms: 5000,
    learning_enabled: true,
  };

  try {
    const response = await fetch(`${API_BASE}/api/v1/agent/start`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(config),
    });

    const data = await response.json();

    if (response.ok) {
      statusDiv.className = "status success";
      statusDiv.textContent = "✅ Trading agent started successfully!";
    } else {
      statusDiv.className = "status error";
      statusDiv.textContent = `❌ Failed to start: ${
        data.message || "Unknown error"
      }`;
    }
  } catch (error) {
    statusDiv.className = "status error";
    statusDiv.textContent = "❌ Network error starting agent";
    console.error("Start agent failed:", error);
  }
}

// Stop the trading agent
async function stopAgent() {
  const statusDiv = document.getElementById("agent-status");
  statusDiv.textContent = "⏳ Stopping trading agent...";

  try {
    const response = await fetch(`${API_BASE}/api/v1/agent/stop`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({}),
    });

    const data = await response.json();

    if (response.ok) {
      statusDiv.className = "status warning";
      statusDiv.textContent = "⚠️ Trading agent stopped";
    } else {
      statusDiv.className = "status error";
      statusDiv.textContent = `❌ Failed to stop: ${
        data.message || "Unknown error"
      }`;
    }
  } catch (error) {
    statusDiv.className = "status error";
    statusDiv.textContent = "❌ Network error stopping agent";
    console.error("Stop agent failed:", error);
  }
}

// Rebalance portfolio
async function rebalanceAgent() {
  const statusDiv = document.getElementById("agent-status");
  statusDiv.textContent = "⏳ Rebalancing portfolio...";

  try {
    const response = await fetch(`${API_BASE}/api/v1/agent/rebalance`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({}),
    });

    const data = await response.json();

    if (response.ok) {
      statusDiv.className = "status success";
      statusDiv.textContent = "✅ Portfolio rebalanced successfully!";
    } else {
      statusDiv.className = "status error";
      statusDiv.textContent = `❌ Rebalance failed: ${
        data.message || "Unknown error"
      }`;
    }
  } catch (error) {
    statusDiv.className = "status error";
    statusDiv.textContent = "❌ Network error during rebalance";
    console.error("Rebalance failed:", error);
  }
}

// Update performance metrics display
function updateMetrics(performance) {
  document.getElementById("total-trades").textContent =
    performance.total_trades || "0";
  document.getElementById("success-rate").textContent =
    performance.total_trades > 0
      ? `${(
          (performance.successful_trades / performance.total_trades) *
          100
        ).toFixed(1)}%`
      : "0%";
  document.getElementById("total-pnl").textContent = `$${
    performance.total_pnl || "0.00"
  }`;
  document.getElementById("win-rate").textContent = `${(
    performance.win_rate * 100
  ).toFixed(1)}%`;
}

// Auto-refresh every 10 seconds
setInterval(() => {
  checkServerStatus();
  getAgentStatus();
}, 10000);

// Initial load
window.addEventListener("load", () => {
  checkServerStatus();
  getAgentStatus();
});
```

### Step 3: Open in Browser

1. **Save both files** in the same folder
2. **Update the OpenAI API key** in `dashboard.js` (line 58)
3. **Make sure your ICM Server is running** (`cargo run`)
4. **Open `index.html`** in your web browser
5. **You should see your dashboard!** 🎉

## 🎨 Advanced Features

### Real-Time Updates with WebSocket

Add real-time updates to your dashboard:

```javascript
// Add this to dashboard.js
function connectWebSocket() {
  // Note: You'll need to add WebSocket support to your Rust server
  const ws = new WebSocket("ws://localhost:3000/ws");

  ws.onmessage = function (event) {
    const data = JSON.parse(event.data);

    if (data.type === "trade_update") {
      updateTradeHistory(data.trade);
    } else if (data.type === "metrics_update") {
      updateMetrics(data.metrics);
    }
  };

  ws.onopen = function () {
    console.log("WebSocket connected");
  };

  ws.onclose = function () {
    console.log("WebSocket disconnected, reconnecting...");
    setTimeout(connectWebSocket, 5000);
  };
}
```

### Strategy Configuration Form

Add a form to configure trading strategies:

```html
<!-- Add this to your HTML -->
<div class="card">
  <h2>Strategy Configuration</h2>
  <form id="strategy-form">
    <label>
      Strategy Type:
      <select id="strategy-type">
        <option value="Arbitrage">Arbitrage</option>
        <option value="DCA">Dollar Cost Average</option>
        <option value="GridTrading">Grid Trading</option>
      </select>
    </label>
    <br /><br />

    <label>
      Position Size (USD):
      <input type="number" id="position-size" value="10" min="1" max="1000" />
    </label>
    <br /><br />

    <label>
      Max Slippage (BPS):
      <input type="number" id="max-slippage" value="100" min="10" max="500" />
    </label>
    <br /><br />

    <button type="button" onclick="updateStrategy()" class="btn-primary">
      Update Strategy
    </button>
  </form>
</div>
```

```javascript
// Add this function to dashboard.js
async function updateStrategy() {
  const strategyType = document.getElementById("strategy-type").value;
  const positionSize = parseFloat(
    document.getElementById("position-size").value
  );
  const maxSlippage = parseInt(document.getElementById("max-slippage").value);

  const config = {
    strategy_config: {
      strategy_type: strategyType,
      position_size_usd: positionSize,
      max_slippage_bps: maxSlippage,
    },
  };

  try {
    const response = await fetch(`${API_BASE}/api/v1/agent/strategy`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(config),
    });

    if (response.ok) {
      alert("Strategy updated successfully!");
    } else {
      alert("Failed to update strategy");
    }
  } catch (error) {
    alert("Network error updating strategy");
    console.error(error);
  }
}
```

## 📱 Framework Examples

### React.js Example

```jsx
import React, { useState, useEffect } from "react";

function TradingDashboard() {
  const [serverStatus, setServerStatus] = useState("checking");
  const [agentStatus, setAgentStatus] = useState(null);
  const [metrics, setMetrics] = useState({});

  useEffect(() => {
    const checkStatus = async () => {
      try {
        const response = await fetch("http://localhost:3000/ping");
        const data = await response.json();
        setServerStatus(data.status === "pong" ? "online" : "error");
      } catch (error) {
        setServerStatus("offline");
      }
    };

    checkStatus();
    const interval = setInterval(checkStatus, 10000);
    return () => clearInterval(interval);
  }, []);

  const startAgent = async () => {
    const config = {
      openai_api_key: "your-key-here",
      token_pairs: [["SOL", "USDC"]],
      strategies: [
        {
          strategy_type: "Arbitrage",
          position_size_usd: 10.0,
        },
      ],
    };

    try {
      const response = await fetch("http://localhost:3000/api/v1/agent/start", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
      });

      if (response.ok) {
        alert("Agent started!");
      }
    } catch (error) {
      alert("Failed to start agent");
    }
  };

  return (
    <div className="dashboard">
      <h1>ICM Trading Dashboard</h1>

      <div className="status-card">
        <h2>Server Status</h2>
        <div className={`status ${serverStatus}`}>
          {serverStatus === "online" && "✅ Online"}
          {serverStatus === "offline" && "❌ Offline"}
          {serverStatus === "checking" && "⏳ Checking..."}
        </div>
      </div>

      <div className="control-card">
        <h2>Agent Control</h2>
        <button onClick={startAgent} className="btn-success">
          Start Trading
        </button>
      </div>
    </div>
  );
}

export default TradingDashboard;
```

### Vue.js Example

```vue
<template>
  <div class="dashboard">
    <h1>ICM Trading Dashboard</h1>

    <div class="card">
      <h2>Server Status</h2>
      <div :class="['status', serverStatus]">
        {{ serverStatusText }}
      </div>
      <button @click="checkServerStatus" class="btn-primary">Refresh</button>
    </div>

    <div class="card">
      <h2>Agent Control</h2>
      <button @click="startAgent" class="btn-success">Start Trading</button>
      <button @click="stopAgent" class="btn-danger">Stop Trading</button>
    </div>
  </div>
</template>

<script>
export default {
  data() {
    return {
      serverStatus: "checking",
      agentStatus: null,
    };
  },

  computed: {
    serverStatusText() {
      return (
        {
          online: "✅ Server Online",
          offline: "❌ Server Offline",
          checking: "⏳ Checking...",
        }[this.serverStatus] || "Unknown"
      );
    },
  },

  methods: {
    async checkServerStatus() {
      try {
        const response = await fetch("http://localhost:3000/ping");
        const data = await response.json();
        this.serverStatus = data.status === "pong" ? "online" : "error";
      } catch (error) {
        this.serverStatus = "offline";
      }
    },

    async startAgent() {
      const config = {
        openai_api_key: "your-key-here",
        token_pairs: [["SOL", "USDC"]],
        strategies: [
          {
            strategy_type: "Arbitrage",
            position_size_usd: 10.0,
          },
        ],
      };

      try {
        const response = await fetch(
          "http://localhost:3000/api/v1/agent/start",
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(config),
          }
        );

        if (response.ok) {
          this.$toast.success("Agent started successfully!");
        }
      } catch (error) {
        this.$toast.error("Failed to start agent");
      }
    },
  },

  mounted() {
    this.checkServerStatus();
    setInterval(this.checkServerStatus, 10000);
  },
};
</script>
```

## 🎯 Complete API Reference

### All Available Endpoints

```javascript
// Health Check
GET / ping;
// Response: { "status": "pong" }

// Agent Status
GET / api / v1 / agent / status;
// Response: { "status": "active", "is_running": true, "stats": {...} }

// Agent State (Detailed)
GET / api / v1 / agent / state;
// Response: { "is_active": true, "current_positions": {...}, "performance": {...} }

// Start Agent
POST / api / v1 / agent / start;
// Body: { "openai_api_key": "...", "token_pairs": [...], "strategies": [...] }

// Stop Agent
POST / api / v1 / agent / stop;
// Body: {}

// Rebalance Portfolio
POST / api / v1 / agent / rebalance;
// Body: {}

// Update Strategy
POST / api / v1 / agent / strategy;
// Body: { "strategy_config": {...} }

// Emergency Stop
POST / api / v1 / agent / emergency - stop;
// Body: {}
```

### Error Handling

```javascript
// Always handle errors properly
async function apiCall(url, options = {}) {
  try {
    const response = await fetch(url, {
      headers: {
        "Content-Type": "application/json",
        ...options.headers,
      },
      ...options,
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    return await response.json();
  } catch (error) {
    console.error("API call failed:", error);
    throw error;
  }
}

// Usage
try {
  const data = await apiCall("http://localhost:3000/api/v1/agent/status");
  console.log("Agent status:", data);
} catch (error) {
  alert("Failed to get agent status: " + error.message);
}
```

## 🔒 Security Best Practices

### Environment Variables

```javascript
// Don't hardcode API keys! Use environment variables
const config = {
  openai_api_key: process.env.REACT_APP_OPENAI_API_KEY,
  // ... other config
};
```

### CORS Setup

Your Rust server should handle CORS. Add this to your server:

```rust
// In your Rust server (src/server.rs)
use tower_http::cors::{CorsLayer, Any};

let app = Router::new()
    .route("/ping", get(ping))
    .layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    );
```

This completes your frontend integration guide! You now have everything you need to build a beautiful, functional frontend for your ICM trading system. Start with the simple HTML example and then upgrade to your preferred framework when you're ready! 🚀
