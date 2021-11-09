# rs-queue-bench

## Getting started

To run a benchmark, run:

```
cargo run --release --bin <benchmark-name>
```

where a list of `<benchmark-name>`s can be found under each `[[bin]]` entry in the `Cargo.toml` file.

## Benchmarks

Some queue implementations to benchmark:

- https://docs.rs/nolock/0.3.1/nolock/queues/spsc/unbounded/index.html
- https://docs.rs/lockfree/0.5.1/lockfree/channel/spsc/index.html
- https://docs.rs/rtrb/0.2.0/rtrb/
