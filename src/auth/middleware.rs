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
        tracing::info!("[AuthMiddleware] Incoming request: {} {}", req.method(), req.uri());
        // Try to extract token from Authorization header (Bearer) or access_token cookie
        let token_opt = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .and_then(|auth_header| {
                if auth_header.starts_with("Bearer ") {
                    Some(auth_header[7..].to_string())
                } else {
                    None
                }
            })
            .or_else(|| {
                // Fallback: try to get from cookie
                req.headers()
                    .get(header::COOKIE)
                    .and_then(|cookie_header| cookie_header.to_str().ok())
                    .and_then(|cookie_str| {
                        // Parse cookies
                        for cookie in cookie_str.split(';') {
                            let cookie = cookie.trim();
                            if let Some(rest) = cookie.strip_prefix("access_token=") {
                                return Some(rest.to_string());
                            }
                        }
                        None
                    })
            });

        let token = match token_opt {
            Some(token) => token,
            None => {
                tracing::warn!("[AuthMiddleware] Missing Authorization header and access_token cookie");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };
        tracing::info!("[AuthMiddleware] Extracted token: {}", token);

        // Validate the token
        let claims = match jwt_service.validate_token(&token) {
            Ok(data) => {
                tracing::info!("[AuthMiddleware] JWT validated successfully for sub={}, email={}", data.claims.sub, data.claims.email);
                data.claims
            },
            Err(e) => {
                tracing::warn!("[AuthMiddleware] JWT validation failed: {:?}", e);
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

        // Create AuthUser from claims
        let auth_user = AuthUser {
            id: claims.sub,
            email: claims.email,
        };
        tracing::info!("[AuthMiddleware] AuthUser injected: id={}, email={}", auth_user.id, auth_user.email);

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
