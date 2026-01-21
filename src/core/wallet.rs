use crate::core::error::AppError;
use bip39::Mnemonic;
use bitcoin::bip32::{DerivationPath, Xpriv, Xpub};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Comprehensive wallet information with multiple address types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    /// The mnemonic phrase (BIP39)
    pub mnemonic: String,

    /// All derived addresses with their information
    pub addresses: Vec<AddressInfo>,

    /// Cached total balance across all addresses
    pub total_balance: u64,

    /// Number of failed balance checks across addresses
    pub num_failed_checks: usize,

    /// Number of words in the mnemonic
    pub word_count: usize,

    /// Seed bytes (cached for performance, not serialized)
    #[serde(skip)]
    #[allow(dead_code)]
    seed: Vec<u8>,
}

impl WalletInfo {
    /// Create a new wallet with the given mnemonic and addresses
    pub fn new(mnemonic: &Mnemonic, addresses: Vec<AddressInfo>) -> Self {
        let word_count = mnemonic.word_count();
        let total_balance = addresses.iter().map(|a| a.balance).sum();
        let num_failed_checks = 0usize;
        let seed = mnemonic.to_seed("").to_vec();

        Self {
            mnemonic: mnemonic.to_string(),
            addresses,
            total_balance,
            num_failed_checks,
            word_count,
            seed,
        }
    }

    /// Get the primary address (first one, typically P2PKH)
    pub fn primary_address(&self) -> &str {
        self.addresses
            .first()
            .map(|a| a.address.as_str())
            .unwrap_or("")
    }

    /// Get the primary private key (first one, typically P2PKH)
    #[allow(dead_code)]
    pub fn primary_private_key(&self) -> &str {
        self.addresses
            .first()
            .map(|a| a.private_key.as_str())
            .unwrap_or("")
    }

    /// Get the total balance across all addresses
    pub fn total_balance(&self) -> u64 {
        self.total_balance
    }

    /// Get all addresses for this wallet
    #[allow(dead_code)]
    pub fn all_addresses(&self) -> &[AddressInfo] {
        &self.addresses
    }


    /// Check if wallet has any balance
    pub fn has_balance(&self) -> bool {
        self.total_balance > 0
    }

}

/// Information about a single derived address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    /// The address string
    pub address: String,

    /// WIF-encoded private key
    pub private_key: String,

    /// BIP32 derivation path used
    pub derivation_path: String,

    /// Human-readable address type (e.g., "P2PKH (Legacy)")
    pub address_type: String,

    /// Current balance in satoshis
    pub balance: u64,

    /// Whether balance check succeeded
    pub checked: bool,
}

impl AddressInfo {
    /// Create a new address info
    #[allow(dead_code)]
    pub fn new(
        address: String,
        private_key: String,
        derivation_path: String,
        address_type: String,
    ) -> Self {
        Self {
            address,
            private_key,
            derivation_path,
            address_type,
            balance: 0,
            checked: false,
        }
    }

    /// Check if this address has a balance
    #[allow(dead_code)]
    pub fn has_balance(&self) -> bool {
        self.balance > 0
    }
}

pub struct WalletGenerator {
    config: std::sync::Arc<super::config::Config>,
}

impl WalletGenerator {
    pub fn new(config: std::sync::Arc<super::config::Config>) -> Self {
        Self { config }
    }

    pub fn generate_random_mnemonic(&self) -> crate::core::error::Result<Mnemonic> {
        // Cache entropy calculation to avoid repeated computation
        let entropy_bytes = match self.config.wallet.word_count {
            12 => 16, // 128 bits = 16 bytes
            24 => 32, // 256 bits = 32 bytes
            _ => {
                return Err(AppError::Wallet {
                    message: format!("Unsupported word count: {}", self.config.wallet.word_count),
                    details: Some(format!("word count: {}", self.config.wallet.word_count)),
                })
            }
        };

        // Reuse entropy buffer allocation
        let mut entropy = vec![0u8; entropy_bytes];
        rand::thread_rng().fill_bytes(&mut entropy);

        Mnemonic::from_entropy(&entropy).map_err(|e| AppError::Wallet {
            message: format!("Failed to generate mnemonic: {}", e),
            details: Some(format!("word count: {}", self.config.wallet.word_count)),
        })
    }

    pub async fn generate_wallet(&self) -> crate::core::error::Result<WalletInfo> {
        let mnemonic = self.generate_random_mnemonic()?;
        let addresses = self.derive_all_addresses(&mnemonic)?;
        Ok(WalletInfo::new(&mnemonic, addresses))
    }

    pub fn derive_all_addresses(&self, mnemonic: &Mnemonic) -> crate::core::error::Result<Vec<AddressInfo>> {
        const NUM_ADDRESS_TYPES: usize = 4;

        let seed = mnemonic.to_seed("");
        let network = self.parse_network()?;
        let mut addresses = Vec::with_capacity(NUM_ADDRESS_TYPES);

        // Reuse Secp256k1 context for all derivations (significant performance improvement)
        let secp = bitcoin::secp256k1::Secp256k1::new();

        // Pre-compute master key once for all address types
        let master_key = Xpriv::new_master(network, &seed).map_err(|e| AppError::Wallet {
            message: format!("Failed to create master key: {}", e),
            details: None,
        })?;

        // Common derivation paths and address types
        let derivation_schemes = [
            ("m/44'/0'/0'/0/0", "P2PKH (Legacy)"),
            ("m/49'/0'/0'/0/0", "P2SH-P2WPKH (SegWit)"),
            ("m/84'/0'/0'/0/0", "P2WPKH (Native SegWit)"),
            ("m/86'/0'/0'/0/0", "P2TR (Taproot)"),
        ];

        for (path_str, address_type) in derivation_schemes {
            let derivation_path: DerivationPath =
                path_str.parse().map_err(|e| AppError::Wallet {
                    message: format!("Invalid derivation path {}: {}", path_str, e),
                    details: Some(format!("path: {}, type: {}", path_str, address_type)),
                })?;

            let child_key = master_key
                .derive_priv(&secp, &derivation_path)
                .map_err(|e| AppError::Wallet {
                    message: format!("Failed to derive child key for {}: {}", path_str, e),
                    details: Some(format!("path: {}, type: {}", path_str, address_type)),
                })?;

            let pub_key = Xpub::from_priv(&secp, &child_key);
            let bitcoin_pub_key = PublicKey::from(pub_key.public_key);

            let address = match path_str {
                "m/44'/0'/0'/0/0" => Address::p2pkh(bitcoin_pub_key, network),
                "m/49'/0'/0'/0/0" => {
                    // For P2SH-P2WPKH, we need to create a script hash
                    let script = bitcoin::blockdata::script::Builder::new()
                        .push_int(0)
                        .push_slice(bitcoin_pub_key.pubkey_hash())
                        .into_script();
                    Address::p2sh(&script, network).map_err(|e| AppError::Wallet {
                        message: format!("Failed to create P2SH address: {}", e),
                        details: Some(format!("path: {}, type: {}", path_str, address_type)),
                    })?
                }
                "m/84'/0'/0'/0/0" => {
                    // P2WPKH expects CompressedPublicKey - create from serialized bytes
                    let compressed_bytes = bitcoin_pub_key.inner.serialize();
                    let compressed_pub_key = bitcoin::CompressedPublicKey::from_slice(
                        &compressed_bytes,
                    )
                    .map_err(|e| AppError::Wallet {
                        message: format!("Failed to create compressed public key: {}", e),
                        details: Some(format!("path: {}, type: {}", path_str, address_type)),
                    })?;
                    Address::p2wpkh(&compressed_pub_key, network)
                }
                "m/86'/0'/0'/0/0" => {
                    // For Taproot, we create a taproot address using the public key
                    let x_only_pubkey =
                        bitcoin::secp256k1::XOnlyPublicKey::from(bitcoin_pub_key.inner);
                    Address::p2tr(&secp, x_only_pubkey, None, network)
                }
                _ => {
                    return Err(AppError::Wallet {
                        message: format!("Unsupported derivation path: {}", path_str),
                        details: Some(format!("path: {}, type: {}", path_str, address_type)),
                    })
                }
            };

            let private_key = PrivateKey::new(child_key.private_key, network);
            let wif = private_key.to_wif();

            addresses.push(AddressInfo {
                address: address.to_string(),
                private_key: wif,
                derivation_path: path_str.to_string(),
                address_type: address_type.to_string(),
                balance: 0, // Will be checked later
                checked: false,
            });
        }

        Ok(addresses)
    }

    fn parse_network(&self) -> crate::core::error::Result<Network> {
        // Use direct string comparison instead of to_lowercase() for performance
        match self.config.wallet.network.as_str() {
            "bitcoin" => Ok(Network::Bitcoin),
            "mainnet" => Ok(Network::Bitcoin),
            "testnet" => Ok(Network::Testnet),
            "signet" => Ok(Network::Signet),
            "regtest" => Ok(Network::Regtest),
            _ => Err(AppError::Wallet {
                message: format!("Unsupported network: {}", self.config.wallet.network),
                details: Some(format!("network: {}", self.config.wallet.network)),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;

    #[test]
    fn test_generate_24_word_mnemonic() {
        let mut config = Config::default();
        config.wallet.word_count = 24;
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let mnemonic_string = mnemonic.to_string();
        let words: Vec<&str> = mnemonic_string.split_whitespace().collect();
        assert_eq!(words.len(), 24);
    }

    #[test]
    fn test_generate_12_word_mnemonic() {
        let mut config = Config::default();
        config.wallet.word_count = 12;
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let mnemonic_string = mnemonic.to_string();
        let words: Vec<&str> = mnemonic_string.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }

    #[test]
    fn test_comprehensive_address_derivation() {
        let config = Config::default();
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let addresses = generator.derive_all_addresses(&mnemonic).unwrap();

        // Should generate 4 different address types
        assert_eq!(addresses.len(), 4);

        // Check that we have different address types
        let address_types: Vec<&str> = addresses.iter().map(|a| a.address_type.as_str()).collect();
        assert!(address_types.contains(&"P2PKH (Legacy)"));
        assert!(address_types.contains(&"P2SH-P2WPKH (SegWit)"));
        assert!(address_types.contains(&"P2WPKH (Native SegWit)"));
        assert!(address_types.contains(&"P2TR (Taproot)"));

        // Check that all addresses are valid and different
        for address_info in &addresses {
            assert!(!address_info.address.is_empty());
            assert!(!address_info.private_key.is_empty());
            assert!(!address_info.derivation_path.is_empty());
        }

        // Check that addresses are actually different
        let unique_addresses: std::collections::HashSet<_> =
            addresses.iter().map(|a| &a.address).collect();
        assert_eq!(unique_addresses.len(), 4);
    }

    #[test]
    fn test_wallet_info_creation() {
        let config = Config::default();
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let addresses = generator.derive_all_addresses(&mnemonic).unwrap();
        let wallet = WalletInfo::new(&mnemonic, addresses);

        assert_eq!(wallet.word_count, 24);
        assert_eq!(wallet.addresses.len(), 4);
        assert!(!wallet.primary_address().is_empty());
        assert!(!wallet.primary_private_key().is_empty());
    }

    #[test]
    fn test_wallet_balance_calculation() {
        let config = Config::default();
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let mut addresses = generator.derive_all_addresses(&mnemonic).unwrap();

        // Set some balances
        addresses[0].balance = 1000;
        addresses[1].balance = 2000;

        let wallet = WalletInfo::new(&mnemonic, addresses);
        assert_eq!(wallet.total_balance(), 3000);
        assert!(wallet.has_balance());
    }

    #[test]
    fn test_wallet_no_balance() {
        let config = Config::default();
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let mnemonic = generator.generate_random_mnemonic().unwrap();
        let addresses = generator.derive_all_addresses(&mnemonic).unwrap();
        let wallet = WalletInfo::new(&mnemonic, addresses);

        assert_eq!(wallet.total_balance(), 0);
        assert!(!wallet.has_balance());
    }

    #[test]
    fn test_network_parsing() {
        let config = Config::default();
        let generator = WalletGenerator::new(std::sync::Arc::new(config));

        assert_eq!(generator.parse_network().unwrap(), bitcoin::Network::Bitcoin);
    }

    #[test]
    fn test_invalid_word_count() {
        let mut config = Config::default();
        config.wallet.word_count = 18; // Invalid word count
        let generator = WalletGenerator::new(std::sync::Arc::new(config));
        let result = generator.generate_random_mnemonic();
        assert!(result.is_err());
    }
}
