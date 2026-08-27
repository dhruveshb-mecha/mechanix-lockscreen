export CROSS_CONTAINER_ENGINE := "podman"

target := "aarch64-unknown-linux-gnu"
image := "mecha-lockscreen-cross-aarch64:latest"

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

build:
    cargo build

test:
    cargo test

ci: fmt-check clippy build test

image:
    podman build -f Dockerfile.cross -t {{ image }} .

cross:
    "${CARGO_HOME:-$HOME/.cargo}/bin/cross" build --release --target {{ target }}
