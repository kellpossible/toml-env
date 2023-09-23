# `toml-env`

A simple configuration library using `toml`.

This library is designed to load a configuration for an application at startup using the `initialize()` function. The configuration can be loaded (in order of preference):

1. From a dotenv style file `.env.toml` (a file name of your choosing)
2. From an environment variable `CONFIG` (or a variable name of your choosing).
3. From a configuration file.

## Why yet another config library?

Here are some possible alternatives to this library:

+ [`config`](https://crates.io/crates/config/) You want maximum flexibility.
+ [`figment`](https://crates.io/crates/figment) You want maximum flexibility.
+ [`just-config`](https://crates.io/crates/justconfig/) You want maximum flexibility.
+ [`dotenvy`](https://crates.io/crates/dotenvy) You just want `.env` support.
+ [`env_inventory`](https://crates.io/crates/env-inventory/) You just want environment variable configuration file support.

Why would you use this one?

+ Small feature set.
+ Minimal dependencies.
+ Opinionated defaults.
+ `.env` using `TOML` which is a more established file format standard.
+ Loading config from TOML stored in a multiline environment variable.
  + For large configurations with nested maps, this could be seen as a bit more legible than `MY_VARIABLE__SOMETHING_ELSE__SOMETHING_SOMETHING_ELSE`.
  + You can also just copy text from a TOML file to use in the environment variable instead of translating it into complicated names of variables.

## Config Struct

Firstly you need to define your struct which implements `serde::de::DeserializeOwned` + `serde::Serialize` + `Default`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Config {
    config_value_1: String,
    config_value_2: String,
    config_child: ConfigChild
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ConfigChild {
    config_value_3: String,
}
```

## `.env.toml`

Initally configuration will attempted to be loaded from a file named `.env.toml` by default. You can elect to customize the name of this file. The format of this file is as follows:

```toml
SECRET_ENV_VAR_1="some value"
SECRET_ENV_VAR_2="some other value"

[CONFIG]
config_value_1="some value"
config_value_2="some other value"

[CONFIG.config_child]
config_value_3="some other other value"
```

Environment variables for the application can be set using the top level keys in the file (e.g. `SECRET_ENV_VAR_1`).

The configuration can be loaded from a subset of this file in `CONFIG`. The `CONFIG` key will be the name from the `Args::config_variable_name` which is `CONFIG` by default.


## Environment Variable `CONFIG`

You can specify the configuration by storing it in the variable name as specified using `Args::config_variable_name` (`CONFIG` by default).

```bash
# Store a multiline string into an environment variable.
read -r -d '' CONFIG << EOM
config_value_1="some value"
config_value_2="some other value"

[config_child]
config_value_3="some other other value"
EOM
```

## Example

```rust
use serde::{Deserialize, Serialize};
use tempfile::tempdir;
use toml_env::{Args, initialize, Logging};

#[derive(Serialize, Deserialize)]
struct Config {
    value_1: String,
    value_2: bool,
    child: Child,
}

#[derive(Serialize, Deserialize, Default)]
struct Child {
    value_3: i32,
    value_4: u8,
}
let dir = tempdir().unwrap();
let dotenv_path = dir.path().join(".env.toml");
let config_path = dir.path().join("config.toml");

// Normally you would read this from .env.toml file
std::fs::write(
    &dotenv_path,
    r#"
SECRET="hello-world"
[MY_CONFIG]
value_1="Something from .env.toml"
[MY_CONFIG.child]
value_3=-5
value_4=16
"#,
)
.unwrap();


// Normally you may choose set this from a shell script or some
// other source in your environment (docker file or server config file).
std::env::set_var(
    "MY_CONFIG",
    r#"
value_1="Something from MY_CONFIG environment"
value_2=true
"#,
);

// Normally you would read this from config.toml 
// (or whatever name you want) file.
std::fs::write(
    &config_path,
    r#"
value_1="Something from config.toml"
value_2=false
value_4=45
"#,
)
.unwrap();

let config: Config = initialize(Args {
    dotenv_path: &dotenv_path,
    config_path: Some(&config_path),
    config_variable_name: "MY_CONFIG",
    logging: Logging::StdOut,
})
.unwrap()
.unwrap();

assert_eq!(config.value_1, "Something from .env.toml");
assert_eq!(config.value_2, true);
assert_eq!(config.child.value_3, -5);
assert_eq!(config.child.value_4, 16);

let secret = std::env::var("SECRET").unwrap();
assert_eq!(secret, "hello-world");
```

## Changelog

See [CHANGELOG.md](https://github.com/kellpossible/toml-env/blob/master/CHANGELOG.md) for an account of changes to this library.
