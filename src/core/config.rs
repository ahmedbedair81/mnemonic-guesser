use crate::core::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main application configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api: ApiConfig,
    pub wallet: WalletConfig,
    pub ui: UiConfig,
}

/// API-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Base URL for the blockchain API (default: blockstream)
    pub base_url: String,
    /// Fallback base URL (leave blank/None for no fallback, e.g. mempool.space)
    pub fallback_base_url: Option<String>,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Delay between requests in milliseconds (rate limiting)
    pub rate_limit_delay_ms: u64,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
}

/// Wallet generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Blockchain network (bitcoin, testnet, signet, regtest)
    pub network: String,
    /// Default BIP32 derivation path
    pub derivation_path: String,
    /// Number of words in generated mnemonics (12 or 24)
    pub word_count: usize,
}

/// UI layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Width of labels in the UI
    pub label_width: f32,
    /// Width of values in the UI
    pub value_width: f32,
    /// Height of current wallet display area
    pub current_wallet_height: f32,
    /// Height of found wallet display area
    pub found_wallet_height: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            wallet: WalletConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration with validation
    /// 
    /// # Panics
    /// 
    /// Panics if the default configuration fails validation. This should never happen
    /// with the default configuration, but could occur if default values are changed.
    pub fn load() -> Self {
        let config = Self::default();

        // Validate the configuration
        config.validate().unwrap_or_else(|e| {
            panic!("Invalid default configuration: {}. This indicates a bug in the default configuration.", e)
        });

        config
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<()> {
        self.api.validate()?;
        self.wallet.validate()?;
        self.ui.validate()?;
        Ok(())
    }

    /// Get API timeout as Duration
    pub fn api_timeout(&self) -> Duration {
        Duration::from_secs(self.api.timeout_seconds)
    }

    /// Get rate limit delay as Duration
    pub fn rate_limit_delay(&self) -> Duration {
        Duration::from_millis(self.api.rate_limit_delay_ms)
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://mempool.space/api".to_string(),
            fallback_base_url: Some("https://blockstream.info/api".to_string()),
            timeout_seconds: 30,
            rate_limit_delay_ms: 500,
            max_retries: 3,
        }
    }
}

impl ApiConfig {
    /// Validate API configuration
    pub fn validate(&self) -> Result<()> {
        if self.base_url.is_empty() {
            return Err(AppError::Config {
                field: "api.base_url".to_string(),
                value: None,
                reason: "Base URL cannot be empty".to_string(),
            });
        }

        if !self.base_url.starts_with("http") {
            return Err(AppError::Config {
                field: "api.base_url".to_string(),
                value: None,
                reason: "Base URL must start with http:// or https://".to_string(),
            });
        }

        if self.timeout_seconds == 0 {
            return Err(AppError::Config {
                field: "api.timeout_seconds".to_string(),
                value: Some(self.timeout_seconds.to_string()),
                reason: "Timeout must be greater than 0".to_string(),
            });
        }

        if self.max_retries == 0 {
            return Err(AppError::Config {
                field: "api.max_retries".to_string(),
                value: Some(self.max_retries.to_string()),
                reason: "Max retries must be greater than 0".to_string(),
            });
        }

        if let Some(fallback) = &self.fallback_base_url {
            if fallback.is_empty() {
                return Err(AppError::Config {
                    field: "api.fallback_base_url".to_string(),
                    value: Some("".to_string()),
                    reason: "Fallback base URL cannot be empty if set".to_string(),
                });
            }
            if !fallback.starts_with("http") {
                return Err(AppError::Config {
                    field: "api.fallback_base_url".to_string(),
                    value: None,
                    reason: "Fallback base URL must start with http:// or https://".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            derivation_path: "m/44'/0'/0'/0/0".to_string(),
            network: "bitcoin".to_string(),
            word_count: 24, // Default to 24 words
        }
    }
}

impl WalletConfig {
    /// Validate wallet configuration
    pub fn validate(&self) -> Result<()> {
        // Validate network (values are stored lowercase by convention)
        match self.network.as_str() {
            "bitcoin" | "mainnet" | "testnet" | "signet" | "regtest" => {}
            _ => {
                return Err(AppError::Config {
                    field: "wallet.network".to_string(),
                    value: Some(self.network.clone()),
                    reason: "Network must be one of: bitcoin, testnet, signet, regtest".to_string(),
                });
            }
        }

        // Validate word count
        match self.word_count {
            12 | 24 => {}
            _ => {
                return Err(AppError::Config {
                    field: "wallet.word_count".to_string(),
                    value: Some(self.word_count.to_string()),
                    reason: "Word count must be 12 or 24".to_string(),
                });
            }
        }

        // Validate derivation path (basic check)
        if !self.derivation_path.starts_with('m') {
            return Err(AppError::Config {
                field: "wallet.derivation_path".to_string(),
                value: Some(self.derivation_path.clone()),
                reason: "Derivation path must start with 'm'".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            label_width: 140.0,
            value_width: 400.0,
            current_wallet_height: 200.0,
            found_wallet_height: 250.0,
        }
    }
}

/// Validate that a UI dimension value is positive.
fn validate_positive(value: f32, field: &str, label: &str) -> Result<()> {
    if value <= 0.0 {
        return Err(AppError::Config {
            field: field.to_string(),
            value: Some(value.to_string()),
            reason: format!("{} must be positive", label),
        });
    }
    Ok(())
}

impl UiConfig {
    /// Validate UI configuration
    pub fn validate(&self) -> Result<()> {
        validate_positive(self.label_width, "ui.label_width", "Label width")?;
        validate_positive(self.value_width, "ui.value_width", "Value width")?;
        validate_positive(self.current_wallet_height, "ui.current_wallet_height", "Current wallet height")?;
        validate_positive(self.found_wallet_height, "ui.found_wallet_height", "Found wallet height")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_network() {
        let mut config = Config::default();
        config.wallet.network = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_word_count() {
        let mut config = Config::default();
        config.wallet.word_count = 18;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_api_url() {
        let mut config = Config::default();
        config.api.base_url = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_with_fallback_url() {
        let mut config = Config::default();
        config.api.fallback_base_url = Some("https://fallback.example.com".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_invalid_fallback_url() {
        let mut config = Config::default();
        config.api.fallback_base_url = Some("not-a-url".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_empty_fallback_url() {
        let mut config = Config::default();
        config.api.fallback_base_url = Some("".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_api_timeout_duration() {
        let config = Config::default();
        let timeout = config.api_timeout();
        assert_eq!(timeout.as_secs(), 30);
    }

    #[test]
    fn test_rate_limit_delay_duration() {
        let config = Config::default();
        let delay = config.rate_limit_delay();
        assert_eq!(delay.as_millis(), 500);
    }

    #[test]
    fn test_ui_config_validation() {
        let ui_config = UiConfig::default();
        assert!(ui_config.validate().is_ok());
    }

    #[test]
    fn test_ui_config_negative_width() {
        let mut ui_config = UiConfig::default();
        ui_config.label_width = -10.0;
        assert!(ui_config.validate().is_err());
    }
}
