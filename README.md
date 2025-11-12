# Numo Engine Arbitrage Bot

An automated arbitrage bot that keeps the discount factors of **Numo Engine pools** (e.g. `USDT/fyUSDT` pools) aligned with real-world fixed-income markets by executing profitable trades when prices diverge from the SOFR curve.

## Overview

The bot monitors Numo Engine pools on Celo and executes atomic arbitrage transactions when pool-implied discount factors diverge from the SOFR curve.

### Architecture

**Collector → Strategy → Executor**

1. **BlockCollector** (per block): Monitors new blocks on Celo
2. **NumoArb Strategy**:
   - Computes discount factors from SOFR curve
   - Compares pool implied discount factors via marginal price probes
   - Solves for optimal trade size using bisection search
   - Emits arbitrage actions when profitable
3. **MempoolExecutor**: Submits transactions to Celo with slippage protection

### Smart Contract

The bot uses `NumoArbRouter.sol` to execute atomic arbitrage:
- **Buy FY tokens on cheap pool**
- **Sell FY tokens on rich pool**
- **Profit extraction** with built-in slippage and profit checks

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Foundry (for contract deployment)
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### Build

```bash
# Build the bot
cargo build --release

# Binary will be at: ./target/release/numo
```

### Configure

Copy and edit the `.env.example` file:

```bash
cp .env.example .env
# Edit .env with your configuration
```

Required configuration:
- `WSS` - Celo WebSocket RPC endpoint
- `PRIVATE_KEY` - Bot wallet private key
- `ROUTER_ADDRESS` - Deployed NumoArbRouter contract address
- `POOL_ADDRESSES` - Comma-separated list of Numo Engine pool addresses

### Deploy Router Contract

```bash
cd crates/strategies/numo-arb/contracts

# Deploy to Celo Alfajores (testnet)
forge create src/NumoArbRouter.sol:NumoArbRouter \
  --rpc-url https://alfajores-forno.celo-testnet.org \
  --private-key YOUR_DEPLOY_KEY \
  --constructor-args BASE_TOKEN_ADDRESS FY_TOKEN_ADDRESS

# Update .env with deployed address
```

### Run

```bash
# Run the bot
./target/release/numo

# Or with explicit config
./target/release/numo \
  --wss wss://forno.celo.org/ws \
  --private-key YOUR_KEY \
  --router-address 0xROUTER \
  --pool-addresses 0xPOOL1,0xPOOL2
```

## Configuration

See `.env.example` for all available configuration options:

- **Edge threshold** (`EDGE_BPS`): Minimum price divergence to trade (default: 10 bps)
- **Slippage** (`SLIPPAGE_BPS`): Slippage tolerance (default: 50 bps)
- **Position limits**: Max FY and base token amounts per trade
- **Gas bidding**: Percentage of profit to spend on gas

## Project Structure

```
.
├── bin/numo/                  # Main executable
├── crates/
│   ├── artemis-core/          # Core framework (collectors, executors, engine)
│   └── strategies/numo-arb/   # Numo arbitrage strategy
│       ├── src/
│       │   ├── sofr.rs        # SOFR curve & discount factors
│       │   ├── pricing.rs     # Price discovery & trade sizing
│       │   ├── strategy.rs    # Main arbitrage logic
│       │   └── types.rs       # Type definitions
│       ├── contracts/         # Smart contracts
│       │   └── src/NumoArbRouter.sol
│       └── bindings/          # Contract ABI bindings
└── .env                       # Configuration

```

## Development

```bash
# Check compilation
cargo check -p numo-arb

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Safety & Risk Management

⚠️ **Important Safety Notes:**

1. **Start small** - Test on Alfajores testnet first
2. **Position limits** - Configure conservative max amounts
3. **Monitoring** - Always monitor bot logs and transactions
4. **SOFR updates** - Keep discount factor curve updated with real data
5. **Wallet security** - Use a dedicated wallet with limited funds

## License

Licensed under MIT OR Apache-2.0

