-- Fix trading_pools table timestamp columns to use TIMESTAMP WITH TIME ZONE
-- Migration: 004_fix_trading_pools_timestamps.sql

-- Update trading_end_time column to use TIMESTAMP WITH TIME ZONE
ALTER TABLE trading_pools 
ALTER COLUMN trading_end_time TYPE TIMESTAMP WITH TIME ZONE;

-- Update created_at column to use TIMESTAMP WITH TIME ZONE
ALTER TABLE trading_pools 
ALTER COLUMN created_at TYPE TIMESTAMP WITH TIME ZONE;

-- Update updated_at column to use TIMESTAMP WITH TIME ZONE
ALTER TABLE trading_pools 
ALTER COLUMN updated_at TYPE TIMESTAMP WITH TIME ZONE;

-- Ensure default values are set correctly
ALTER TABLE trading_pools 
ALTER COLUMN created_at SET DEFAULT NOW();

ALTER TABLE trading_pools 
ALTER COLUMN updated_at SET DEFAULT NOW();