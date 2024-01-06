# Job Manager

[![MIT licensed](https://img.shields.io/badge/license-MIT0-blue.svg)](./LICENSE)

Job Manager is a Rust crate for scheduling and managing asynchronous jobs.

# Status

This is a very early release and completely untested. There will be frequent enhancements while we explore this
create in the context of the ply backend crates.

If you'd like to contribute (eg by providing another storage implementation) please open an issue.

# Features

- Asynchronous job execution.
- Job scheduling using cron-like expressions.
- Flexible job runner implementations.
- Extensible for custom job and lock repositories.
- Lock management for job synchronization.

# Example Usage

The [counter example with MongoDB backend](examples/counter/main.rs) is a good place to start to understand
how to use the jobs crate. 

You can run this example like this:

~~~~
docker-compose -f examples/counter/docker-compose.yml up -d
cargo run --example counter --features mongodb
~~~~

Hit `CTRL+C` to stop the example and

~~~~
docker-compose -f examples/counter/docker-compose.yml down
~~~~

To stop the MongoDB container.
