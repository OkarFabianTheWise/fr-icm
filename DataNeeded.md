## ✅ On-Chain: Scalable & Efficient to Save in Anchor

These are lightweight, essential state records that scale well on Solana:

```rust
// 1. Core Pool Entity
pub struct TradingPool {
    pub pool_id: Pubkey,
    pub pool_bump: u8,
    pub creator: Pubkey,
    pub strategy_description: String,
    pub token_bucket: Vec<Pubkey>,
    pub target_amount: u64,
    pub min_contribution: u64,
    pub max_contribution: u64,
    pub trading_duration: u64,
    pub created_at: i64,
    pub fundraising_deadline: i64,
    pub trading_start_time: Option<i64>,
    pub trading_end_time: Option<i64>,
    pub phase: PoolPhase,
    pub raised_amount: u64,
    pub contributor_count: u32,
    pub management_fee: u16,
    pub performance_fee: u16,
    pub trade_count: u32,
    pub last_trade_time: Option<i64>,
}

// 2. Contributions
pub struct PoolContribution {
    pub pool_id: Pubkey,
    pub contributor: Pubkey,
    pub contribution_amount: u64,
    pub contribution_timestamp: i64,
    pub pool_share_percentage: u64,
    pub claimed: bool,
}

// 3. Trade Records (individual trades)
pub struct TradeRecord {
    pub pool_id: Pubkey,
    pub trade_id: u64,
    pub timestamp: i64,
    pub trade_type: TradeType,
    pub from_token: Pubkey,
    pub to_token: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub slippage: u16,
    pub success: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum TradeType {
    BuyToken,
    SellToken,
    Rebalance,
}

// 4. Creator reputation minimal tracking
pub struct CreatorProfile {
    pub creator: Pubkey,
    pub pools_created: u32,
    pub successful_pools: u32,
    pub total_volume_managed: u64,
    pub reputation_score: u32,
    pub created_at: i64,
}
```

## ⚠️ Off-Chain: Better Handled Externally

These items are large, frequently updated, or analytical—better off-loaded to off-chain systems:

```rust
// 5. Portfolio Snapshots (heavy data, use DB)
pub struct PortfolioSnapshot {
    pub pool_id: Pubkey,
    pub timestamp: i64,
    pub total_value_usdc: u64,
    pub token_balances: Vec<TokenBalance>,
    pub pnl_percentage: i16,
}

pub struct TokenBalance {
    pub token_mint: Pubkey,
    pub amount: u64,
    pub value_usdc: u64,
}

// 6. Pool Performance Analytics
pub struct PoolPerformance {
    pub pool_id: Pubkey,
    pub current_pnl: i16,
    pub peak_pnl: i16,
    pub drawdown: i16,
    pub total_trades: u32,
    pub successful_trades: u32,
    pub last_updated: i64,
    pub roi_annualized: i16,
}

// 7. User Profile Aggregates
pub struct UserProfile {
    pub user: Pubkey,
    pub total_pools_joined: u32,
    pub active_contributions: Vec<Pubkey>,
    pub completed_contributions: Vec<Pubkey>,
    pub total_contributed: u64,
    pub total_pnl: i64,
}
```

---

## **Operations**

## 🔍 To Verify It Exists

```sql
-- You can list tables in the current database:
\dt
```

## And describe the trading_pools table's structure:

```sql
\d trading_pools
```

---

## 📁 Off-Chain DB & API Schema (PostgreSQL)

## **Database Schema (PostgreSQL)**

```sql
-- Main pools table
CREATE TABLE trading_pools (
    id VARCHAR PRIMARY KEY,
    creator_pubkey VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    strategy TEXT NOT NULL,
    token_bucket TEXT[], -- JSON array of token addresses
    phase VARCHAR CHECK (phase IN ('fundraising', 'trading', 'completed', 'failed')),
    target_amount BIGINT NOT NULL,
    raised_amount BIGINT DEFAULT 0,
    min_contribution BIGINT NOT NULL,
    max_contribution BIGINT NOT NULL,
    contributor_count INTEGER DEFAULT 0,
    trading_duration INTEGER NOT NULL, -- hours
    fundraising_deadline TIMESTAMP NOT NULL,
    trading_start_time TIMESTAMP,
    trading_end_time TIMESTAMP,
    management_fee INTEGER NOT NULL, -- basis points
    performance_fee INTEGER NOT NULL, -- basis points
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Pool contributions
CREATE TABLE pool_contributions (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR REFERENCES trading_pools(id),
    contributor_pubkey VARCHAR NOT NULL,
    amount BIGINT NOT NULL,
    pool_share_percentage INTEGER NOT NULL, -- basis points
    transaction_signature VARCHAR NOT NULL,
    contributed_at TIMESTAMP DEFAULT NOW(),
    claimed BOOLEAN DEFAULT FALSE
);

-- Trading records
CREATE TABLE trade_records (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR REFERENCES trading_pools(id),
    trade_type VARCHAR NOT NULL,
    from_token VARCHAR NOT NULL,
    to_token VARCHAR NOT NULL,
    amount_in BIGINT NOT NULL,
    amount_out BIGINT NOT NULL,
    transaction_signature VARCHAR NOT NULL,
    executed_at TIMESTAMP DEFAULT NOW(),
    success BOOLEAN DEFAULT TRUE
);

-- pool_performance_snapshots — Time-series charting & PnL history
CREATE TABLE pool_performance_snapshots (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR NOT NULL REFERENCES trading_pools(id),
    snapshot_time TIMESTAMP DEFAULT NOW(),
    portfolio_value BIGINT NOT NULL,         -- In USDC lamports
    pnl_percentage INTEGER NOT NULL,         -- Basis points (-150 = -1.5%)
    token_balances JSONB NOT NULL            -- Array of token balances per snapshot
);

-- pool_performance_metrics — Real-time analytics of pool health
CREATE TABLE pool_performance_metrics (
    pool_id VARCHAR PRIMARY KEY REFERENCES trading_pools(id),
    current_pnl INTEGER NOT NULL,        -- Basis points
    peak_pnl INTEGER NOT NULL,
    drawdown INTEGER NOT NULL,
    total_trades INTEGER NOT NULL,
    successful_trades INTEGER NOT NULL,
    last_updated TIMESTAMP DEFAULT NOW(),
    roi_annualized INTEGER               -- Optional, if available
);

-- user_profiles — Aggregated wallet stats for dashboards
CREATE TABLE user_profiles (
    email VARCHAR, -- not NULL
    password_hash VARCHAR,
    user_pubkey VARCHAR PRIMARY KEY,
    private_key INTEGER[], -- solana private key byte array [222, 323, ..]
    total_pools_joined INTEGER DEFAULT 0,
    active_contributions TEXT[] DEFAULT '{}',     -- List of pool IDs (active)
    completed_contributions TEXT[] DEFAULT '{}',  -- List of pool IDs (completed)
    total_contributed BIGINT DEFAULT 0,           -- USDC lamports
    total_pnl BIGINT DEFAULT 0,                   -- Signed int (negative = loss)
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Portfolio table
CREATE TABLE portfolios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    total_value_usd DECIMAL(20, 8) NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- PortfolioAsset table
CREATE TABLE portfolio_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    asset_symbol VARCHAR(50) NOT NULL,
    asset_type VARCHAR(50) NOT NULL,
    target_allocation_percent DECIMAL(10, 6) NOT NULL,
    current_allocation_percent DECIMAL(10, 6) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    average_cost_usd DECIMAL(20, 8) NOT NULL,
    current_value_usd DECIMAL(20, 8) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

## 🧩 API & WebSocket (Off-Chain)

- **GET /pools**, **POST /pools/{id}/contribute**
- **GET /pools/{id}/performance**
- **GET /user/{pubkey}/pools**, **/activity**
- **WS events**: `FUNDRAISING_UPDATE`, `TRADE_EXECUTED`, `PHASE_CHANGE`

---

## 🛠 Summary: On-Chain vs Off-Chain Responsibilities

| **On-Chain**                     | **Off-Chain**                           |
| -------------------------------- | --------------------------------------- |
| Pool creation & config           | Portfolio snapshots & historical values |
| Contributions & share tracking   | PnL calculations, performance analytics |
| Trade logs (time, type, amounts) | Aggregated user profiles                |
| Creator reputation minimal stats | ROI, drawdown, peak performance metrics |
| Phase/amount updates             | WebSocket notifications, API endpoints  |

---

## ✅ Result: Updated Anchor File

Your Anchor program should handle:

- Pool lifecycle (`TradingPool`)
- Contributions (`PoolContribution`)
- Lightweight trade logs (`TradeRecord`)
- Creator badge (`CreatorProfile`)
