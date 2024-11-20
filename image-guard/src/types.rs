use serde::Deserialize;

#[derive(Deserialize)]
pub struct Env {
    pub api_port: u16,
}
