# Changelog for `toml-env`

## `main` Branch

### Documentation

- Add badges to README.
- Fix type code comment for `Result`.

## v1.0.0

### Notes

I consider this library to be feature complete now! It will be version `1.0.0` in order to prevent future scope creep temptations. It already has a lot of functionality with a surprisingly small amount of code.

### New Features

- Add ability to map variables from environment in an automatic fashion, similar to other libraries.
- Improved README with many more examples.

### Bug Fixes

- Fixed some corner cases with order of operations.

### Breaking

- Change `Args::map_env` to a hashmap, better API and simpler code.

## v0.3.5

### Bug Fixes

- Improved log formatting.

## v0.3.4

### Bug Fixes

- Improved log formatting.

## v0.3.3

### Bug Fixes

- Improved logging for setting environment variables from `.env.toml`.

## v0.3.3

### Bug Fixes

- Improved log formatting.

## v0.3.2

### Bug Fixes

- Fix logging formatting of environment variables mapped.

## v0.3.1

### Bug Fixes

- Fixed bug parsing environment variables.

## v0.3.0

### New Features

- Improved api for `Args::map_env` using `IntoIterator`.

## v0.2.0

### New Features

- Support for mapping environment variables into the config (`X => y.z`).

## v0.1.1

### Documentation

- Fix example in README.

## v0.1.0

- Initial release.
