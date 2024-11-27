use crate::{
    app::App,
    error::SubstreamError,
    pb::sf::substreams::v1::{
        module::{
            input::{Input as InputEnum, Map, Params},
            Input,
        },
        Modules, Package,
    },
    substreams::SubstreamsEndpoint,
    utils::{read_block_range, read_package},
    Cli,
};
use std::sync::Arc;

/// A struct that prepares the endpoint and package for the substreams stream.
#[derive(Clone, Debug)]
pub struct PreparedEndpointAndPackage {
    pub endpoint: Arc<SubstreamsEndpoint>,
    pub package: Package,
    pub block_range: (i64, u64),
}

impl PreparedEndpointAndPackage {
    pub async fn new(cli: &Cli, app: &App) -> Result<PreparedEndpointAndPackage, SubstreamError> {
        let package = read_package(&cli.spkg).await?;
        let block_range = read_block_range(&package, &cli.module)?;
        Ok(PreparedEndpointAndPackage {
            endpoint: Arc::new(
                SubstreamsEndpoint::new(&cli.endpoint, Some(app.env.substreams_api_token.clone()))
                    .await?,
            ),
            package,
            block_range,
        })
    }

    /// Prepare the modules for the substreams stream.
    pub async fn mutable_modules(&self) -> Result<Modules, SubstreamError> {
        let mut mutable_modules = self.package.modules.as_ref().unwrap().clone();
        mutable_modules.modules.iter_mut().for_each(|module| {
            if module.name == "filtered_events" {
                module.inputs.clear();
                module.inputs.push(Input {
                    input: Some(InputEnum::Params(Params {
                        value: "evt_addr:0x430bbf52503bd4801e51182f4cb9f8f534225de5".to_string(),
                    })),
                });
                module.inputs.push(Input {
                    input: Some(InputEnum::Map(Map {
                        module_name: "all_events".to_string(),
                    })),
                });
            }
        });

        Ok(mutable_modules)
    }
}
