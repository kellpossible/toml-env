#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// A source of configuration.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Two configuration sources merged together.
    Merged {
        /// Merged from.
        from: Box<Self>,
        /// Merged into.
        into: Box<Self>,
    },
    /// A `.toml.env` file (Path may be different if user has specified something different).
    DotEnv(PathBuf),
    /// A configuration file.
    File(PathBuf),
    /// An environment variable
    Environment {
        /// The name of the environment variable.
        variable_name: String,
    },
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Merged { from, into } => write!(f, "{from} merged into {into}"),
            ConfigSource::DotEnv(path) => write!(f, "dotenv TOML file {path:?}"),
            ConfigSource::File(path) => write!(f, "config TOML file {path:?}"),
            ConfigSource::Environment { variable_name } => {
                write!(f, "environment variable {variable_name}")
            }
        }
    }
}

/// An error that occurs while initializing configuration.
#[derive(Debug, Error)]
pub enum Error {
    /// Error reading environment variable.
    #[error("Error reading {name} environment variable")]
    ErrorReadingEnvironmentVariable {
        /// Name of the environment variable.
        name: String,
        /// Source of the error.
        #[source]
        error: std::env::VarError,
    },
    /// Error reading TOML file.
    #[error("Error reading TOML file {path:?}")]
    ErrorReadingFile {
        /// Path to the file.
        path: PathBuf,
        /// Source of the error.
        #[source]
        error: std::io::Error,
    },
    /// Error parsing TOML file.
    #[error("Error parsing TOML file {path:?}")]
    ErrorParsingTomlFile {
        /// Path to the file.
        path: PathBuf,
        /// Source of the error.
        #[source]
        error: toml::de::Error,
    },
    /// Cannot parse a table in the `.toml.env` file.
    #[error("Cannot parse {key} as environment variable in {path:?}. Advice: {advice}")]
    CannotParseTomlDotEnvFile {
        /// Key in the TOML file.
        key: String,
        /// Path to the file.
        path: PathBuf,
        /// Advice
        advice: String,
    },
    /// Error parsing envirnment variable
    #[error("Error parsing config key ({name}) in TOML config file {path:?}")]
    ErrorParsingTomlDotEnvFileKey {
        /// Name of the key variable.
        name: String,
        /// Path to the file.
        path: PathBuf,
        /// Source of the error.
        #[source]
        error: toml::de::Error,
    },
    /// Error parsing an environment variable as the config.
    #[error("Error parsing environment variable ({name}={value:?}) as the config.")]
    ErrorParsingEnvironmentVariableAsConfig {
        /// Name of the environment variable.
        name: String,
        /// Value of the environment variable.
        value: String,
        /// Source of the error.
        #[source]
        error: toml::de::Error,
    },
    /// Either there was an error parsing the environment variable as the config, or if the value
    /// is a filename, it does not exist.
    #[error(
        "Error parsing config environment variable ({name}={value:?}) as the config or if it is a filename, the file does not exist."
    )]
    ErrorParsingEnvironmentVariableAsConfigOrFile {
        /// Name of the environment variable.
        name: String,
        /// Value of the environment variable.
        value: String,
        /// Source of the error.
        #[source]
        error: toml::de::Error,
    },
    /// Error parsing file as `.env.toml` format.
    #[error(
        "Error parsing the {path:?} as `.env.toml` format file:\n{value:#?}\nTop level should be a table."
    )]
    UnexpectedTomlDotEnvFileFormat {
        /// Path to file.
        path: PathBuf,
        /// Value that was unable to be parsed as `.env.toml` format
        value: toml::Value,
    },
    /// Error parsing merged configuration.
    #[error("Error parsing merged configuration from {source}")]
    ErrorParsingMergedToml {
        /// Source(s) of the configuration.
        source: ConfigSource,
        /// Source of the error.
        #[source]
        error: toml::de::Error,
    },
    /// Error merging configurations.
    #[error("Error merging configuration {from} into {into}: {error}")]
    ErrorMerging {
        /// Error merging from this source.
        from: ConfigSource,
        /// Error merging into this source.
        into: ConfigSource,
        /// Source of the error.
        error: serde_toml_merge::Error,
    },
}

/// Convenience type shorthand for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Default name for attempting to load the configuration (and environment variables) from a file.
pub const DEFAULT_DOTENV_PATH: &str = ".env.toml";

/// Default environment variable name to use for loading configuration from. Also the same name
/// used for the table of the configuration within the `.env.toml`.
pub const DEFAULT_CONFIG_VARIABLE_NAME: &str = "CONFIG";

/// What method of logging for this library to use.
#[derive(Default, Clone, Copy)]
pub enum Logging {
    /// Don't perform any logging
    #[default]
    None,
    /// Use STDOUT for logging.
    StdOut,
    /// Use the [`log`] crate for logging.
    #[cfg(feature = "log")]
    Log,
}

/// Args as input to [`initialize()`].
pub struct Args<'a> {
    /// Path to `.env.toml` format file. The value is [`DEFAULT_DOTENV_PATH`] by default.
    pub dotenv_path: &'a Path,
    /// Path to a config file to load.
    pub config_path: Option<&'a Path>,
    /// Name of the environment variable to use that stores the config. The value is [`DEFAULT_CONFIG_VARIABLE_NAME`] by default.
    pub config_variable_name: &'a str,
    /// What method of logging to use (if any). [`Logging::None`] by default.
    pub logging: Logging,
}

impl Default for Args<'static> {
    fn default() -> Self {
        Self {
            dotenv_path: Path::new(DEFAULT_DOTENV_PATH),
            config_path: None,
            config_variable_name: DEFAULT_CONFIG_VARIABLE_NAME,
            logging: Logging::default(),
        }
    }
}

fn log_info(logging: Logging, args: std::fmt::Arguments<'_>) {
    match logging {
        Logging::None => {}
        Logging::StdOut => println!("INFO {}: {}", module_path!(), std::fmt::format(args)),
        #[cfg(feature = "log")]
        Logging::Log => log::info!("{}", std::fmt::format(args)),
    }
}

/// Reads and parses the .env.toml file (or whatever is specified in `dotenv_path`). Returns
/// `Some(C)` if the file contains a table with the name matching `config_variable_name`.
fn initialize_dotenv_toml<C: DeserializeOwned + Serialize>(
    Args {
        dotenv_path,
        config_variable_name,
        logging,
        ..
    }: &Args,
) -> Result<Option<C>> {
    let path = Path::new(dotenv_path);
    if !path.exists() {
        return Ok(None);
    }

    log_info(
        *logging,
        format_args!("Loading environment variables from {path:?}"),
    );

    let env_str = std::fs::read_to_string(path).map_err(|error| Error::ErrorReadingFile {
        path: path.to_owned(),
        error,
    })?;
    let env: toml::Value =
        toml::from_str(&env_str).map_err(|error| Error::ErrorParsingTomlFile {
            path: path.to_owned(),
            error,
        })?;
    let table: toml::value::Table = match env {
        toml::Value::Table(table) => table,
        unexpected => {
            return Err(Error::UnexpectedTomlDotEnvFileFormat {
                path: path.to_owned(),
                value: unexpected,
            });
        }
    };

    let mut config: Option<C> = None;
    for (key, value) in table {
        let value_string = match value {
            toml::Value::Table(_) => {
                if key.as_str() != *config_variable_name {
                    return Err(Error::CannotParseTomlDotEnvFile {
                        key,
                        path: path.to_owned(),
                        advice: format!("Only a table with {config_variable_name} is allowed in a .toml.env format file."),
                    });
                }
                match C::deserialize(value.clone()) {
                    Ok(c) => config = Some(c),
                    Err(error) => {
                        return Err(Error::ErrorParsingTomlDotEnvFileKey {
                            name: key,
                            path: path.to_owned(),
                            error,
                        })
                    }
                }
                None
            }
            toml::Value::String(value) => Some(value),
            toml::Value::Integer(value) => Some(value.to_string()),
            toml::Value::Float(value) => Some(value.to_string()),
            toml::Value::Boolean(value) => Some(value.to_string()),
            toml::Value::Datetime(value) => Some(value.to_string()),
            toml::Value::Array(value) => {
                return Err(Error::CannotParseTomlDotEnvFile {
                    key,
                    path: path.to_owned(),
                    advice: format!("Array values are not supported: {value:?}"),
                })
            }
        };

        if let Some(value_string) = value_string {
            std::env::set_var(key.as_str(), value_string)
        }
    }
    Ok(config)
}

/// Initialize configuration from available sources specified in [`Args`].
///
/// If no configuration was found, will return `None`.
///
/// See [`toml-env`](crate).
pub fn initialize<C: DeserializeOwned + Serialize>(args: Args<'_>) -> Result<Option<C>> {
    let config_variable_name = args.config_variable_name;
    let logging = args.logging;

    let env_config: Option<(toml::Value, ConfigSource)> = match std::env::var(config_variable_name) {
        Ok(variable_value) => match toml::from_str(&variable_value) {
            Ok(config) => {
                log_info(
                    logging,
                    format_args!(
                        "Options loaded from `{config_variable_name}` environment variable"
                    ),
                );
                Ok(Some(config))
            }
            Err(error) => {
                let path = Path::new(&variable_value);
                if path.is_file() {
                    log_info(
                        args.logging,
                        format_args!("Loading environment variables from {path:?}"),
                    );

                    let config_str =
                        std::fs::read_to_string(path).map_err(|error| Error::ErrorReadingFile {
                            path: path.to_owned(),
                            error,
                        })?;
                    let config: toml::Value = toml::from_str(&config_str).map_err(|error| {
                        Error::ErrorParsingTomlFile {
                            path: path.to_owned(),
                            error,
                        }
                    })?;
                    log_info(logging, format_args!("Options loaded from file specified in `{config_variable_name}` environment variable: {path:?}"));
                    Ok(Some(config))
                } else {
                    Err(Error::ErrorParsingEnvironmentVariableAsConfigOrFile {
                        name: config_variable_name.to_owned(),
                        value: variable_value,
                        error,
                    })
                }
            }
        },
        Err(std::env::VarError::NotPresent) => {
            log_info(
                logging,
                format_args!(
                    "No environment variable with the name {config_variable_name} found, using default options."
                ),
            );
            Ok(None)
        }
        Err(error) => Err(Error::ErrorReadingEnvironmentVariable {
            name: config_variable_name.to_owned(),
            error,
        }),
    }?.map(|config| {
        let source = ConfigSource::DotEnv(args.dotenv_path.to_owned());
        (config, source)
    });

    let dotenv_config = initialize_dotenv_toml(&args)?.map(|config| {
        (
            config,
            ConfigSource::Environment {
                variable_name: args.config_variable_name.to_owned(),
            },
        )
    });

    let env_and_dotenv_config: Option<(toml::Value, ConfigSource)> =
        match (dotenv_config, env_config) {
            (None, None) => None,
            (None, Some(config)) => Some(config),
            (Some(config), None) => Some(config),
            (Some(from), Some(into)) => {
                let config = serde_toml_merge::merge(into.0, from.0).map_err(|error| {
                    Error::ErrorMerging {
                        from: from.1.clone(),
                        into: into.1.clone(),
                        error,
                    }
                })?;

                let source = ConfigSource::Merged {
                    from: from.1.into(),
                    into: into.1.into(),
                };

                Some((config, source))
            }
        };

    let file_config: Option<(toml::Value, ConfigSource)> =
        Option::transpose(args.config_path.map(|path| {
            if path.is_file() {
                let file_string =
                    std::fs::read_to_string(path).map_err(|error| Error::ErrorReadingFile {
                        path: path.to_owned(),
                        error,
                    })?;
                return Ok(Some((
                    toml::from_str(&file_string).map_err(|error| Error::ErrorParsingTomlFile {
                        path: path.to_owned(),
                        error,
                    })?,
                    ConfigSource::File(path.to_owned()),
                )));
            }
            Ok(None)
        }))?
        .flatten();

    let config = match (env_and_dotenv_config, file_config) {
        (None, None) => None,
        (None, Some(config)) => Some(config),
        (Some(config), None) => Some(config),
        (Some(from), Some(into)) => {
            let config =
                serde_toml_merge::merge(into.0, from.0).map_err(|error| Error::ErrorMerging {
                    from: from.1.clone(),
                    into: into.1.clone(),
                    error,
                })?;

            let source = ConfigSource::Merged {
                from: from.1.into(),
                into: into.1.into(),
            };
            Some((config, source))
        }
    };

    let config = Option::transpose(config.map(|(config, source)| {
        C::deserialize(config).map_err(|error| Error::ErrorParsingMergedToml { source, error })
    }))?;

    if !matches!((logging, &config), (Logging::None, None)) {
        let config_string = toml::to_string_pretty(&config)
            .expect("Expected to be able to re-serialize config toml");
        log_info(
            logging,
            format_args!("{config_variable_name}:\n\x1b[34m{config_string}\x1b[0m"),
        );
    }

    Ok(config)
}
