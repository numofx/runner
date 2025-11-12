set dotenv-load := true

# Build the numo binary
build:
    cargo build --release

# Run the numo bot
run:
    ./target/release/numo

# Check compilation without building
check:
    cargo check -p numo-arb
    cargo check -p numo

# Run tests
test:
    cargo test

# Format code
fmt:
    cargo +nightly fmt --all

# Lint code
clippy:
    cargo clippy --all --all-features

# Clean build artifacts
clean:
    cargo clean

# Deploy router contract to Celo Alfajores testnet
deploy-router-testnet:
    #!/usr/bin/env bash
    cd crates/strategies/numo-arb/contracts
    echo "Deploying NumoArbRouter to Celo Alfajores..."
    forge create src/NumoArbRouter.sol:NumoArbRouter \
        --rpc-url https://alfajores-forno.celo-testnet.org \
        --private-key $PRIVATE_KEY \
        --constructor-args $BASE_TOKEN_ADDRESS $FY_TOKEN_ADDRESS

# Show help
help:
    @just --list
