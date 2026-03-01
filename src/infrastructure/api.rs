use crate::core::config::Config;
use crate::core::error::{AppError, Result};
use crate::core::wallet::WalletInfo;
use futures::future::join_all;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

/// API response structure from mempool.space
#[derive(Serialize, Deserialize, Debug)]
struct MempoolStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

/// Top-level API response structure
#[derive(Serialize, Deserialize, Debug)]
struct MempoolResponse {
    chain_stats: MempoolStats,
}

/// Result of a balance check for a single address
#[derive(Debug)]
pub struct BalanceResult {
    pub address: String,
    pub balance: u64,
    pub error: Option<AppError>,
}

/// Thread-safe API client with improved performance and error handling
pub struct ApiClient {
    client: Client,
    config: std::sync::Arc<Config>,
}

impl ApiClient {
    /// Create a new API client with optimized settings
    pub async fn new(config: std::sync::Arc<Config>) -> Result<Self> {
        const POOL_MAX_IDLE_PER_HOST: usize = 50; // Increased for better performance
        const TCP_KEEPALIVE_SECS: u64 = 60;
        const CONNECT_TIMEOUT_SECS: u64 = 10;

        let client = Client::builder()
            .timeout(config.api_timeout())
            .connect_timeout(std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .user_agent("MnemonicGuesser/1.0")
            .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .tcp_keepalive(Some(std::time::Duration::from_secs(TCP_KEEPALIVE_SECS)))
            .build()
            .map_err(|e| AppError::Network {
                source: Box::new(e),
                url: None,
                retry_count: Some(0),
            })?;

        Ok(Self { client, config })
    }

    /// Clone necessary parts for async task (Client is cheap to clone - it's Arc internally)
    fn clone_for_task(&self) -> Self {
        Self {
            client: self.client.clone(),
            config: std::sync::Arc::clone(&self.config),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Check balance for a single address with retry logic (helper for batch operations)
    async fn check_single_address_with_retry(
        &self,
        address: &str,
        base_url: &str,
    ) -> BalanceResult {
        // Build URL once, reuse for all retry attempts
        let url = format!("{}/address/{}", base_url, address);
        let address_owned = address.to_string(); // Owned copy for return value
        
        for attempt in 1..=self.config.api.max_retries {
            match timeout(self.config.api_timeout(), self.client.get(&url).send()).await {
                Ok(Ok(response)) if response.status().is_success() => {
                    match response.json::<MempoolResponse>().await {
                        Ok(mempool_data) => {
                            let balance = mempool_data
                                .chain_stats
                                .funded_txo_sum
                                .saturating_sub(mempool_data.chain_stats.spent_txo_sum);

                            debug!("Balance for {}: {} sat", address, balance);
                            return BalanceResult {
                                address: address_owned,
                                balance,
                                error: None,
                            };
                        }
                        Err(e) if attempt == self.config.api.max_retries => {
                            return BalanceResult {
                                address: address_owned,
                                balance: 0,
                                error: Some(AppError::Network {
                                    source: Box::new(e),
                                    url: Some(url),
                                    retry_count: Some(attempt),
                                }),
                            };
                        }
                        Err(_) => {}
                    }
                }
                Ok(Ok(response)) => {
                    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        sleep(self.config.rate_limit_delay() * attempt as u32).await;
                        continue;
                    } else if attempt == self.config.api.max_retries {
                        let status = response.status();
                        return BalanceResult {
                            address: address_owned,
                            balance: 0,
                            error: Some(AppError::Network {
                                source: Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("HTTP {}", status),
                                )),
                                url: Some(url),
                                retry_count: Some(attempt),
                            }),
                        };
                    }
                }
                Ok(Err(e)) if attempt == self.config.api.max_retries => {
                    return BalanceResult {
                        address: address_owned,
                        balance: 0,
                        error: Some(AppError::Network {
                            source: Box::new(e),
                            url: Some(url),
                            retry_count: Some(attempt),
                        }),
                    };
                }
                Err(_) if attempt == self.config.api.max_retries => {
                    return BalanceResult {
                        address: address_owned,
                        balance: 0,
                        error: Some(AppError::Network {
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "Request timeout after all retries",
                            )),
                            url: Some(url),
                            retry_count: Some(attempt),
                        }),
                    };
                }
                _ => {}
            }

            // Wait before retry
            sleep(self.config.rate_limit_delay()).await;
        }

        // Should not reach here, but just in case
        BalanceResult {
            address: address_owned,
            balance: 0,
            error: Some(AppError::Network {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Max retries exceeded",
                )),
                url: Some(url),
                retry_count: Some(self.config.api.max_retries),
            }),
        }
    }

    /// Check balances for multiple addresses concurrently
    pub async fn perform_batch_check(&self, base_url: &str, addresses: &[&str]) -> Vec<BalanceResult> {
        if addresses.is_empty() {
            return Vec::new();
        }

        debug!("Checking balances for {} addresses concurrently", addresses.len());

        // Create concurrent tasks for all addresses
        // Clone client once per task (Client uses Arc internally, so cheap)
        let base_url_arc: Arc<str> = Arc::from(base_url);
        let tasks: Vec<_> = addresses
            .iter()
            .map(|&address| {
                let client = self.clone_for_task();
                let addr = address.to_string();
                let base_url = Arc::clone(&base_url_arc);

                async move {
                    client.check_single_address_with_retry(&addr, &base_url).await
                }
            })
            .collect();

        // Execute all requests concurrently
        let results: Vec<BalanceResult> = join_all(tasks).await;

        // Log results summary at debug level
        let success_count = results.iter().filter(|r| r.error.is_none()).count();
        let error_count = results.len() - success_count;

        debug!(
            "Completed balance checks: {} successful, {} failed",
            success_count, error_count
        );

        results
    }
}

/// Check wallet balances concurrently for maximum performance
pub async fn check_wallet_with_balance(
    api_client: Arc<ApiClient>,
    wallet_info: &mut WalletInfo,
) -> Result<()> {
    // Collect pending (unchecked) address indices and their string slices in one pass
    let (pending_indices, pending_addresses): (Vec<usize>, Vec<String>) = wallet_info
        .addresses
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.checked)
        .map(|(i, a)| (i, a.address.clone()))
        .unzip();

    if pending_indices.is_empty() {
        return Ok(());
    }

    // Check pending balances concurrently
    let base_url = &api_client.config().api.base_url;
    let pending_refs: Vec<&str> = pending_addresses.iter().map(String::as_str).collect();
    let balance_results = api_client.perform_batch_check(base_url, &pending_refs).await;

    for (j, result) in balance_results.into_iter().enumerate() {
        let i = pending_indices[j];
        let addr_info = &mut wallet_info.addresses[i];
        if result.error.is_none() {
            addr_info.balance = result.balance;
            addr_info.checked = true;
            if result.balance > 0 {
                info!(
                    "Found balance for {}: {} sat",
                    result.address, result.balance
                );
            }
        }
    }

    // Recalculate total balance
    wallet_info.total_balance = wallet_info.addresses.iter().map(|a| a.balance).sum();
    wallet_info.num_failed_checks = wallet_info.addresses.iter().filter(|a| !a.checked).count();

    if wallet_info.total_balance > 0 {
        info!(
            "Total wallet balance: {} sat across {} addresses",
            wallet_info.total_balance,
            wallet_info.addresses.len()
        );
    }

    Ok(())
}
