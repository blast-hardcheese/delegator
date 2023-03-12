delegator
===

Analysis, Planning, and Routing of cryptograms, a part of the appreciate platform.

- [delegator](#delegator)
  - [Setup](#setup)
    - [Install dependencies](#install-dependencies)
    - [Typecheck](#typecheck)
    - [Run the service](#run-the-service)
  - [Run all tests](#run-tests)
    - [Run some tests](#run-some-tests)

## Setup

### Install dependencies

```bash
$ cargo run -- ./config/development.conf
```

### Typecheck

Only run the typechecking phase (faster than a full build, good for development)

```bash
$ cargo watch
```

### Run the service

```bash
$ cargo run ./config/development.conf
```

Or with hot-reload...

```bash
$ cargo watch -x 'run ./config/development.conf'
```

Or with more logging...

```bash
$ RUST_LOG=actix_web=debug RUST_BACKTRACE=1 cargo watch -x 'run ./config/development.conf'
```

## Run all tests

```bash
$ cargo test
```

### Run some tests

Run only the tests which contain the word "translate"
```
RUST_LOG=actix_web=debug RUST_BACKTRACE=1 cargo test translate
```
