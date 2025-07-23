//! Authentication Middleware
//! 
//! Axum middleware for JWT token validation and user authentication.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::auth::{jwt::JwtService, models::AuthUser};

/// Authentication middleware that validates JWT tokens and injects user info
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Middleware function for validating JWT tokens
    pub async fn validate_token(
        State(jwt_service): State<Arc<JwtService>>,
        mut req: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // Extract Authorization header
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Check for Bearer token format
        if !auth_header.starts_with("Bearer ") {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let token = &auth_header[7..]; // Remove "Bearer " prefix

        // Validate the token
        let claims = jwt_service
            .validate_token(token)
            .map_err(|_| StatusCode::UNAUTHORIZED)?
            .claims;

        // Create AuthUser from claims
        let auth_user = AuthUser {
            id: claims.sub,
            email: claims.email,
        };

        // Insert the user into request extensions for downstream handlers
        req.extensions_mut().insert(auth_user);

        Ok(next.run(req).await)
    }

    /// Optional authentication - doesn't fail if no token is provided
    pub async fn optional_auth(
        State(jwt_service): State<Arc<JwtService>>,
        mut req: Request,
        next: Next,
    ) -> Response {
        // Try to extract and validate token, but don't fail if missing
        if let Some(auth_header) = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
        {
            if auth_header.starts_with("Bearer ") {
                let token = &auth_header[7..];
                
                if let Ok(token_data) = jwt_service.validate_token(token) {
                    let auth_user = AuthUser {
                        id: token_data.claims.sub,
                        email: token_data.claims.email,
                    };
                    req.extensions_mut().insert(auth_user);
                }
            }
        }

        next.run(req).await
    }
}

/// Extension trait for extracting AuthUser from request
pub trait RequestAuthExt {
    fn auth_user(&self) -> Option<&AuthUser>;
    fn require_auth(&self) -> Result<&AuthUser, StatusCode>;
}

impl RequestAuthExt for Request {
    fn auth_user(&self) -> Option<&AuthUser> {
        self.extensions().get::<AuthUser>()
    }

    fn require_auth(&self) -> Result<&AuthUser, StatusCode> {
        self.auth_user().ok_or(StatusCode::UNAUTHORIZED)
    }
}
