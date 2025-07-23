//! # Database Module
//! 
//! Database integration using SQLx with PostgreSQL for async operations.
//! Includes connection management, models, and migrations.

pub mod connection;
pub mod models;
pub mod migrations;

pub use connection::{DatabaseConnection, DatabaseConfig};
pub use models::*;
