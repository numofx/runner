# Runner 🏃‍♂️

A bot that keeps the discount factors of **Numo Engines** (e.g. `USDT/fyUSDT` pools) aligned with real-world FX markets using a fixed-income arbitrage strategy.

## Overview

### Collector (per block)

**Goal:** Batch-read all Engine pools via multicall, then compute the marginal price:

### Strategy

1. **Gets target discount factor**  
   Computes discount factor from SOFR curve or analogous STIR benchmark if not a USD pool. 

2. **Compares implied discount factors**  
   Compares the pool implied discount factor

3. **Solves for optimal trade size**  
   Determines the trade size such that the **post-trade marginal price** equals the discount factor.

4. **Emits arbitrage action**  
   Triggers the Router contract to execute both legs atomically:
   - **Buy ZCB on cheap pool / Sell ZCB on rich pool**, or  
   - **Single pool mint/sell loop** (if applicable).


###  Executor

Submit a single transaction on [Celo](https://celoscan.io/):
- **Include slippage guards.**  
- **Retry** with a fee bump if not included within *N* blocks.

## Build

Before running, make sure the following are installed:

```bash
npm install
forge build
```

## Run

forge script Runner.s.sol --rpc-url $RPC_URL --broadcast

