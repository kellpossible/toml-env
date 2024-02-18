#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![deny(missing_docs)]

// NOTE: This crate intentionally uses a single module in order to put pressure on keeping the
// feature list small.

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use toml::Value;

/// Convenience type shorthand for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Default name for attempting to load the configuration (and environment variables) from a file.
pub const DEFAULT_DOTENV_PATH: &str = ".env.toml";

/// Default environment variable name to use for loading configuration from. Also the same name
/// used for the table of the configuration within the `.env.toml`.
pub const DEFAULT_CONFIG_VARIABLE_NAME: &str = "CONFIG";

/// The default divider between different levels of parent.child in environment variable names.
/// This will be replaced with a `.` for the [`TomlKeyPath`].
pub const DEFAULT_MAP_ENV_DIVIDER: &str = "__";

/// A source of configuration.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// From two configuration sources merged together.
    Merged {
        /// Merged from.
        from: Box<Self>,
        /// Merged into.
        into: Box<Self>,
    },
    /// From a `.toml.env` file (Path may be different if user has specified something different).
    DotEnv(PathBuf),
    /// From a configuration file.
    File(PathBuf),
    /// From environment variables.
    Environment {
        /// The names of the environment variables.
        variable_names: Vec<String>,
    },
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Merged { from, into } => write!(f, "({from}) merged into ({into})"),
            ConfigSource::DotEnv(path) => write!(f, "dotenv TOML file {path:?}"),
            ConfigSource::File(path) => write!(f, "config TOML file {path:?}"),
            ConfigSource::Environment { variable_names } => {
                let variable_names = variable_names.join(", ");
                write!(f, "environment variables {variable_names}")
            }
        }
    }
}

/// An error that occurs while initializing configuration.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct Error(#[from] InnerError);

/// An error that occurs while initializing configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
enum InnerError {
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
        error: Box<toml::de::Error>,
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
        error: Box<toml::de::Error>,
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
        error: Box<toml::de::Error>,
    },
    /// Error parsing file as `.env.toml` format.
    #[error(
        "Error parsing the {path:?} as `.env.toml` format file:\n{value:#?}\nTop level should be a table."
    )]
    UnexpectedTomlDotEnvFileFormat {
        /// Path to file.
        path: PathBuf,
        /// Value that was unable to be parsed as `.env.toml` format
        value: Value,
    },
    /// Error parsing merged configuration.
    #[error("Error parsing merged configuration from {source}")]
    ErrorParsingMergedToml {
        /// Source(s) of the configuration.
        source: ConfigSource,
        /// Source of the error.
        #[source]
        error: Box<toml::de::Error>,
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
    #[error("Error inserting toml value")]
    InsertTomlValueError(#[from] InsertTomlValueError),
}

/// What method of logging for this library to use.
#[derive(Default, Clone, Copy)]
pub enum Logging {
    /// Don't perform any logging
    #[default]
    None,
    /// Use STDOUT for logging. This may be attractive if you are relying on the output of this
    /// library to configure your logging system and still want to see what's going on here before
    /// the system is configured.
    StdOut,
    /// Use the [`log`] crate for logging.
    #[cfg(feature = "log")]
    Log,
}

type InnerResult<T> = std::result::Result<T, InnerError>;

/// A path to a key into a [`toml::Value`]. In the format of `key.0.key` (`0` for indexing into an
/// array) when parsed using [`FromStr`].
///
/// See [`TomlKeyPath::resolve()`] for an example.
#[derive(Debug, Clone, Default)]
pub struct TomlKeyPath(Vec<PathElement>);

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
enum PathElement {
    TableProperty(String),
    ArrayIndex(usize),
}

impl std::fmt::Display for PathElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathElement::TableProperty(p) => p.fmt(f),
            PathElement::ArrayIndex(i) => i.fmt(f),
        }
    }
}

impl TomlKeyPath {
    /// Resolve a value contained within a [`toml::Value`] using this [`TomlKeyPath`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use toml_env::TomlKeyPath;
    /// use toml;
    ///
    /// let toml_value = toml::from_str(r#"
    /// key="value1"
    /// array=["hello", "world"]
    /// [child]
    /// key="value2"
    /// "#).unwrap();
    ///
    /// let key1: TomlKeyPath = "key".parse().unwrap();
    /// let key1_value = key1.resolve(&toml_value)
    ///     .expect("Expected to resolve")
    ///     .as_str()
    ///     .expect("Expected to be a string");
    /// assert_eq!(key1_value, "value1");
    ///
    /// let key2: TomlKeyPath = "child.key".parse().unwrap();
    /// let key2_value = key2.resolve(&toml_value)
    ///     .expect("Expected to resolve")
    ///     .as_str()
    ///     .expect("Expected to be a string");
    /// assert_eq!(key2_value, "value2");
    ///
    /// let hello: TomlKeyPath = "array.0".parse().unwrap();
    /// let hello_value = hello.resolve(&toml_value)
    ///     .expect("Expected to resolve")
    ///     .as_str()
    ///     .expect("Expected to be a string");
    /// assert_eq!(hello_value, "hello");
    /// ```
    pub fn resolve<'a>(&self, value: &'a toml::Value) -> Option<&'a toml::Value> {
        Self::resolve_impl(&mut self.clone(), value)
    }

    fn resolve_impl<'a>(key: &mut Self, value: &'a toml::Value) -> Option<&'a toml::Value> {
        if key.0.is_empty() {
            return Some(value);
        }

        let current_key = key.0.remove(0);

        match value {
            Value::Table(table) => match current_key {
                PathElement::TableProperty(p) => {
                    let value = table.get(&p)?;
                    Self::resolve_impl(key, value)
                }
                PathElement::ArrayIndex(_) => None,
            },
            Value::Array(array) => match current_key {
                PathElement::ArrayIndex(i) => {
                    let value = array.get(i)?;
                    Self::resolve_impl(key, value)
                }
                PathElement::TableProperty(_) => None,
            },
            _ => None,
        }
    }
}

impl std::fmt::Display for TomlKeyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &self
                .0
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("."),
        )
    }
}

impl FromStr for TomlKeyPath {
    type Err = ();

    /// Parse a string into a [`TomlKeyPath`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use toml_env::TomlKeyPath;
    ///
    /// "key".parse::<TomlKeyPath>().unwrap();
    /// "key.0".parse::<TomlKeyPath>().unwrap();
    /// "key.key".parse::<TomlKeyPath>().unwrap();
    /// "key.0.key".parse::<TomlKeyPath>().unwrap();
    /// "key.key.key".parse::<TomlKeyPath>().unwrap();
    /// "".parse::<TomlKeyPath>().unwrap();
    /// ".".parse::<TomlKeyPath>().unwrap();
    /// ".key".parse::<TomlKeyPath>().unwrap();
    /// "key.".parse::<TomlKeyPath>().unwrap();
    /// ```

    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        if s.is_empty() {
            return Ok(Self::default());
        }

        let v: Vec<PathElement> = s
            .split('.')
            .filter_map(|k| {
                if k.is_empty() {
                    None
                } else {
                    if let Ok(i) = usize::from_str(k) {
                        Some(PathElement::ArrayIndex(i))
                    } else {
                        Some(PathElement::TableProperty(k.to_owned()))
                    }
                }
            })
            .collect();

        Ok(Self(v))
    }
}

/// Automatically map environment variables into config.
pub struct AutoMapEnvArgs<'a> {
    /// The divider that separates different levels of the parent.child relationship for the
    /// mapping. This will get replaced with `.` when converting the name of a variable to a [`TomlKeyPath`]. The default value is [`DEFAULT_DOTENV_PATH`].
    pub divider: &'a str,
    /// Prefix for environment variables to be mapped. By default this will be [`DEFAULT_CONFIG_VARIABLE_NAME`].
    pub prefix: Option<&'a str>,
    /// A transform operation to perform on the environment variable before parsing it. By default
    /// this transforms it to lowercase.
    pub transform: Box<dyn Fn(&str) -> String>,
}

impl Default for AutoMapEnvArgs<'_> {
    fn default() -> Self {
        Self {
            divider: DEFAULT_MAP_ENV_DIVIDER,
            prefix: None,
            transform: Box::new(|name| name.to_lowercase()),
        }
    }
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
    /// Map the specified environment variables into config keys.
    pub map_env: HashMap<&'a str, TomlKeyPath>,
    /// See [`AutoMapEnvArgs`].
    pub auto_map_env: Option<AutoMapEnvArgs<'a>>,
}

impl Default for Args<'static> {
    fn default() -> Self {
        Self {
            dotenv_path: Path::new(DEFAULT_DOTENV_PATH),
            config_path: None,
            config_variable_name: DEFAULT_CONFIG_VARIABLE_NAME,
            logging: Logging::default(),
            map_env: HashMap::default(),
            auto_map_env: None,
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
fn initialize_dotenv_toml<'a, C: DeserializeOwned + Serialize>(
    dotenv_path: &'a Path,
    config_variable_name: &'a str,
    logging: Logging,
) -> InnerResult<Option<C>> {
    let path = Path::new(dotenv_path);
    if !path.exists() {
        return Ok(None);
    }

    log_info(
        logging,
        format_args!("Loading config and environment variables from dotenv {path:?}"),
    );

    let env_str = std::fs::read_to_string(path).map_err(|error| InnerError::ErrorReadingFile {
        path: path.to_owned(),
        error,
    })?;
    let env: Value =
        toml::from_str(&env_str).map_err(|error| InnerError::ErrorParsingTomlFile {
            path: path.to_owned(),
            error: error.into(),
        })?;
    let table: toml::value::Table = match env {
        Value::Table(table) => table,
        unexpected => {
            return Err(InnerError::UnexpectedTomlDotEnvFileFormat {
                path: path.to_owned(),
                value: unexpected,
            });
        }
    };

    if table.is_empty() {
        return Ok(None);
    }

    let mut config: Option<C> = None;
    let mut set_keys: String = String::new();
    for (key, value) in table {
        let value_string = match value {
            Value::Table(_) => {
                if key.as_str() != config_variable_name {
                    return Err(InnerError::CannotParseTomlDotEnvFile {
                        key,
                        path: path.to_owned(),
                        advice: format!("Only a table with {config_variable_name} is allowed in a .toml.env format file."),
                    });
                }
                match C::deserialize(value.clone()) {
                    Ok(c) => config = Some(c),
                    Err(error) => {
                        return Err(InnerError::ErrorParsingTomlDotEnvFileKey {
                            name: key,
                            path: path.to_owned(),
                            error: error.into(),
                        })
                    }
                }
                None
            }
            Value::String(value) => Some(value),
            Value::Integer(value) => Some(value.to_string()),
            Value::Float(value) => Some(value.to_string()),
            Value::Boolean(value) => Some(value.to_string()),
            Value::Datetime(value) => Some(value.to_string()),
            Value::Array(value) => {
                return Err(InnerError::CannotParseTomlDotEnvFile {
                    key,
                    path: path.to_owned(),
                    advice: format!("Array values are not supported: {value:?}"),
                })
            }
        };

        if let Some(value_string) = value_string {
            set_keys.push('\n');
            set_keys.push_str(key.as_str());
            std::env::set_var(key.as_str(), value_string)
        }
    }

    log_info(
        logging,
        format_args!(
            "Set environment variables specified in {dotenv_path:?}:\x1b[34m{set_keys}\x1b[0m"
        ),
    );
    Ok(config)
}

#[derive(Debug, thiserror::Error)]
enum InsertTomlValueError {
    #[error("Table property {property:?} can only be used to index into a table. Cannot index into {value:?}")]
    TablePropertyCannotIndex {
        property: String,
        value: toml::Value,
    },
    #[error(
        "Array index {index} can only be used to index into an array. Cannot index into {value:?}"
    )]
    ArrayIndexCannotIndex { index: usize, value: toml::Value },
    #[error("Array index {index} cannot be greater than the length of {array:?}")]
    ArrayOutOfBounds {
        index: usize,
        array: Vec<toml::Value>,
    },
}

/// Insert a `new_value` into a `value` at the location specified by `path`, creating any required
/// tables or arrays if they are missing. If the `path` is empty, it will replace the value
/// entirely.
fn insert_toml_value(
    value: &mut toml::Value,
    mut path: TomlKeyPath,
    new_value: Value,
) -> std::result::Result<(), InsertTomlValueError> {
    if path.0.is_empty() {
        *value = new_value;
        return Ok(());
    }

    let current_key = path.0.remove(0);
    let next_key = path.0.get(0);

    match (current_key, value) {
        (PathElement::TableProperty(property), Value::Table(table)) => {
            let next_value = table.get_mut(&property);
            match (next_value, next_key) {
                (None, None) => {
                    table.insert(property, new_value);
                    return Ok(());
                }
                (None, Some(PathElement::ArrayIndex(_))) => {
                    table.insert(property.clone(), toml::Value::Array(Vec::with_capacity(1)));
                    return insert_toml_value(
                        table
                            .get_mut(&property)
                            .expect("Expect inserted property to be present"),
                        path,
                        new_value,
                    );
                }
                (None, Some(PathElement::TableProperty(_))) => {
                    table.insert(
                        property.clone(),
                        toml::Value::Table(toml::Table::with_capacity(1)),
                    );
                    return insert_toml_value(
                        table
                            .get_mut(&property)
                            .expect("Expect inserted property to be present"),
                        path,
                        new_value,
                    );
                }
                (Some(next_value), None) => {
                    *next_value = new_value;
                    return Ok(());
                }
                (Some(next_value), Some(_)) => {
                    return insert_toml_value(next_value, path, new_value)
                }
            }
        }
        (PathElement::TableProperty(property), value) => {
            return Err(InsertTomlValueError::TablePropertyCannotIndex {
                property,
                value: value.clone(),
            })
        }
        (PathElement::ArrayIndex(index), Value::Array(array)) => {
            if index > array.len() {
                return Err(InsertTomlValueError::ArrayOutOfBounds {
                    index,
                    array: array.clone(),
                });
            }
            let next_value = array.get_mut(index);
            match (next_value, next_key) {
                (None, None) => {
                    array.insert(index, new_value);
                    return Ok(());
                }
                (None, Some(PathElement::ArrayIndex(_))) => {
                    array.insert(index, toml::Value::Array(Vec::with_capacity(1)));
                    return insert_toml_value(
                        array
                            .get_mut(index)
                            .expect("Expect inserted element to be present"),
                        path,
                        new_value,
                    );
                }
                (None, Some(PathElement::TableProperty(_))) => {
                    array.insert(index, toml::Value::Table(toml::Table::with_capacity(1)));
                    return insert_toml_value(
                        array
                            .get_mut(index)
                            .expect("Expect inserted element to be present"),
                        path,
                        new_value,
                    );
                }
                (Some(next_value), None) => {
                    *next_value = new_value;
                    return Ok(());
                }
                (Some(next_value), Some(_)) => {
                    return insert_toml_value(next_value, path, new_value)
                }
            }
        }
        (PathElement::ArrayIndex(index), value) => {
            Err(InsertTomlValueError::ArrayIndexCannotIndex {
                index,
                value: value.clone(),
            })
        }
    }
}

/// Initialize from environment variables.
fn initialize_env(
    logging: Logging,
    map_env: HashMap<&'_ str, TomlKeyPath>,
    auto_args: Option<AutoMapEnvArgs<'_>>,
    config_variable_name: &'_ str,
) -> InnerResult<Option<Value>> {
    fn parse_toml_value(value: String) -> Value {
        if let Ok(value) = bool::from_str(&value) {
            return Value::Boolean(value);
        }
        if let Ok(value) = f64::from_str(&value) {
            return Value::Float(value);
        }
        if let Ok(value) = i64::from_str(&value) {
            return Value::Integer(value);
        }
        if let Ok(value) = toml::value::Datetime::from_str(&value) {
            return Value::Datetime(value);
        }

        Value::String(value)
    }

    // Using a BTreeMap to ensure values are sorted by environment variable, so that array indices
    // are in the correct order of insertion to avoid an out of bounds.
    let mut map_env: BTreeMap<Cow<'_, str>, TomlKeyPath> = map_env
        .into_iter()
        .map(|(key, value)| (Cow::Borrowed(key), value))
        .collect();

    if let Some(auto_args) = auto_args {
        let mut prefix = auto_args.prefix.unwrap_or(config_variable_name).to_owned();
        prefix.push_str(auto_args.divider);
        for (key, _) in std::env::vars_os() {
            let key = if let Some(key) = key.to_str() {
                key.to_owned()
            } else {
                continue;
            };

            let key_without_prefix: &str = if let Some(0) = key.find(&prefix) {
                key.split_at(prefix.len()).1
            } else {
                continue;
            };

            let key_transformed = (auto_args.transform)(key_without_prefix);
            let toml_key: TomlKeyPath =
                if let Ok(key) = key_transformed.replace(auto_args.divider, ".").parse() {
                    key
                } else {
                    continue;
                };

            map_env.entry(key.into()).or_insert(toml_key);
        }
    }

    if map_env.is_empty() {
        return Ok(None);
    }

    if !matches!(logging, Logging::None) {
        let mut buffer = String::new();
        buffer.push_str("\x1b[34m");
        for (k, v) in &map_env {
            if std::env::var(k.as_ref()).is_ok() {
                buffer.push_str(&format!("\n{k} => {v}"));
            }
        }
        buffer.push_str("\x1b[0m");
        log_info(
            logging,
            format_args!("Loading config from current environment variables: {buffer}"),
        );
    }

    log_info(logging, format_args!("Loading config from environment"));

    let mut config = toml::Value::Table(toml::Table::new());
    for (variable_name, toml_key) in map_env {
        let value = match std::env::var(variable_name.as_ref()) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => continue,
            Err(error) => {
                return Err(InnerError::ErrorReadingEnvironmentVariable {
                    name: (*variable_name.into_owned()).to_owned(),
                    error,
                })
            }
        };
        let value = parse_toml_value(value);
        insert_toml_value(&mut config, toml_key.clone(), value)?;
    }

    Ok(Some(config.into()))
}

/// Initialize configuration from available sources specified in [`Args`].
///
/// If no configuration was found, will return `None`.
///
/// See [`toml-env`](crate).
pub fn initialize<C>(args: Args<'_>) -> Result<Option<C>>
where
    C: DeserializeOwned + Serialize,
{
    let config_variable_name = args.config_variable_name;
    let logging = args.logging;
    let dotenv_path = args.dotenv_path;

    let config_env_config: Option<(Value, ConfigSource)> = match std::env::var(config_variable_name) {
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
                        std::fs::read_to_string(path).map_err(|error| InnerError::ErrorReadingFile {
                            path: path.to_owned(),
                            error,
                        })?;
                    let config: Value = toml::from_str(&config_str).map_err(|error| {
                        InnerError::ErrorParsingTomlFile {
                            path: path.to_owned(),
                            error: error.into(),
                        }
                    })?;
                    log_info(logging, format_args!("Options loaded from file specified in `{config_variable_name}` environment variable: {path:?}"));
                    Ok(Some(config))
                } else {
                    Err(InnerError::ErrorParsingEnvironmentVariableAsConfigOrFile {
                        name: config_variable_name.to_owned(),
                        value: variable_value,
                        error: error.into(),
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
        Err(error) => Err(InnerError::ErrorReadingEnvironmentVariable {
            name: config_variable_name.to_owned(),
            error,
        }),
    }?.map(|config| {
        let source = ConfigSource::DotEnv(args.dotenv_path.to_owned());
        (config, source)
    });

    let dotenv_config =
        initialize_dotenv_toml(dotenv_path, config_variable_name, logging)?.map(|config| {
            (
                config,
                ConfigSource::Environment {
                    variable_names: vec![args.config_variable_name.to_owned()],
                },
            )
        });

    let config: Option<(Value, ConfigSource)> = match (dotenv_config, config_env_config) {
        (None, None) => None,
        (None, Some(config)) => Some(config),
        (Some(config), None) => Some(config),
        (Some(from), Some(into)) => {
            let config = serde_toml_merge::merge(into.0, from.0).map_err(|error| {
                InnerError::ErrorMerging {
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

    let env_config = initialize_env(
        args.logging,
        args.map_env.clone(),
        args.auto_map_env,
        config_variable_name,
    )?
    .map(|value| {
        (
            value,
            ConfigSource::Environment {
                variable_names: args.map_env.keys().map(|key| (*key).to_owned()).collect(),
            },
        )
    });

    let config = match (config, env_config) {
        (None, None) => None,
        (None, Some(config)) => Some(config),
        (Some(config), None) => Some(config),
        (Some(from), Some(into)) => {
            let config = serde_toml_merge::merge(into.0, from.0).map_err(|error| {
                InnerError::ErrorMerging {
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

    let file_config: Option<(Value, ConfigSource)> =
        Option::transpose(args.config_path.map(|path| {
            if path.is_file() {
                let file_string = std::fs::read_to_string(path).map_err(|error| {
                    InnerError::ErrorReadingFile {
                        path: path.to_owned(),
                        error,
                    }
                })?;
                return Result::Ok(Some((
                    toml::from_str(&file_string).map_err(|error| {
                        InnerError::ErrorParsingTomlFile {
                            path: path.to_owned(),
                            error: error.into(),
                        }
                    })?,
                    ConfigSource::File(path.to_owned()),
                )));
            }
            Ok(None)
        }))?
        .flatten();

    let config = match (config, file_config) {
        (None, None) => None,
        (None, Some(config)) => Some(config),
        (Some(config), None) => Some(config),
        (Some(from), Some(into)) => {
            let config = serde_toml_merge::merge(into.0, from.0).map_err(|error| {
                InnerError::ErrorMerging {
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

    let config = Option::transpose(config.map(|(config, source)| {
        C::deserialize(config).map_err(|error| InnerError::ErrorParsingMergedToml {
            source,
            error: error.into(),
        })
    }))?;

    match (logging, config.as_ref()) {
        (_, Some(config)) => {
            let config_string = toml::to_string_pretty(&config)
                .expect("Expected to be able to re-serialize config toml");
            log_info(
                logging,
                format_args!("Parsed configuration:\n\x1b[34m{config_string}\x1b[0m"),
            );
        }
        (Logging::None, _) | (_, None) => {}
    }

    Ok(config)
}

#[cfg(test)]
mod test {
    use crate::InsertTomlValueError;

    use super::insert_toml_value;
    #[test]
    fn insert_toml_value_empty_path() {
        let mut value = toml::Value::String("Hello".to_owned());
        insert_toml_value(
            &mut value,
            "".parse().unwrap(),
            toml::Value::String("World".to_owned()),
        )
        .unwrap();
        assert_eq!(value.as_str().unwrap(), "World");
    }

    #[test]
    fn insert_toml_value_table_property() {
        let mut value = toml::Value::Table(toml::Table::new());
        insert_toml_value(
            &mut value,
            "child".parse().unwrap(),
            toml::Value::String("Hello Child".to_owned()),
        )
        .unwrap();
        assert_eq!(value.get("child").unwrap().as_str().unwrap(), "Hello Child");
    }

    #[test]
    fn insert_toml_value_table_property_property() {
        let mut value = toml::Value::Table(toml::Table::new());
        insert_toml_value(
            &mut value,
            "child.value".parse().unwrap(),
            toml::Value::String("Hello Child Value".to_owned()),
        )
        .unwrap();
        assert_eq!(
            value
                .get("child")
                .unwrap()
                .get("value")
                .unwrap()
                .as_str()
                .unwrap(),
            "Hello Child Value"
        );
    }

    #[test]
    fn insert_toml_value_create_array() {
        let mut value = toml::Value::Array(Vec::new());
        insert_toml_value(
            &mut value,
            "0".parse().unwrap(),
            toml::Value::String("Hello Element".to_owned()),
        )
        .unwrap();
        assert_eq!(value.get(0).unwrap().as_str().unwrap(), "Hello Element");
    }

    #[test]
    fn insert_toml_value_array_out_of_bounds_error() {
        let mut value = toml::Value::Array(Vec::new());
        let error = insert_toml_value(
            &mut value,
            "1".parse().unwrap(),
            toml::Value::String("Hello Element".to_owned()),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            InsertTomlValueError::ArrayOutOfBounds { .. }
        ))
    }

    #[test]
    fn insert_toml_value_table_property_index_error() {
        let mut value = toml::Value::Array(Vec::new());
        let error = insert_toml_value(
            &mut value,
            "key".parse().unwrap(),
            toml::Value::String("Hello Element".to_owned()),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            InsertTomlValueError::TablePropertyCannotIndex { .. }
        ))
    }

    #[test]
    fn insert_toml_value_array_index_cannot_index_error() {
        let mut value = toml::Value::Table(toml::Table::new());
        let error = insert_toml_value(
            &mut value,
            "0".parse().unwrap(),
            toml::Value::String("Hello Element".to_owned()),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            InsertTomlValueError::ArrayIndexCannotIndex { .. }
        ))
    }

    #[test]
    fn insert_toml_value_table_child_create_array() {
        let mut value = toml::Value::Table(toml::Table::new());
        insert_toml_value(
            &mut value,
            "child.0".parse().unwrap(),
            toml::Value::String("Hello Element".to_owned()),
        )
        .unwrap();
        assert_eq!(
            value
                .get("child")
                .unwrap()
                .get(0)
                .unwrap()
                .as_str()
                .unwrap(),
            "Hello Element"
        );
    }
}
