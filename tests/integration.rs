//! Integration tests for the mnemonic wallet guesser.
//!
//! These tests verify that the components work together correctly.

use mnemonic_guesser::core::config::Config;
use mnemonic_guesser::core::wallet::{WalletGenerator, WalletInfo};
use mnemonic_guesser::infrastructure::api::ApiClient;
use std::sync::Arc;

#[tokio::test]
async fn test_wallet_generation_and_api_client_creation() {
    let config = Arc::new(Config::default());
    let generator = WalletGenerator::new(Arc::clone(&config));

    // Generate a wallet
    let wallet = generator.generate_wallet().await.unwrap();
    assert_eq!(wallet.addresses.len(), 4);
    assert!(!wallet.primary_address().is_empty());

    // Create API client
    let api_client = ApiClient::new(Arc::clone(&config)).await.unwrap();

    // Verify API client has correct config
    assert_eq!(api_client.config().api.base_url, config.api.base_url);
}

#[tokio::test]
async fn test_wallet_with_known_mnemonic() {
    let config = Arc::new(Config::default());
    let generator = WalletGenerator::new(Arc::clone(&config));

    // Use a known mnemonic for deterministic testing
    let known_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic = bip39::Mnemonic::from_phrase(known_mnemonic, bip39::Language::English).unwrap();

    let addresses = generator.derive_all_addresses(&mnemonic).unwrap();
    let wallet = WalletInfo::new(&mnemonic, addresses);

    // Verify the wallet was created correctly
    assert_eq!(wallet.word_count, 12);
    assert_eq!(wallet.addresses.len(), 4);

    // Check that addresses are derived correctly (this is a deterministic test)
    for address_info in &wallet.addresses {
        assert!(!address_info.address.is_empty());
        assert!(!address_info.private_key.is_empty());
        assert!(address_info.address.starts_with("1") || address_info.address.starts_with("3") || address_info.address.starts_with("bc1"));
    }
}

#[test]
fn test_config_validation_integration() {
    let mut config = Config::default();

    // Test valid config
    assert!(config.validate().is_ok());

    // Test invalid network
    config.wallet.network = "invalid_network".to_string();
    assert!(config.validate().is_err());

    // Reset and test invalid URL
    config = Config::default();
    config.api.base_url = "not-a-url".to_string();
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_api_client_error_handling() {
    let mut config = Config::default();
    config.api.base_url = "http://invalid-url-that-does-not-exist.com".to_string();
    let config = Arc::new(config);

    let api_client = ApiClient::new(config).await.unwrap();

    // This should fail with network error
    let result = api_client.check_wallet_balance("invalid_address").await;
    assert!(result.is_err());
}

#[test]
fn test_wallet_info_serialization() {
    let config = Arc::new(Config::default());
    let generator = WalletGenerator::new(Arc::clone(&config));

    let mnemonic = generator.generate_random_mnemonic().unwrap();
    let addresses = generator.derive_all_addresses(&mnemonic).unwrap();
    let wallet = WalletInfo::new(&mnemonic, addresses);

    // Test serialization (WalletInfo implements Serialize)
    let serialized = serde_json::to_string(&wallet).unwrap();
    assert!(!serialized.is_empty());

    // Test deserialization
    let deserialized: WalletInfo = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.mnemonic, wallet.mnemonic);
    assert_eq!(deserialized.addresses.len(), wallet.addresses.len());
}