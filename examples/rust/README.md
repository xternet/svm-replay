# Rust host

From the repository root:

```sh
cargo run --locked --example replay-rust -- \
  /path/to/prepared-request.json /path/to/catalog.json CATALOG_SHA256 \
  /path/to/pinned/svm-replay /absolute/path/to/new-run-data
```

main.rs embeds the engine using an already prepared boundary. The pinned CLI
owns worker child processes; the receipt separately identifies this Rust host.
This example does not fetch from a provider. Native workers and their reviewed
catalog must already be installed. A non-completed replay returns an error.
