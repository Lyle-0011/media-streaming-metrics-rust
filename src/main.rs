mod streaming_metrics;

use serde_json::{Map, Value};
use streaming_metrics::{delivery_decision, DeliveryInput, InfraiClient, MetricReport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = InfraiClient::from_env()?;
    let asset = DeliveryInput { asset_id: "episode-184".into(), processed_seconds: 3600, delivered_seconds: 3600, creator_id: "creator-17".into() };
    let decision = delivery_decision(&asset);
    let mut labels = Map::new();
    labels.insert("asset_id".into(), Value::String(asset.asset_id.clone()));
    labels.insert("creator_id".into(), Value::String(asset.creator_id.clone()));
    labels.insert("decision".into(), Value::String(format!("{decision:?}")));
    client.report(&MetricReport { name: "creator_delivery_seconds".into(), value: asset.delivered_seconds as f64, metric_type: "gauge".into(), labels }).await?;
    let _ = client.query("creator_delivery_seconds", "avg").await?;
    let _ = client.usage_timeseries().await?;
    println!("{decision:?}: business metric and account usage share one Infrai key and base URL");
    Ok(())
}
