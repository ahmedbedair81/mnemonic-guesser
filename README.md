# Mnemonic Wallet Guesser

A high-performance tool for generating and checking Bitcoin wallet balances from BIP39 mnemonics.

## 🚀 Features

- **Fast Wallet Generation**: Generates random BIP39 mnemonics with support for 12 and 24 word phrases
- **Multi-Address Checking**: Checks multiple address types (Legacy, SegWit, Native SegWit, Taproot)
- **Concurrent Processing**: Uses parallel workers for maximum performance
- **Rate Limiting**: Respects API limits with configurable delays
- **Fallback APIs**: Supports multiple blockchain APIs for reliability
- **Real-time UI**: Modern Iced-based GUI with live progress updates

## 📋 Requirements

- Rust 1.70+
- Linux/macOS/Windows

## 🛠️ Installation

```bash
# Clone the repository
git clone https://github.com/bedair81/mnemonic-guesser.git
cd mnemonic-wallet-guesser

# Build the application
cargo build --release
```

## 🚀 Usage

### GUI Mode (Default)

```bash
cargo run --release
```

### Command Line Options

```bash
# Generate a single wallet and check balance
cargo run -- --single

# Start indefinite guessing with custom word count
cargo run -- --guess --words 24

# Use custom API endpoint
cargo run -- --api-url https://blockstream.info/api
```

## 🏗️ Architecture

The application follows a clean architecture pattern with clear separation of concerns:

- **Core**: Domain logic (wallet generation, configuration)
- **Infrastructure**: External dependencies (API clients)
- **Application**: Business logic and use cases
- **Presentation**: UI components and state management

### Key Components

- **Wallet Generator**: Creates BIP39 mnemonics and derives addresses
- **API Client**: Concurrent balance checking with retry logic
- **State Manager**: Orchestrates wallet generation and checking
- **UI**: Real-time progress display and controls

## ⚙️ Configuration

The application uses a configuration file with the following options:

```toml
[api]
base_url = "https://mempool.space/api"
fallback_base_url = "https://blockstream.info/api"
timeout_seconds = 30
rate_limit_delay_ms = 500
max_retries = 3

[wallet]
network = "bitcoin"
derivation_path = "m/44'/0'/0'/0/0"
word_count = 24

[ui]
label_width = 140.0
value_width = 400.0
current_wallet_height = 200.0
found_wallet_height = 250.0
```

## 🔒 Security Notice

This tool is for educational and research purposes. Generating and checking random wallets may have legal implications in some jurisdictions. Always ensure compliance with local laws and regulations.

**Important**: The tool displays private keys when wallets with balances are found. Never share or store these keys insecurely.

## 🧪 Testing

```bash
# Run unit tests
cargo test

# Run with test coverage
cargo tarpaulin

# Run integration tests
cargo test --test integration
```

## 📊 Performance

- **Concurrent Workers**: 8 parallel wallet generation/checking threads
- **Batch API Calls**: Up to 32 concurrent address balance checks
- **Optimized Crypto**: Reused Secp256k1 context for address derivation
- **Connection Pooling**: HTTP client with persistent connections

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This software is provided "as is" without warranty of any kind. Use at your own risk. The authors are not responsible for any financial losses or legal issues arising from the use of this software.

## 🔗 Related Projects

- [bitcoin-core/secp256k1](https://github.com/bitcoin-core/secp256k1) - Cryptographic library
- [bip39](https://github.com/rust-bitcoin/bip39) - BIP39 mnemonic implementation
- [iced](https://github.com/iced-rs/iced) - GUI framework