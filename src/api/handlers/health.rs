// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Health check and about handlers

use axum::Json;
use serde::Serialize;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// About response with license and source information
#[derive(Serialize)]
pub struct AboutResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub license: &'static str,
    pub source: &'static str,
}

/// GET /api/healthz
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("MOCKSERVER_VERSION"),
    })
}

/// GET /api/about
pub async fn about() -> Json<AboutResponse> {
    Json(AboutResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("MOCKSERVER_VERSION"),
        license: env!("CARGO_PKG_LICENSE"),
        source: env!("CARGO_PKG_REPOSITORY"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let response = health_check().await;
        assert_eq!(response.status, "ok");
    }

    #[tokio::test]
    async fn test_health_check_returns_version() {
        let response = health_check().await;
        assert_eq!(response.version, env!("MOCKSERVER_VERSION"));
    }

    #[tokio::test]
    async fn test_about_returns_license() {
        let response = about().await;
        assert_eq!(response.license, env!("CARGO_PKG_LICENSE"));
    }

    #[tokio::test]
    async fn test_about_returns_name() {
        let response = about().await;
        assert_eq!(response.name, "mockserver");
    }
}
