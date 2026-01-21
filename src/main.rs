//! Mnemonic Wallet Guesser - A tool for generating and checking mnemonic wallet balances.
//!
//! This application generates random BIP39 mnemonics and checks if the derived
//! addresses have balances on the blockchain.

mod application;
mod core;
mod infrastructure;
mod presentation;
mod utils;

use crate::core::config::Config;
use crate::application::services::AppState;
use crate::presentation::ui::Message;
use crate::presentation::ui_state::UiState;
use iced::{Application, Command, Element, Settings, Subscription, Theme};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::runtime::Runtime;
use tracing::info;

// Common error messages to avoid allocations
const ERR_BUSY_OPERATION: &str = "Another operation is already in progress";
const ERR_OPERATION_BUSY: &str = "busy";

/// Main application struct
pub struct MnemonicGuesser {
    app_state: Arc<RwLock<AppState>>,
    ui_state: UiState,
    runtime: Runtime,
}

impl MnemonicGuesser {
    /// Create a new application instance
    fn new() -> (Self, Command<Message>) {
        let config = Arc::new(Config::load());

        // Initialize tracing
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        info!(
            "🚀 Starting Mnemonic Wallet Guesser v{}",
            env!("CARGO_PKG_VERSION")
        );
        info!("📋 Using {} API for balance checking", config.api.base_url);
        info!("🎯 Derivation path: {}", config.wallet.derivation_path);
        info!("🌐 Network: {}", config.wallet.network);

        // Create tokio runtime for async operations
        let runtime = Runtime::new().expect("Failed to create tokio runtime");

        let app_state = Arc::new(RwLock::new(
            runtime.block_on(async {
                AppState::new(Arc::clone(&config)).await.expect("Failed to initialize application state")
            }),
        ));
        let ui_state = UiState::new(Arc::clone(&config));

        let app = Self {
            app_state,
            ui_state,
            runtime,
        };

        (app, Command::none())
    }
}

impl Application for MnemonicGuesser {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        Self::new()
    }

    fn title(&self) -> String {
        // Use cached attempts from UI state to avoid blocking
        format!("Mnemonic Wallet Guesser - {} attempts", self.ui_state.attempts)
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::GenerateSingle => self.handle_generate_single(),
            Message::StartIndefinite => self.handle_start_indefinite(),
            Message::StopIndefinite => self.handle_stop_indefinite(),
            Message::CancelSingle => self.handle_cancel_single(),
            Message::ToggleWordCount => self.handle_toggle_word_count(),
            Message::ApiUrlChanged(url) => {
                self.ui_state.api_url_input = url.clone();
                self.handle_api_url_changed(url)
            }
            Message::FallbackApiUrlChanged(url) => {
                self.ui_state.fallback_api_url_input = url.clone();
                self.handle_fallback_api_url_changed(url)
            }
            Message::Tick => self.handle_tick(),
            Message::CopyToClipboard(text) => {
                // Note: iced doesn't have built-in clipboard support in all backends
                // This would need platform-specific implementation or a workaround
                info!("Copy to clipboard requested: {}", text);
                Command::none()
            }
            Message::WalletChecked(_) | Message::ErrorOccurred(_) => Command::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.ui_state.view()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Use cached state from UI to avoid blocking
        if self.ui_state.is_guessing || self.ui_state.is_generating_single {
            const TICK_INTERVAL_MS: u64 = 100;
            iced::time::every(Duration::from_millis(TICK_INTERVAL_MS)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }
}

impl MnemonicGuesser {
    /// Helper to handle async state operations with common error handling
    fn handle_async_operation<F>(&mut self, operation: F) -> Command<Message>
    where
        F: std::future::Future<Output = Result<(), crate::core::error::AppError>> + Send + 'static,
    {
        Command::perform(operation, |result| match result {
            Ok(_) => Message::Tick,
            Err(e) => Message::ErrorOccurred(e.to_string()),
        })
    }

    /// Handle single wallet generation
    fn handle_generate_single(&mut self) -> Command<Message> {
        info!("Generate Single button clicked!");
        let app_state = Arc::clone(&self.app_state);

        // Start the generation process asynchronously
        let result = self.runtime.block_on(async {
            let mut state = app_state.write().await;
            if state.has_active_operations() {
                return Err(crate::core::error::AppError::Validation {
                    field: "operation".to_string(),
                    value: "busy".to_string(),
                    reason: "Another operation is already in progress".to_string(),
                });
            }

            state.generate_single_wallet().await
        });

        match result {
            Ok(_) => {
                // Trigger immediate tick to process results
                self.handle_tick()
            }
            Err(e) => {
                info!("Error starting generation: {}", e);
                Command::none()
            }
        }
    }

    /// Handle canceling single wallet generation
    fn handle_cancel_single(&mut self) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        self.handle_async_operation(async move {
            let mut state = app_state.write().await;
            state.cancel_single_wallet_generation()
        })
    }

    /// Handle start of indefinite guessing
    fn handle_start_indefinite(&mut self) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        self.handle_async_operation(async move {
            let mut state = app_state.write().await;
            if state.has_active_operations() {
                return Err(crate::core::error::AppError::Validation {
                    field: "operation".to_string(),
                    value: ERR_OPERATION_BUSY.to_string(),
                    reason: ERR_BUSY_OPERATION.to_string(),
                });
            }
            state.start_indefinite_guessing()
        })
    }

    /// Handle stopping indefinite guessing
    fn handle_stop_indefinite(&mut self) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        tokio::spawn(async move {
            let mut state = app_state.write().await;
            state.stop_indefinite_guessing();
        });
        Command::none()
    }

    /// Handle word count toggle
    fn handle_toggle_word_count(&mut self) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        self.handle_async_operation(async move {
            let mut state = app_state.write().await;
            state.toggle_word_count().await
        })
    }

    /// Handle API URL change
    fn handle_api_url_changed(&mut self, url: String) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        self.handle_async_operation(async move {
            let mut state = app_state.write().await;
            state.update_api_url(url).await
        })
    }

    fn handle_fallback_api_url_changed(&mut self, url: String) -> Command<Message> {
        let app_state = Arc::clone(&self.app_state);
        self.handle_async_operation(async move {
            let mut state = app_state.write().await;
            state.update_fallback_api_url(url).await
        })
    }

    /// Handle periodic tick for processing background tasks
    fn handle_tick(&mut self) -> Command<Message> {
        // Process results with minimal lock time
        let (current_wallet, found_wallet, attempts, is_guessing, is_generating_single, last_error) =
            self.runtime.block_on(async {
                let mut state = self.app_state.write().await;
                state.process_guessing_results().await;

                // Extract all data while holding the lock
                (
                    state.current_wallet().cloned(),
                    state.found_wallet().cloned(),
                    state.attempts(),
                    state.is_guessing(),
                    state.is_generating_single(),
                    state.last_error().map(|s| s.to_string()),
                )
            });

        // Update UI state outside of lock
        self.ui_state.update_from_app_state(
            current_wallet,
            found_wallet,
            attempts,
            is_guessing,
            is_generating_single,
            last_error,
        );

        Command::none()
    }
}

fn main() -> iced::Result {
    MnemonicGuesser::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(1400.0, 900.0),
            min_size: Some(iced::Size::new(1000.0, 700.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let (_app, _) = MnemonicGuesser::new();
        // App should be created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_ui_state_creation() {
        use crate::presentation::ui_state::UiState;
        let config = std::sync::Arc::new(crate::core::config::Config::default());
        let ui_state = UiState::new(config);
        assert!(!ui_state.api_url_input.is_empty());
    }
}
