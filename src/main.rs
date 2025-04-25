use helius::Helius;
use helius::error::Result;
use helius::types::{
    Cluster, RpcTransactionsConfig, TransactionCommitment, TransactionDetails,
    TransactionSubscribeFilter, TransactionSubscribeOptions, UiEnhancedTransactionEncoding,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_stream::StreamExt;
use tracing::{error, info};
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

const WSOL_MINT_KEY_STR: &str = "So11111111111111111111111111111111111111112";
const USDC_MINT_KEY_STR: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT_MINT_KEY_STR: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let formatting_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_line_number(false)
        .with_target(true)
        .with_level(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(formatting_layer)
        .init();

    let api_key = std::env::var("HELIUS_ATLAS_API_KEY").expect("HELIUS_ATLAS_API_KEY is not set");
    let cluster: Cluster = Cluster::MainnetBeta;

    // Uses custom ping-pong timeouts to ping every 15s and timeout after 45s of no pong
    let helius: Helius =
        Helius::new_with_ws_with_timeouts(&api_key, cluster, Some(15), Some(45)).await?;

    let config = RpcTransactionsConfig {
        filter: TransactionSubscribeFilter {
            account_include: Some(vec![
                USDC_MINT_KEY_STR.to_string(),
                USDT_MINT_KEY_STR.to_string(),
                WSOL_MINT_KEY_STR.to_string(),
            ]),
            account_exclude: None,
            account_required: None,
            vote: None,
            failed: None,
            signature: None,
        },
        options: TransactionSubscribeOptions {
            commitment: Some(TransactionCommitment::Confirmed),
            encoding: Some(UiEnhancedTransactionEncoding::Base64),
            transaction_details: Some(TransactionDetails::Full),
            show_rewards: None,
            max_supported_transaction_version: Some(0),
        },
    };

    let count = Arc::new(AtomicU64::new(0));
    let count_clone = count.clone();

    // Spawn a separate task for printing statistics
    tokio::spawn(async move {
        let mut last_count = 0;
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

        loop {
            interval.tick().await;
            let current_count = count_clone.load(Ordering::Relaxed);

            if last_count != 0 && current_count == last_count {
                error!("No new transactions received in the last 5 seconds");
            } else {
                info!(
                    "helius_atlas_ws_transaction_updates_received: {}",
                    current_count
                );
            }
            last_count = current_count;
        }
    });

    info!("Starting helius ws transaction updates");
    if let Some(ws) = helius.ws() {
        let (mut stream, _unsub) = ws.transaction_subscribe(config).await?;
        while let Some(_event) = stream.next().await {
            count.fetch_add(1, Ordering::Relaxed);
        }
    }

    Ok(())
}
