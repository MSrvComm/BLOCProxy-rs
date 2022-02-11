# MiCoProxy Sidecar in Rust

The idea here is to build a Rust version of the MiCoProxy which I hope will be faster. Also, I hope to be able to replace all syncing/locking/atomics that are currently present in the Go version with alternate techniques like "I don't care if the data is absolutely the latest", thread-local storage or something similar.

The proxy is dependent on the some external async crates like `async-web`, `future` and `reqwest`. For a full list refer to the `Cargo.toml` file.