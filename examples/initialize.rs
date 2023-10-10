use std::{path::Path, str::FromStr};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use toml_env::{self, Args, AutoMapEnvArgs, TomlKeyPath};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    value_1: String,
    value_2: String,
    child: Child,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Child {
    value_1: String,
    value_2: String,
    value_3: String,
}

pub fn main() -> anyhow::Result<()> {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples"));
    std::env::set_current_dir(path)?;
    let Config {
        value_1,
        value_2,
        child,
    } = toml_env::initialize(Args {
        logging: toml_env::Logging::StdOut,
        config_path: Some("config.toml".as_ref()),
        map_env: [
            ("ENV_VAR", "child.value_1"),
            ("MY_APP__CHILD__VALUE_7", "child.value_3"),
        ]
        .into_iter()
        .map(|(name, key)| (name, TomlKeyPath::from_str(key).unwrap()))
        .collect(),
        auto_map_env: Some(AutoMapEnvArgs {
            prefix: Some("MY_APP"),
            ..AutoMapEnvArgs::default()
        }),
        ..Args::default()
    })?
    .context("Config is missing")?;

    let env_var = std::env::var("ENV_VAR").unwrap();

    println!(
        "{value_1} | {value_2} | {env_var} | {} | {} | {}",
        child.value_1, child.value_2, child.value_3
    );
    Ok(())
}
