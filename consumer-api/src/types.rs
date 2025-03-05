use serde::Deserialize;

#[derive(Deserialize)]
pub struct Env {
    pub consumer_api_port: Option<u16>,
    pub resolver_queue_url: String,
    pub localstack_url: Option<String>,
}
