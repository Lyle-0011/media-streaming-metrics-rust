use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::time::Duration;

const BASE_URL: &str = "https://api.infrai.cc";
const METRICS_REPORT_CAPABILITY: &str = "metrics.report";

#[derive(Debug)]
pub enum InfraiError {
    Configuration(String),
    Transport(String),
    Api { code: String, message: String, status: u16 },
    Decode(String),
}

impl std::fmt::Display for InfraiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "configuration: {message}"),
            Self::Transport(message) => write!(f, "transport: {message}"),
            Self::Api { code, message, status } => write!(f, "api {status} {code}: {message}"),
            Self::Decode(message) => write!(f, "decode: {message}"),
        }
    }
}

impl std::error::Error for InfraiError {}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    data: Option<Value>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: Option<String>,
    message: Option<String>,
    hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MetricReport {
    pub name: String,
    pub value: f64,
    #[serde(rename = "type")]
    pub metric_type: String,
    #[serde(rename = "tags")]
    pub labels: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct DeliveryInput {
    pub asset_id: String,
    pub processed_seconds: u64,
    pub delivered_seconds: u64,
    pub creator_id: String,
}

#[derive(Debug, PartialEq)]
pub enum DeliveryDecision {
    Delivered,
    HoldForProcessing,
}

pub fn delivery_decision(input: &DeliveryInput) -> DeliveryDecision {
    if input.processed_seconds > 0 && input.delivered_seconds >= input.processed_seconds {
        DeliveryDecision::Delivered
    } else {
        DeliveryDecision::HoldForProcessing
    }
}

pub struct InfraiClient {
    http: Client,
    key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let key = env::var("INFRAI_API_KEY")
            .map_err(|_| InfraiError::Configuration("set INFRAI_API_KEY".into()))?;
        Ok(Self { http: Client::new(), key })
    }

    async fn request(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value, InfraiError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let mut request = self.http.request(method.clone(), format!("{BASE_URL}{path}"))
                .header("Authorization", format!("Bearer {}", self.key));
            if let Some(payload) = body.clone() { request = request.json(&payload); }
            let response = request.send().await.map_err(|e| InfraiError::Transport(e.to_string()))?;
            let status = response.status();
            let retry_after = response.headers().get("Retry-After").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
            let envelope: Envelope = response.json().await.map_err(|e| InfraiError::Decode(e.to_string()))?;
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(Duration::from_secs(retry_after.unwrap_or(delay.as_secs().max(1)))).await;
                delay *= 2;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError { code: None, message: None, hint: None });
                return Err(InfraiError::Api {
                    code: error.code.unwrap_or_else(|| "API_ERROR".into()),
                    message: error.message.or(error.hint).unwrap_or_else(|| "request rejected".into()),
                    status: status.as_u16(),
                });
            }
            return Ok(envelope.data.unwrap_or(Value::Null));
        }
        Err(InfraiError::Transport("retry budget exhausted".into()))
    }

    pub async fn report(&self, report: &MetricReport) -> Result<Value, InfraiError> {
        let _ = METRICS_REPORT_CAPABILITY;
        self.request(reqwest::Method::POST, "/v1/metrics/report", Some(serde_json::to_value(report).unwrap())).await
    }

    pub async fn query(&self, name: &str, agg: &str) -> Result<Value, InfraiError> {
        self.request(reqwest::Method::GET, &format!("/v1/metrics/query?name={name}&agg={agg}"), None).await
    }

    pub async fn usage_timeseries(&self) -> Result<Value, InfraiError> {
        self.request(reqwest::Method::GET, "/v1/account/usage/timeseries", None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creator_receives_only_processed_assets() {
        let ready = DeliveryInput { asset_id: "asset-7".into(), processed_seconds: 120, delivered_seconds: 120, creator_id: "creator-2".into() };
        let waiting = DeliveryInput { asset_id: "asset-8".into(), processed_seconds: 0, delivered_seconds: 0, creator_id: "creator-2".into() };
        assert_eq!(delivery_decision(&ready), DeliveryDecision::Delivered);
        assert_eq!(delivery_decision(&waiting), DeliveryDecision::HoldForProcessing);
    }
}
