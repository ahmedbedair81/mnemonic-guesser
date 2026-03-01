use crate::infrastructure::api::ApiClient;
use crate::core::config::Config;
use crate::core::error::{AppError, Result};
use crate::core::wallet::{WalletGenerator, WalletInfo};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tracing::info;

/// Default retry delay when API checks fail (in seconds)
const DEFAULT_RETRY_DELAY_SECS: u64 = 2;

/// Check whether the stop flag is set and return an error if so.
fn check_stop_flag(should_stop: Option<&AtomicBool>) -> Result<()> {
    if let Some(flag) = should_stop {
        if flag.load(Ordering::Relaxed) {
            return Err(AppError::Validation {
                field: "operation".to_string(),
                value: "stopped".to_string(),
                reason: "Operation was stopped by user".to_string(),
            });
        }
    }
    Ok(())
}

/// Service for managing wallet guessing operations
#[derive(Clone)]
pub struct WalletGuessingService {
    wallet_generator: Arc<WalletGenerator>,
    api_client: Arc<ApiClient>,
    config: Arc<Config>,
}

impl WalletGuessingService {
    /// Create a new wallet guessing service
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let wallet_generator = Arc::new(WalletGenerator::new(Arc::clone(&config)));
        let api_client = Arc::new(ApiClient::new(Arc::clone(&config)).await?);

        Ok(Self {
            wallet_generator,
            api_client,
            config,
        })
    }

    /// Check a wallet with retry logic until all addresses are checked
    async fn check_wallet_with_retries(
        api_client: Arc<ApiClient>,
        mut wallet: WalletInfo,
        should_stop: Option<&AtomicBool>,
    ) -> Result<WalletInfo> {
        let retry_delay = Duration::from_secs(DEFAULT_RETRY_DELAY_SECS);

        loop {
            // Check stop flag before starting API call
            check_stop_flag(should_stop)?;

            match crate::infrastructure::api::check_wallet_with_balance(
                Arc::clone(&api_client),
                &mut wallet,
            )
            .await
            {
                Ok(()) => {}
                Err(_) => {
                    sleep(retry_delay).await;
                    continue;
                }
            }

            // Check stop flag after API call completes
            check_stop_flag(should_stop)?;

            // If all addresses are checked, we're done
            if wallet.num_failed_checks == 0 {
                return Ok(wallet);
            }

            // Some addresses still failed - retry after a delay
            sleep(retry_delay).await;
        }
    }

    /// Generate a single wallet and check its balance
    pub async fn generate_and_check_wallet(&self) -> crate::core::error::Result<WalletInfo> {
        let wallet = self.wallet_generator.generate_wallet()?;
        Self::check_wallet_with_retries(
            Arc::clone(&self.api_client),
            wallet,
            None,
        ).await
    }

    /// Update configuration
    pub async fn update_config(&mut self, new_config: Arc<Config>) -> Result<()> {
        // Create new components before replacing old ones (fail fast if error)
        let api_client = Arc::new(ApiClient::new(Arc::clone(&new_config)).await?);
        let wallet_generator = Arc::new(WalletGenerator::new(Arc::clone(&new_config)));

        // Only update if creation succeeded
        self.api_client = api_client;
        self.wallet_generator = wallet_generator;
        self.config = new_config;
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }
}

/// Application state manager
pub struct AppState {
    service: WalletGuessingService,
    current_wallet: Option<WalletInfo>,
    found_wallet: Option<WalletInfo>,
    attempts: u64,
    is_guessing: bool,
    is_generating_single: bool,
    last_error: Option<String>,
    guessing_task: Option<mpsc::UnboundedReceiver<Result<WalletInfo>>>,
    guessing_handles: Vec<JoinHandle<()>>,
    single_generation_handle: Option<JoinHandle<Result<WalletInfo>>>,
    stop_flag: Option<Arc<AtomicBool>>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let service = WalletGuessingService::new(config).await?;

        Ok(Self {
            service,
            current_wallet: None,
            found_wallet: None,
            is_generating_single: false,
            attempts: 0,
            is_guessing: false,
            single_generation_handle: None,
            last_error: None,
            guessing_task: None,
            guessing_handles: Vec::new(),
            stop_flag: None,
        })
    }

    /// Generate a single wallet (starts async task)
    pub async fn generate_single_wallet(&mut self) -> Result<()> {
        if self.is_generating_single {
            return Err(AppError::Validation {
                field: "operation".to_string(),
                value: "already_generating".to_string(),
                reason: "Single wallet generation is already in progress".to_string(),
            });
        }

        self.is_generating_single = true;
        self.reset_error_state();

        let service = self.service.clone();
        let handle = tokio::spawn(async move {
            service.generate_and_check_wallet().await
        });

        // Store the handle for potential cancellation
        self.single_generation_handle = Some(handle);

        Ok(())
    }

    /// Start indefinite guessing
    pub fn start_indefinite_guessing(&mut self) -> Result<()> {
        if self.is_guessing {
            return Ok(()); // Already guessing
        }

        self.is_guessing = true;
        self.attempts = 0;
        self.current_wallet = None;
        self.found_wallet = None;
        self.reset_error_state();

        let (tx, rx) = mpsc::unbounded_channel();
        self.guessing_task = Some(rx);

        let wallet_gen = Arc::clone(&self.service.wallet_generator);
        let api_client = Arc::clone(&self.service.api_client);
        let stop_flag = Arc::new(AtomicBool::new(false));
        self.stop_flag = Some(stop_flag.clone());

        const NUM_WORKERS: usize = 8;
        
        for _ in 0..NUM_WORKERS {
            let stop_flag = stop_flag.clone();
            let result_sender = tx.clone();
            let wallet_gen = Arc::clone(&wallet_gen);
            let api_client = Arc::clone(&api_client);

            self.guessing_handles.push(tokio::spawn(async move {
                info!("Guessing worker started");
                loop {
                    // Check stop flag before generating wallet
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    
                    // Generate a new wallet
                    let wallet = match wallet_gen.generate_wallet() {
                        Ok(w) => w,
                        Err(_) => {
                            // Check stop flag after generation error
                            if stop_flag.load(Ordering::Relaxed) {
                                break;
                            }
                            continue; // Skip to next iteration on generation errors
                        }
                    };
                    
                    // Check stop flag before checking wallet
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    
                    // Check this wallet completely before moving to the next one
                    match WalletGuessingService::check_wallet_with_retries(
                        Arc::clone(&api_client),
                        wallet,
                        Some(&stop_flag),
                    ).await {
                        Ok(checked_wallet) => {
                            // Check stop flag before sending result
                            if stop_flag.load(Ordering::Relaxed) {
                                break;
                            }
                            let _ = result_sender.send(Ok(checked_wallet));
                        }
                        Err(_) => {
                            // Operation was stopped or failed - break from wallet generation loop
                            break;
                        }
                    }
                }
                info!("Guessing worker stopped");
            }));
        }

        info!("Started {} parallel guessing workers", NUM_WORKERS);

        Ok(())
    }

    /// Stop indefinite guessing
    pub fn stop_indefinite_guessing(&mut self) {
        self.is_guessing = false;
        if let Some(flag) = &self.stop_flag {
            flag.store(true, Ordering::Relaxed);
        }
        for handle in self.guessing_handles.drain(..) {
            handle.abort();
        }
        self.guessing_task = None;
        self.stop_flag = None;
    }

    /// Process any pending results from indefinite guessing
    pub async fn process_guessing_results(&mut self) {
        // Process single wallet generation results
        if let Some(ref mut handle) = self.single_generation_handle {
            if handle.is_finished() {
                self.is_generating_single = false;
                match handle.await {
                    Ok(Ok(wallet)) => {
                        self.attempts += 1;
                        if wallet.has_balance() {
                            self.handle_wallet_with_balance(wallet);
                        } else {
                            self.current_wallet = Some(wallet);
                        }
                    }
                    Ok(Err(e)) => {
                        self.last_error = Some(e.to_string());
                    }
                    Err(e) => {
                        // Task was aborted (cancelled)
                        let e: &tokio::task::JoinError = &e;
                        if !e.is_cancelled() {
                            self.last_error = Some(e.to_string());
                        }
                    }
                }
                self.single_generation_handle = None;
            }
        }

        // Process indefinite guessing results
        const MAX_RESULTS_PER_TICK: usize = 32;
        let mut results = Vec::with_capacity(MAX_RESULTS_PER_TICK);

        if let Some(ref mut receiver) = self.guessing_task {
            for _ in 0..MAX_RESULTS_PER_TICK {
                match receiver.try_recv() {
                    Ok(result) => results.push(result),
                    Err(_) => break,
                }
            }
        }

        for result in results {
            match result {
                Ok(wallet) => {
                    self.attempts += 1;
                    if wallet.has_balance() {
                        self.handle_wallet_with_balance(wallet);
                        self.stop_indefinite_guessing();
                        break;
                    } else {
                        self.current_wallet = Some(wallet);
                    }
                }
                Err(e) => {
                    self.last_error = Some(e.to_string());
                    // Continue guessing despite errors
                }
            }
        }
    }

    /// Toggle word count in configuration
    pub async fn toggle_word_count(&mut self) -> Result<()> {
        let new_word_count = match self.service.config().wallet.word_count {
            12 => 24,
            24 => 12,
            _ => unreachable!("word_count is validated to be 12 or 24"),
        };

        // Update config without full clone - modify in place where possible
        let mut new_config = (**self.service.config()).clone();
        new_config.wallet.word_count = new_word_count;

        self.service.update_config(Arc::new(new_config)).await?;
        info!("Switched to {} word mnemonics", new_word_count);
        Ok(())
    }

    /// Apply a mutation to ApiConfig, validate, and commit
    async fn update_api_config<F: FnOnce(&mut crate::core::config::ApiConfig)>(
        &mut self,
        mutate: F,
    ) -> Result<()> {
        let mut new_config = (**self.service.config()).clone();
        mutate(&mut new_config.api);
        new_config.api.validate()?;
        self.service.update_config(Arc::new(new_config)).await
    }

    /// Update API URL
    pub async fn update_api_url(&mut self, url: String) -> Result<()> {
        self.update_api_config(|api| api.base_url = url).await
    }

    /// Update Fallback API URL
    pub async fn update_fallback_api_url(&mut self, url: String) -> Result<()> {
        self.update_api_config(|api| {
            let trimmed = url.trim();
            api.fallback_base_url = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        })
        .await
    }

    /// Handle successful wallet discovery
    fn handle_wallet_with_balance(&mut self, wallet: WalletInfo) {
        // Log discovery without sensitive data (mnemonic/keys)
        info!(
            "Found wallet with balance! Address: {}, Balance: {} sat",
            wallet.primary_address(),
            wallet.total_balance()
        );

        self.is_guessing = false;
        self.found_wallet = Some(wallet);
    }

    /// Reset error state
    fn reset_error_state(&mut self) {
        self.last_error = None;
    }

    // Getters for UI
    pub fn current_wallet(&self) -> Option<&WalletInfo> {
        self.current_wallet.as_ref()
    }

    pub fn found_wallet(&self) -> Option<&WalletInfo> {
        self.found_wallet.as_ref()
    }

    pub fn attempts(&self) -> u64 {
        self.attempts
    }

    pub fn is_guessing(&self) -> bool {
        self.is_guessing
    }

    pub fn is_generating_single(&self) -> bool {
        self.is_generating_single
    }

    /// Check if any operation is currently running
    pub fn has_active_operations(&self) -> bool {
        self.is_guessing || self.is_generating_single()
    }

    /// Cancel single wallet generation
    pub fn cancel_single_wallet_generation(&mut self) -> Result<()> {
        if let Some(handle) = self.single_generation_handle.take() {
            handle.abort();
        }
        self.is_generating_single = false;
        self.reset_error_state();
        Ok(())
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

}
