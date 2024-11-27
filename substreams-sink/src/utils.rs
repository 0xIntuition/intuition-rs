use crate::{error::SubstreamError, pb::sf::substreams::v1::Package};
use anyhow::{format_err, Context};
use prost::Message;
use std::env;

pub fn read_block_range(pkg: &Package, module_name: &str) -> Result<(i64, u64), SubstreamError> {
    let module = pkg
        .modules
        .as_ref()
        .unwrap()
        .modules
        .iter()
        .find(|m| m.name == module_name)
        .ok_or_else(|| format_err!("module '{}' not found in package", module_name))?;

    let mut input: String = "".to_string();
    if let Some(range) = env::args().nth(4) {
        input = range;
    };

    let (prefix, suffix) = match input.split_once(":") {
        Some((prefix, suffix)) => (prefix.to_string(), suffix.to_string()),
        None => ("".to_string(), input),
    };

    let start: i64 = match prefix.as_str() {
        "" => module.initial_block as i64,
        x if x.starts_with("+") => {
            let block_count = x
                .trim_start_matches("+")
                .parse::<u64>()
                .context("argument <stop> is not a valid integer")?;

            (module.initial_block + block_count) as i64
        }
        x => x
            .parse::<i64>()
            .context("argument <start> is not a valid integer")?,
    };

    let stop: u64 = match suffix.as_str() {
        "" => 0,
        "-" => 0,
        x if x.starts_with("+") => {
            let block_count = x
                .trim_start_matches("+")
                .parse::<u64>()
                .context("argument <stop> is not a valid integer")?;

            start as u64 + block_count
        }
        x => x
            .parse::<u64>()
            .context("argument <stop> is not a valid integer")?,
    };

    Ok((start, stop))
}

pub async fn read_package(input: &str) -> Result<Package, SubstreamError> {
    if input.starts_with("http") {
        return read_http_package(input).await;
    }

    // Assume it's a local file
    let content =
        std::fs::read(input).context(format_err!("read package from file '{}'", input))?;
    Package::decode(content.as_ref())
        .context("decode command")
        .map_err(SubstreamError::from)
}

pub async fn read_http_package(input: &str) -> Result<Package, SubstreamError> {
    let body = reqwest::get(input).await?.bytes().await?;

    Package::decode(body)
        .context("decode command")
        .map_err(SubstreamError::from)
}
