-- Testudo Trading Platform - Initial Database Schema
-- Roman Military Discipline in Data Management
--
-- This migration establishes the foundational data structure for
-- disciplined crypto trading with Van Tharp position sizing methodology.

-- Create extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "timescaledb";

-- User accounts and authentication
CREATE TABLE user_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    role VARCHAR(50) NOT NULL DEFAULT 'trader',
    
    -- Risk management settings
    default_risk_percentage DECIMAL(5,4) NOT NULL DEFAULT 0.02,  -- 2%
    max_risk_percentage DECIMAL(5,4) NOT NULL DEFAULT 0.06,      -- 6%
    daily_loss_limit DECIMAL(12,2) NOT NULL DEFAULT 500.00,
    max_consecutive_losses INTEGER NOT NULL DEFAULT 3,
    
    -- Account equity tracking
    current_equity DECIMAL(18,8) NOT NULL DEFAULT 0,
    last_equity_update TIMESTAMPTZ,
    
    -- Trading preferences
    preferred_exchange VARCHAR(50) NOT NULL DEFAULT 'binance',
    trading_symbols TEXT[] DEFAULT ARRAY['BTC/USDT', 'ETH/USDT'],
    
    CONSTRAINT valid_risk_percentage CHECK (
        default_risk_percentage >= 0.005 AND default_risk_percentage <= 0.06
    ),
    CONSTRAINT valid_max_risk CHECK (
        max_risk_percentage >= default_risk_percentage
    )
);

-- Exchange API credentials (encrypted)
CREATE TABLE user_exchange_credentials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES user_accounts(id) ON DELETE CASCADE,
    exchange_name VARCHAR(50) NOT NULL,
    api_key_encrypted TEXT NOT NULL,
    api_secret_encrypted TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    
    UNIQUE(user_id, exchange_name)
);

-- Trading positions with immutable history
CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES user_accounts(id),
    
    -- Trade identification
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('BUY', 'SELL')),
    
    -- Van Tharp calculation inputs (immutable)
    entry_price DECIMAL(18,8) NOT NULL,
    stop_loss DECIMAL(18,8) NOT NULL,
    take_profit DECIMAL(18,8),
    account_equity_at_entry DECIMAL(18,8) NOT NULL,
    risk_percentage DECIMAL(5,4) NOT NULL,
    calculated_position_size DECIMAL(18,8) NOT NULL,
    
    -- Execution details
    actual_position_size DECIMAL(18,8) NOT NULL,
    average_entry_price DECIMAL(18,8),
    entry_order_ids TEXT[],
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    
    -- Timestamps (immutable once set)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    entered_at TIMESTAMPTZ,
    exited_at TIMESTAMPTZ,
    
    -- P&L calculation (updated as position evolves)
    unrealized_pnl DECIMAL(18,8) DEFAULT 0,
    realized_pnl DECIMAL(18,8) DEFAULT 0,
    current_price DECIMAL(18,8),
    last_price_update TIMESTAMPTZ,
    
    -- R-multiple analysis
    r_multiple DECIMAL(8,4),
    risk_amount DECIMAL(18,8) GENERATED ALWAYS AS 
        (account_equity_at_entry * risk_percentage) STORED,
    
    CONSTRAINT valid_position_status CHECK (
        status IN ('PENDING', 'OPEN', 'CLOSED', 'CANCELLED')
    ),
    CONSTRAINT valid_side CHECK (side IN ('BUY', 'SELL')),
    CONSTRAINT valid_prices CHECK (
        entry_price > 0 AND stop_loss > 0 AND
        CASE WHEN take_profit IS NOT NULL THEN take_profit > 0 ELSE TRUE END
    )
);

-- Convert positions to hypertable for time-series optimization
SELECT create_hypertable('positions', 'created_at', if_not_exists => TRUE);

-- Trade execution history (OODA loop audit trail)
CREATE TABLE trade_executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    position_id UUID NOT NULL REFERENCES positions(id),
    
    -- OODA loop phase timing
    ooda_phase VARCHAR(20) NOT NULL CHECK (
        ooda_phase IN ('OBSERVE', 'ORIENT', 'DECIDE', 'ACT')
    ),
    phase_start_time TIMESTAMPTZ NOT NULL,
    phase_duration_ms INTEGER NOT NULL,
    phase_success BOOLEAN NOT NULL,
    
    -- Market observation data
    market_price DECIMAL(18,8),
    bid_price DECIMAL(18,8),
    ask_price DECIMAL(18,8),
    spread_bps INTEGER,
    market_data_age_ms INTEGER,
    
    -- Risk calculation results
    calculated_size DECIMAL(18,8),
    risk_assessment JSONB,
    protocol_violations TEXT[],
    
    -- Exchange execution
    exchange_order_id VARCHAR(100),
    exchange_response JSONB,
    execution_price DECIMAL(18,8),
    executed_quantity DECIMAL(18,8),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT create_hypertable('trade_executions', 'created_at', if_not_exists => TRUE);

-- Market data cache
CREATE TABLE market_data (
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    
    price DECIMAL(18,8) NOT NULL,
    bid DECIMAL(18,8) NOT NULL,
    ask DECIMAL(18,8) NOT NULL,
    volume_24h DECIMAL(18,8) NOT NULL,
    price_change_24h DECIMAL(8,4) NOT NULL,
    
    -- Order book snapshot
    bids JSONB,
    asks JSONB,
    
    PRIMARY KEY (symbol, exchange, timestamp)
);

SELECT create_hypertable('market_data', 'timestamp', if_not_exists => TRUE);

-- System events and audit log
CREATE TABLE system_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL CHECK (
        severity IN ('DEBUG', 'INFO', 'WARN', 'ERROR', 'CRITICAL')
    ),
    component VARCHAR(50) NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    user_id UUID REFERENCES user_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT create_hypertable('system_events', 'created_at', if_not_exists => TRUE);

-- Risk calculation verification log
CREATE TABLE risk_calculations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES user_accounts(id),
    
    -- Van Tharp inputs
    account_equity DECIMAL(18,8) NOT NULL,
    risk_percentage DECIMAL(5,4) NOT NULL,
    entry_price DECIMAL(18,8) NOT NULL,
    stop_loss DECIMAL(18,8) NOT NULL,
    
    -- Calculation results
    calculated_position_size DECIMAL(18,8) NOT NULL,
    calculation_time_ms INTEGER NOT NULL,
    verification_hash VARCHAR(64) NOT NULL, -- SHA-256 of inputs + result
    
    -- Verification status
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    verification_method VARCHAR(50),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT create_hypertable('risk_calculations', 'created_at', if_not_exists => TRUE);

-- Indexes for performance optimization
CREATE INDEX idx_user_accounts_email ON user_accounts(email);
CREATE INDEX idx_user_accounts_active ON user_accounts(is_active) WHERE is_active = TRUE;

CREATE INDEX idx_positions_user_status ON positions(user_id, status);
CREATE INDEX idx_positions_symbol ON positions(symbol);
CREATE INDEX idx_positions_created_at ON positions(created_at);

CREATE INDEX idx_trade_executions_position ON trade_executions(position_id);
CREATE INDEX idx_trade_executions_phase ON trade_executions(ooda_phase);

CREATE INDEX idx_market_data_symbol_time ON market_data(symbol, timestamp);
CREATE INDEX idx_system_events_type_time ON system_events(event_type, created_at);

-- Insert default system configuration
INSERT INTO system_events (event_type, severity, component, message, metadata) 
VALUES (
    'SYSTEM_INIT', 'INFO', 'DATABASE', 
    'Testudo trading platform database initialized',
    '{"schema_version": "001", "migration": "initial_schema"}'::jsonb
);

-- Comments for documentation
COMMENT ON TABLE user_accounts IS 'User accounts with integrated risk management settings';
COMMENT ON TABLE positions IS 'Immutable trading positions with Van Tharp calculations';  
COMMENT ON TABLE trade_executions IS 'OODA loop execution audit trail';
COMMENT ON TABLE market_data IS 'Real-time market data cache';
COMMENT ON TABLE risk_calculations IS 'Risk calculation verification log';

COMMENT ON COLUMN positions.r_multiple IS 'R-multiple = (Exit Price - Entry Price) / (Entry Price - Stop Loss)';
COMMENT ON COLUMN positions.risk_amount IS 'Dollar risk amount = Account Equity × Risk Percentage';

-- Triggers for updated_at timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_user_accounts_updated_at 
    BEFORE UPDATE ON user_accounts 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();