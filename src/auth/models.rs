//! Authentication Models
//! 
//! Data structures for authentication requests, responses, and user information.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authenticated user information extracted from JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
}

/// Login request payload
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Token response after successful authentication
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: AuthUser,
}

impl TokenResponse {
    pub fn new(access_token: String, user: AuthUser) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: 24 * 60 * 60, // 24 hours in seconds
            user,
        }
    }
}
