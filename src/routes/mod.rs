pub mod auth;
// # Routes Module
//
// - This module contains all HTTP route handlers for the ICM Server.
// - Routes are organized by functionality into separate submodules.
//
//  ## Available Route Modules
// - `health`: Health check and monitoring endpoints
// - `icm`: ICM program transaction endpoints
// - `agent`: AI-powered trading agent endpoints
//
// - ## Adding New Routes
// - To add new route modules:
// - 1. Create a new file in the `routes/` directory
// - 2. Add the module declaration here with `pub mod module_name;`
// - 3. Register the routes in `server.rs` using the Router
//
// ## Route Organization Best Practices
// - Group related endpoints in the same module
// - Use descriptive module names that reflect the API domain
// - Keep route handlers focused and single-purpose
// - Document all public route handler functions

/// Health check and monitoring endpoints
pub mod health;

/// ICM program transaction endpoints
pub mod icm;

/// AI-powered trading agent endpoints
pub mod agent;
