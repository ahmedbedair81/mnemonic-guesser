//! Benchmarks for wallet generation performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mnemonic_guesser::core::config::Config;
use mnemonic_guesser::core::wallet::WalletGenerator;
use std::sync::Arc;

fn bench_wallet_generation_12_words(c: &mut Criterion) {
    let mut config = Config::default();
    config.wallet.word_count = 12;
    let config = Arc::new(config);
    let generator = WalletGenerator::new(config);

    c.bench_function("wallet_generation_12_words", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                black_box(generator.generate_wallet().await.unwrap())
            });
        });
    });
}

fn bench_wallet_generation_24_words(c: &mut Criterion) {
    let mut config = Config::default();
    config.wallet.word_count = 24;
    let config = Arc::new(config);
    let generator = WalletGenerator::new(config);

    c.bench_function("wallet_generation_24_words", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                black_box(generator.generate_wallet().await.unwrap())
            });
        });
    });
}

fn bench_address_derivation(c: &mut Criterion) {
    let config = Arc::new(Config::default());
    let generator = WalletGenerator::new(config);

    c.bench_function("address_derivation", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mnemonic = generator.generate_random_mnemonic().unwrap();
                black_box(generator.derive_all_addresses(&mnemonic).unwrap())
            });
        });
    });
}

criterion_group!(
    benches,
    bench_wallet_generation_12_words,
    bench_wallet_generation_24_words,
    bench_address_derivation
);
criterion_main!(benches);