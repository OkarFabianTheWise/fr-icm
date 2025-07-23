//! JWT Token Service
//!
//! Handles JWT creation, validation, and claims management for user authentication.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims structure containing user information and token metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User unique identifier
    pub sub: Uuid,
    /// User email
    pub email: String,
    /// Token issued at timestamp
    pub iat: i64,
    /// Token expiration timestamp
    pub exp: i64,
    /// Token issuer
    pub iss: String,
}

/// JWT Service for token operations
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtService {
    /// Create a new JWT service with the provided secret
    pub fn new(secret: &str) -> Self {
        let encoding_key = EncodingKey::from_secret(secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(secret.as_bytes());

        let mut validation = Validation::default();
        validation.set_issuer(&["icm-server"]);

        Self {
            encoding_key,
            decoding_key,
            validation,
        }
    }

    /// Generate a JWT token for a user
    pub fn create_token(&self, user_id: Uuid, email: String) -> Result<String> {
        let now = Utc::now();
        let expiration = now + Duration::hours(24); // 24 hour expiration

        let claims = Claims {
            sub: user_id,
            email,
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            iss: "icm-server".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .context("Failed to encode JWT token")
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> Result<TokenData<Claims>> {
        decode::<Claims>(token, &self.decoding_key, &self.validation)
            .context("Failed to validate JWT token")
    }

    /// Extract claims from a token without full validation (for debugging)
    pub fn decode_claims(&self, token: &str) -> Result<Claims> {
        let token_data = self.validate_token(token)?;
        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_roundtrip() {
        let jwt_service = JwtService::new("test_secret");
        let user_id = Uuid::new_v4();
        let email = "test@example.com".to_string();

        // Create token
        let token = jwt_service.create_token(user_id, email.clone()).unwrap();

        // Validate token
        let claims = jwt_service.decode_claims(&token).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert_eq!(claims.iss, "icm-server");
    }
}
