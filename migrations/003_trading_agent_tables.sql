-- Create trading agent related tables for ICM server
-- Migration: 003_trading_agent_tables.sql

-- Enable pgcrypto extension for UUID generation
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Trading sessions table
CREATE TABLE IF NOT EXISTS trading_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    strategy_type VARCHAR(50) NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    start_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20) NOT NULL DEFAULT 'Active',
    total_trades INTEGER NOT NULL DEFAULT 0,
    successful_trades INTEGER NOT NULL DEFAULT 0,
    total_pnl DECIMAL(20, 8) NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Trade executions table
CREATE TABLE IF NOT EXISTS trade_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES trading_sessions(id) ON DELETE CASCADE,
    trade_id VARCHAR(100) NOT NULL,
    strategy_type VARCHAR(50) NOT NULL,
    input_token VARCHAR(50) NOT NULL,
    output_token VARCHAR(50) NOT NULL,
    input_amount DECIMAL(20, 8) NOT NULL,
    output_amount DECIMAL(20, 8),
    expected_output DECIMAL(20, 8) NOT NULL,
    slippage DECIMAL(10, 6),
    gas_fee DECIMAL(20, 8),
    transaction_signature VARCHAR(200),
    status VARCHAR(20) NOT NULL DEFAULT 'Pending',
    error_message TEXT,
    execution_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Market data snapshots table
CREATE TABLE IF NOT EXISTS market_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol VARCHAR(20) NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    volume_24h DECIMAL(20, 8),
    price_change_24h DECIMAL(10, 6),
    liquidity DECIMAL(20, 8),
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    source VARCHAR(50) NOT NULL DEFAULT 'jupiter'
);

-- AI decisions table for learning and debugging
CREATE TABLE IF NOT EXISTS ai_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES trading_sessions(id) ON DELETE CASCADE,
    decision_type VARCHAR(50) NOT NULL,
    input_data JSONB NOT NULL,
    output_decision JSONB NOT NULL,
    confidence_score DECIMAL(5, 4),
    execution_result VARCHAR(20),
    feedback_score DECIMAL(5, 4),
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Strategy performance metrics table
CREATE TABLE IF NOT EXISTS strategy_performance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_type VARCHAR(50) NOT NULL,
    time_period VARCHAR(10) NOT NULL,
    total_trades INTEGER NOT NULL,
    successful_trades INTEGER NOT NULL,
    total_pnl DECIMAL(20, 8) NOT NULL,
    max_drawdown DECIMAL(20, 8) NOT NULL,
    sharpe_ratio DECIMAL(10, 6),
    win_rate DECIMAL(5, 4) NOT NULL,
    avg_trade_duration BIGINT,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(strategy_type, time_period, calculated_at)
);

-- Portfolio table
CREATE TABLE IF NOT EXISTS portfolios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id VARCHAR NOT NULL REFERENCES user_profiles(user_pubkey),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    total_value_usd DECIMAL(20, 8) NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- PortfolioAsset table
CREATE TABLE IF NOT EXISTS portfolio_assets (
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

CREATE TABLE IF NOT EXISTS trading_pools (
    id VARCHAR PRIMARY KEY,
    creator_pubkey VARCHAR NOT NULL REFERENCES user_profiles(user_pubkey),
    name VARCHAR NOT NULL,
    strategy TEXT NOT NULL, -- Trading strategy identifier
    token_bucket TEXT[] NOT NULL, -- Array of 1-3 token mint addresses
    total_amount_available_to_trade BIGINT NOT NULL, -- Total funds available for trading (in lamports or smallest unit)
    trading_end_time TIMESTAMP WITH TIME ZONE NOT NULL, -- Deadline/timeframe for trading to end
    management_fee INTEGER NOT NULL, -- Management fee in basis points
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Pool contributions
CREATE TABLE IF NOT EXISTS pool_contributions (
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
CREATE TABLE IF NOT EXISTS trade_records (
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
CREATE TABLE IF NOT EXISTS pool_performance_snapshots (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR NOT NULL REFERENCES trading_pools(id),
    snapshot_time TIMESTAMP DEFAULT NOW(),
    portfolio_value BIGINT NOT NULL,         -- In USDC lamports
    pnl_percentage INTEGER NOT NULL,         -- Basis points (-150 = -1.5%)
    token_balances JSONB NOT NULL            -- Array of token balances per snapshot
);

-- pool_performance_metrics — Real-time analytics of pool health
CREATE TABLE IF NOT EXISTS pool_performance_metrics (
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
CREATE TABLE IF NOT EXISTS user_profiles (
    email VARCHAR, -- not NULL
    password_hash VARCHAR,
    user_pubkey VARCHAR PRIMARY KEY,
    private_key INTEGER[], -- solana private key byte array [222, 323, ..]
    total_pools_joined INTEGER DEFAULT 0,
    active_contributions TEXT[] DEFAULT '{}',     -- List of pool IDs (active)
    completed_contributions TEXT[] DEFAULT '{}',  -- List of pool IDs (completed)
    total_contributed BIGINT DEFAULT 0,           -- USDC lamports
    total_pnl BIGINT DEFAULT 0,                   -- Signed int (negative = loss)
    last_faucet_claim TIMESTAMP,
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_trading_sessions_user_id ON trading_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_trading_sessions_status ON trading_sessions(status);
CREATE INDEX IF NOT EXISTS idx_trading_sessions_strategy ON trading_sessions(strategy_type);
CREATE INDEX IF NOT EXISTS idx_trading_sessions_created_at ON trading_sessions(created_at);

CREATE INDEX IF NOT EXISTS idx_trade_executions_session_id ON trade_executions(session_id);
CREATE INDEX IF NOT EXISTS idx_trade_executions_status ON trade_executions(status);
CREATE INDEX IF NOT EXISTS idx_trade_executions_strategy ON trade_executions(strategy_type);
CREATE INDEX IF NOT EXISTS idx_trade_executions_execution_time ON trade_executions(execution_time);
CREATE INDEX IF NOT EXISTS idx_trade_executions_trade_id ON trade_executions(trade_id);

CREATE INDEX IF NOT EXISTS idx_market_snapshots_symbol ON market_snapshots(symbol);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_timestamp ON market_snapshots(timestamp);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_symbol_timestamp ON market_snapshots(symbol, timestamp);

CREATE INDEX IF NOT EXISTS idx_ai_decisions_session_id ON ai_decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_ai_decisions_decision_type ON ai_decisions(decision_type);
CREATE INDEX IF NOT EXISTS idx_ai_decisions_timestamp ON ai_decisions(timestamp);

CREATE INDEX IF NOT EXISTS idx_strategy_performance_strategy_type ON strategy_performance(strategy_type);
CREATE INDEX IF NOT EXISTS idx_strategy_performance_calculated_at ON strategy_performance(calculated_at);

-- Update function for updated_at timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for updated_at
CREATE TRIGGER update_trading_sessions_updated_at 
    BEFORE UPDATE ON trading_sessions 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_portfolios_updated_at 
    BEFORE UPDATE ON portfolios 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Add some helpful views for analytics
CREATE OR REPLACE VIEW strategy_summary AS
SELECT 
    strategy_type,
    COUNT(*) as total_sessions,
    SUM(total_trades) as total_trades,
    SUM(successful_trades) as successful_trades,
    ROUND(AVG(CASE WHEN total_trades > 0 THEN (successful_trades::decimal / total_trades::decimal) * 100 ELSE 0 END), 2) as avg_win_rate,
    SUM(total_pnl) as total_pnl,
    AVG(total_pnl) as avg_pnl_per_session
FROM trading_sessions 
GROUP BY strategy_type;

CREATE OR REPLACE VIEW recent_performance AS
SELECT 
    DATE_TRUNC('day', execution_time) as trade_date,
    strategy_type,
    COUNT(*) as trades_count,
    SUM(CASE WHEN status = 'Success' THEN 1 ELSE 0 END) as successful_trades,
    AVG(CASE WHEN output_amount IS NOT NULL AND input_amount > 0 
        THEN ((output_amount - expected_output) / expected_output) * 100 
        ELSE 0 END) as avg_slippage_pct
FROM trade_executions 
WHERE execution_time >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY DATE_TRUNC('day', execution_time), strategy_type
ORDER BY trade_date DESC;
