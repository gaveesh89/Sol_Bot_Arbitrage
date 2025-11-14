// Solana MEV Bot Library
//
// This library provides components for building high-performance arbitrage bots
// on Solana, including:
// - Multi-DEX pool data fetching (Raydium, Orca, Meteora, Pump.fun, Phoenix)
// - Triangular arbitrage detection using Bellman-Ford algorithm
// - Real-time WebSocket pool monitoring
// - Transaction execution and retry logic
// - Trade analytics and reporting

pub mod chain;
pub mod config;
pub mod data;
pub mod dex;
pub mod meteora;
pub mod reporting;
pub mod utils;
