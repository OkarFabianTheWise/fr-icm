use axum::response::Json;
use serde_json::json;

/// Health check endpoint handler.
/// 
/// This function provides a simple health check endpoint that returns a JSON response
/// indicating the server is operational. This is commonly used by load balancers,
/// monitoring systems, and container orchestrators to verify service availability.
/// 
/// # Route
/// - **Method**: GET
/// - **Path**: `/ping`
/// - **Response**: JSON object with status field
/// 
/// # Response Format
/// Returns a JSON object with the following structure:
/// ```json
/// {
///   "status": "pong"
/// }
/// ```
/// 
/// # HTTP Status Codes
/// - **200 OK**: Server is healthy and operational
/// 
/// # Examples
/// ```bash
/// curl http://localhost:3000/ping
/// # Response: {"status":"pong"}
/// ```
/// 
/// # Usage in Monitoring
/// This endpoint can be used for:
/// - Load balancer health checks
/// - Docker/Kubernetes liveness/readiness probes
/// - Uptime monitoring services
/// - CI/CD pipeline health verification
/// 
/// # Performance
/// This endpoint is designed to be lightweight and fast, with minimal processing
/// overhead to provide quick health status responses.
pub async fn ping() -> Json<serde_json::Value> {
    // Return a simple JSON response indicating the server is alive
    Json(json!({ "status": "pong" }))
}
