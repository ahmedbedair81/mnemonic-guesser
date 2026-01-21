//! UI state management separate from presentation logic.
//!
//! This module handles the state of the UI components,
//! keeping presentation logic separate from business logic.

use crate::core::config::Config;
use crate::core::wallet::WalletInfo;

/// UI state that mirrors the application state for display purposes
#[derive(Debug, Clone)]
pub struct UiState {
    pub current_wallet: Option<WalletInfo>,
    pub found_wallet: Option<WalletInfo>,
    pub attempts: u64,
    pub is_guessing: bool,
    pub is_generating_single: bool,
    pub last_error: Option<String>,
    pub config: std::sync::Arc<Config>,
    pub api_url_input: String,
    pub fallback_api_url_input: String,
}

impl UiState {
    pub fn new(config: std::sync::Arc<Config>) -> Self {
        Self {
            current_wallet: None,
            found_wallet: None,
            attempts: 0,
            is_guessing: false,
            is_generating_single: false,
            last_error: None,
            api_url_input: config.api.base_url.clone(),
            fallback_api_url_input: config.api.fallback_base_url.clone().unwrap_or_default(),
            config,
        }
    }

    /// Update the UI state from application state
    pub fn update_from_app_state(
        &mut self,
        current_wallet: Option<WalletInfo>,
        found_wallet: Option<WalletInfo>,
        attempts: u64,
        is_guessing: bool,
        is_generating_single: bool,
        last_error: Option<String>,
    ) {
        self.current_wallet = current_wallet;
        self.found_wallet = found_wallet;
        self.attempts = attempts;
        self.is_guessing = is_guessing;
        self.is_generating_single = is_generating_single;
        self.last_error = last_error;
    }

}