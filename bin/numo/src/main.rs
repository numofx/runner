/// Numo Engine Arbitrage Bot
///
/// Monitors Numo Engine pools on Celo and executes arbitrage when pool-implied
/// discount factors diverge from the SOFR curve.
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use ethers::prelude::*;
use ethers::providers::{Provider, Ws};
use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};

use artemis_core::collectors::block_collector::{BlockCollector, NewBlock};
use artemis_core::engine::Engine;
use artemis_core::executors::mempool_executor::MempoolExecutor;
use artemis_core::types::{CollectorMap, ExecutorMap};

use numo_arb::sofr::SofrCurve;
use numo_arb::strategy::NumoArb;
use numo_arb::types::{Action, Config, Event, NewBlockEvent};

/// CLI Options for the Numo arbitrage bot
#[derive(Parser, Debug)]
#[command(name = "numo")]
#[command(about = "Numo Engine arbitrage bot for Celo", long_about = None)]
pub struct Args {
    /// Celo node WebSocket endpoint (e.g., wss://forno.celo.org/ws)
    #[arg(long, env = "WSS")]
    pub wss: String,

    /// Private key for sending transactions (64 hex chars, no 0x prefix)
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: String,

    /// Address of the deployed NumoArbRouter contract
    #[arg(long, env = "ROUTER_ADDRESS")]
    pub router_address: String,

    /// Comma-separated list of Numo Engine pool addresses to monitor
    #[arg(long, env = "POOL_ADDRESSES", value_delimiter = ',')]
    pub pool_addresses: Vec<String>,

    /// Minimum edge in basis points before executing arbitrage (default: 10 = 0.10%)
    #[arg(long, env = "EDGE_BPS", default_value = "10")]
    pub edge_bps: u32,

    /// Slippage tolerance in basis points (default: 50 = 0.50%)
    #[arg(long, env = "SLIPPAGE_BPS", default_value = "50")]
    pub slippage_bps: u32,

    /// Maximum FY token amount per trade (in smallest units, e.g., wei)
    #[arg(long, env = "MAX_FY_AMOUNT")]
    pub max_fy_amount: Option<u128>,

    /// Maximum base token amount to risk per trade
    #[arg(long, env = "MAX_BASE_AMOUNT")]
    pub max_base_amount: Option<u128>,

    /// Percentage of expected profit to bid in gas fees (0-100, default: 80)
    #[arg(long, env = "BID_PERCENTAGE", default_value = "80")]
    pub bid_percentage: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Set up tracing/logging
    let filter = filter::Targets::new()
        .with_target("numo_arb", Level::INFO)
        .with_target("numo", Level::INFO)
        .with_target("artemis_core", Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    info!("Starting Numo Engine Arbitrage Bot");

    // Parse command-line arguments (with .env fallback)
    let args = Args::parse();

    // Validate configuration
    if args.pool_addresses.is_empty() {
        anyhow::bail!("At least one pool address must be specified");
    }

    info!(
        wss = %args.wss,
        router = %args.router_address,
        pools = args.pool_addresses.len(),
        edge_bps = args.edge_bps,
        slippage_bps = args.slippage_bps,
        "Configuration loaded"
    );

    // Connect to Celo via WebSocket
    info!("Connecting to Celo...");
    let ws = Ws::connect(&args.wss).await?;
    let provider = Provider::new(ws);

    // Set up wallet
    let wallet: LocalWallet = args.private_key.parse()?;
    let address = wallet.address();
    info!(bot_address = ?address, "Wallet loaded");

    // Wrap provider with signer and nonce manager
    let provider = Arc::new(provider.nonce_manager(address).with_signer(wallet));

    // Parse pool addresses
    let pool_addresses: Result<Vec<Address>> = args
        .pool_addresses
        .iter()
        .map(|s| {
            Address::from_str(s).map_err(|e| anyhow::anyhow!("Invalid pool address {}: {}", s, e))
        })
        .collect();
    let pool_addresses = pool_addresses?;

    // Build strategy configuration
    let config = Config {
        router_address: Address::from_str(&args.router_address)?,
        pool_addresses,
        edge_bps: args.edge_bps,
        slippage_bps: args.slippage_bps,
        max_fy_amount: args.max_fy_amount.unwrap_or(100_000u128 * 10u128.pow(18)),
        max_base_amount: args.max_base_amount.unwrap_or(50_000u128 * 10u128.pow(18)),
        bid_percentage: args.bid_percentage,
    };

    info!(
        router = ?config.router_address,
        pools = config.pool_addresses.len(),
        "Strategy configuration initialized"
    );

    // Initialize SOFR curve with default USD rates
    // TODO: Load real SOFR rates from data provider
    let sofr_curve = SofrCurve::default_usd();
    info!(
        "SOFR curve initialized with {} knots",
        sofr_curve.knots.len()
    );

    // Set up Artemis Engine
    let mut engine: Engine<Event, Action> = Engine::default();

    // Add block collector
    let block_collector = Box::new(BlockCollector::new(provider.clone()));
    let block_collector = CollectorMap::new(block_collector, |block: NewBlock| {
        // Get current timestamp from system (blocks don't have timestamps in the event)
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Event::NewBlock(NewBlockEvent {
            block_number: block.number.as_u64(),
            timestamp,
            base_fee: None, // Not available in NewBlock event
        })
    });
    engine.add_collector(Box::new(block_collector));
    info!("Block collector added");

    // Create Numo arbitrage strategy
    // Note: sync_state() is called automatically by the Engine
    let strategy = NumoArb::new(Arc::new(provider.clone()), config, sofr_curve);

    engine.add_strategy(Box::new(strategy));
    info!("Numo arbitrage strategy added");

    // Add mempool executor
    let executor = Box::new(MempoolExecutor::new(provider.clone()));
    let executor = ExecutorMap::new(executor, |action| match action {
        Action::SubmitTx(tx) => Some(tx),
    });
    engine.add_executor(Box::new(executor));
    info!("Mempool executor added");

    // Start the engine
    info!("Starting Artemis engine...");
    info!("Bot is now running. Press Ctrl+C to stop.");

    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            match res {
                Ok(_) => info!("Task completed successfully"),
                Err(e) => tracing::error!("Task error: {:?}", e),
            }
        }
    }

    info!("Shutting down...");
    Ok(())
}
