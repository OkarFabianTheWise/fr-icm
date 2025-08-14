//! # Authentication Module
//!
//! Handles JWT token issuance, validation, and middleware for securing API endpoints.
//! This module provides the foundation for user authentication and authorization
//! in the ICM server.

pub mod jwt;
pub mod middleware;
pub mod models;

// pub use jwt::{Claims, JwtService};
// pub use middleware::AuthMiddleware;
// pub use models::{AuthUser, LoginRequest, TokenResponse};
