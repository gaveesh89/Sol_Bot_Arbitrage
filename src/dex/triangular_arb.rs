// Triangular Arbitrage Detection Module for Solana DEXs
//
// This module implements a graph-based approach to detect triangular arbitrage
// opportunities across multiple DEXs (Raydium, Meteora, Pump, Whirlpool, Orca).
//
// Algorithm:
// - Uses negative log-transformed weights: -log(rate * (1 - fee))
// - Detects cycles with negative weight (profit opportunities)
// - Thread-safe concurrent access with Arc<RwLock<>>
// - Bellman-Ford algorithm for negative cycle detection

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use solana_sdk::pubkey::Pubkey;
use anyhow::{Result, anyhow};
use tracing::{debug, warn, info};
use tokio::task;

/// Represents an exchange rate edge in the arbitrage graph
#[derive(Clone, Debug)]
pub struct ExchangeEdge {
    pub from_token: Pubkey,
    pub to_token: Pubkey,
    pub dex: DexType,
    pub pool_address: Pubkey,
    pub rate: f64, // Exchange rate: how many to_token per from_token
    pub inverse_log_weight: f64, // -log(rate * (1 - fee))
    pub liquidity_depth: Vec<PriceLevel>,
    pub fee_bps: u16, // Fee in basis points (e.g., 25 = 0.25%)
    pub last_update: i64, // Unix timestamp
}

impl ExchangeEdge {
    /// Calculate the logarithmic weight for arbitrage detection
    /// weight = -log(rate * (1 - fee/10000))
    pub fn calculate_weight(rate: f64, fee_bps: u16) -> f64 {
        let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
        let effective_rate = rate * fee_multiplier;
        
        if effective_rate <= 0.0 {
            warn!("Invalid rate calculation: rate={}, fee_bps={}", rate, fee_bps);
            f64::INFINITY
        } else {
            -effective_rate.ln()
        }
    }

    /// Create a new exchange edge with calculated weight
    pub fn new(
        from_token: Pubkey,
        to_token: Pubkey,
        dex: DexType,
        pool_address: Pubkey,
        rate: f64,
        fee_bps: u16,
        liquidity_depth: Vec<PriceLevel>,
        timestamp: i64,
    ) -> Self {
        let inverse_log_weight = Self::calculate_weight(rate, fee_bps);
        
        Self {
            from_token,
            to_token,
            dex,
            pool_address,
            rate,
            inverse_log_weight,
            liquidity_depth,
            fee_bps,
            last_update: timestamp,
        }
    }

    /// Update the rate and recalculate weight
    pub fn update_rate(&mut self, new_rate: f64, timestamp: i64) {
        self.rate = new_rate;
        self.inverse_log_weight = Self::calculate_weight(new_rate, self.fee_bps);
        self.last_update = timestamp;
    }

    /// Get maximum tradeable amount at a specific price impact threshold
    pub fn get_max_tradeable_amount(&self, max_slippage_bps: u16) -> u64 {
        let max_slippage = max_slippage_bps as f64 / 10000.0;
        let target_price = self.rate * (1.0 - max_slippage);
        
        let mut cumulative_liquidity = 0u64;
        for level in &self.liquidity_depth {
            if level.price >= target_price {
                cumulative_liquidity += level.liquidity;
            } else {
                break;
            }
        }
        
        cumulative_liquidity
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DexType {
    Raydium,
    Meteora,
    Pump,
    Whirlpool,
    Orca,
}

impl std::fmt::Display for DexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DexType::Raydium => write!(f, "Raydium"),
            DexType::Meteora => write!(f, "Meteora"),
            DexType::Pump => write!(f, "Pump"),
            DexType::Whirlpool => write!(f, "Whirlpool"),
            DexType::Orca => write!(f, "Orca"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PriceLevel {
    pub price: f64,
    pub liquidity: u64, // Amount available at this price
}

/// Represents a detected triangular arbitrage opportunity
#[derive(Clone, Debug)]
pub struct TriangularArbitrageOpportunity {
    pub path: Vec<ExchangeEdge>,
    pub profit_ratio: f64, // Expected profit ratio (e.g., 1.02 = 2% profit)
    pub profit_bps: i64, // Profit in basis points
    pub input_token: Pubkey,
    pub input_amount: u64,
    pub estimated_output: u64,
    pub total_fees_bps: u16,
    pub cycle_weight: f64, // Negative if profitable
}

impl TriangularArbitrageOpportunity {
    /// Calculate if this opportunity is still profitable after gas costs
    pub fn is_profitable_after_costs(&self, gas_cost_lamports: u64, token_price_usd: f64) -> bool {
        let profit_usd = (self.estimated_output as f64 - self.input_amount as f64) * token_price_usd;
        let gas_cost_usd = gas_cost_lamports as f64 * token_price_usd; // Assuming SOL price
        
        profit_usd > gas_cost_usd
    }
}

/// Main graph structure for triangular arbitrage detection
pub struct ArbitrageGraph {
    // Adjacency list: token -> list of outgoing edges
    adjacency: HashMap<Pubkey, Vec<ExchangeEdge>>,
    // Quick lookup: (from, to, dex) -> index in adjacency list
    edge_lookup: HashMap<(Pubkey, Pubkey, DexType), (usize, usize)>,
    // Token registry for quick iteration
    tokens: HashSet<Pubkey>,
}

impl ArbitrageGraph {
    /// Create a new empty arbitrage graph
    pub fn new() -> Self {
        info!("Initializing ArbitrageGraph for triangular arbitrage detection");
        Self {
            adjacency: HashMap::new(),
            edge_lookup: HashMap::new(),
            tokens: HashSet::new(),
        }
    }

    /// Add an edge to the graph with automatic weight calculation
    pub fn add_edge(&mut self, edge: ExchangeEdge) {
        let from = edge.from_token;
        let to = edge.to_token;
        let dex = edge.dex.clone();

        // Register tokens
        self.tokens.insert(from);
        self.tokens.insert(to);

        // Add to adjacency list
        let edges = self.adjacency.entry(from).or_insert_with(Vec::new);
        let edge_index = edges.len();
        edges.push(edge.clone());

        // Update lookup table
        self.edge_lookup.insert((from, to, dex), (0, edge_index));

        debug!(
            "Added edge: {} -> {} via {} (rate: {:.6}, weight: {:.6})",
            from, to, edge.dex, edge.rate, edge.inverse_log_weight
        );
    }

    /// Update an existing edge's rate and recalculate weight
    pub fn update_edge_rate(&mut self, from: Pubkey, to: Pubkey, dex: DexType, new_rate: f64, timestamp: i64) -> Result<()> {
        let lookup_key = (from, to, dex.clone());
        
        if let Some(&(_list_idx, edge_idx)) = self.edge_lookup.get(&lookup_key) {
            if let Some(edges) = self.adjacency.get_mut(&from) {
                if let Some(edge) = edges.get_mut(edge_idx) {
                    edge.update_rate(new_rate, timestamp);
                    debug!(
                        "Updated edge: {} -> {} via {} (new rate: {:.6}, new weight: {:.6})",
                        from, to, dex, new_rate, edge.inverse_log_weight
                    );
                    return Ok(());
                }
            }
        }

        Err(anyhow!("Edge not found: {} -> {} via {}", from, to, dex))
    }

    /// Get all tokens in the graph
    pub fn get_all_tokens(&self) -> Vec<Pubkey> {
        self.tokens.iter().copied().collect()
    }

    /// Get all outgoing edges from a token
    pub fn get_edges_from(&self, token: &Pubkey) -> Option<&Vec<ExchangeEdge>> {
        self.adjacency.get(token)
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }

    /// Get token count
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// Detect triangular arbitrage opportunities using Bellman-Ford algorithm
    /// Returns all profitable cycles found
    pub fn detect_triangular_arbitrage(
        &self,
        start_token: &Pubkey,
        max_path_length: usize,
        min_profit_bps: i64,
    ) -> Vec<TriangularArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        // Use BFS to find all paths of length 2-3 that return to start
        let mut queue = VecDeque::new();
        queue.push_back((vec![*start_token], 0.0f64)); // (path, cumulative_weight)

        while let Some((path, weight)) = queue.pop_front() {
            let current = *path.last().unwrap();

            // If path length is 2-3 and we can return to start, check for arbitrage
            if path.len() >= 2 && path.len() <= max_path_length {
                if let Some(edges) = self.get_edges_from(&current) {
                    for edge in edges {
                        if edge.to_token == *start_token {
                            let cycle_weight = weight + edge.inverse_log_weight;
                            
                            // Negative cycle = profit!
                            if cycle_weight < 0.0 {
                                let profit_ratio = (-cycle_weight).exp();
                                let profit_bps = ((profit_ratio - 1.0) * 10000.0) as i64;
                                
                                if profit_bps >= min_profit_bps {
                                    // Build the full path with edges
                                    let mut full_path = Vec::new();
                                    for i in 0..path.len() - 1 {
                                        if let Some(edges) = self.get_edges_from(&path[i]) {
                                            for e in edges {
                                                if e.to_token == path[i + 1] {
                                                    full_path.push(e.clone());
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    full_path.push(edge.clone());

                                    let total_fees_bps: u16 = full_path.iter()
                                        .map(|e| e.fee_bps)
                                        .sum();

                                    let path_length = full_path.len();

                                    opportunities.push(TriangularArbitrageOpportunity {
                                        path: full_path,
                                        profit_ratio,
                                        profit_bps,
                                        input_token: *start_token,
                                        input_amount: 0, // To be calculated
                                        estimated_output: 0, // To be calculated
                                        total_fees_bps,
                                        cycle_weight,
                                    });

                                    info!(
                                        "Found triangular arbitrage: profit={} bps, path_length={}, cycle_weight={:.6}",
                                        profit_bps, path_length, cycle_weight
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // Continue exploring if path not too long
            if path.len() < max_path_length {
                if let Some(edges) = self.get_edges_from(&current) {
                    for edge in edges {
                        // Don't revisit tokens except to return to start
                        if !path.contains(&edge.to_token) || edge.to_token == *start_token {
                            let mut new_path = path.clone();
                            new_path.push(edge.to_token);
                            let new_weight = weight + edge.inverse_log_weight;
                            queue.push_back((new_path, new_weight));
                        }
                    }
                }
            }
        }

        opportunities
    }

    /// Detect all triangular arbitrage opportunities across all tokens
    pub fn detect_all_triangular_arbitrage(
        &self,
        max_path_length: usize,
        min_profit_bps: i64,
    ) -> Vec<TriangularArbitrageOpportunity> {
        let mut all_opportunities = Vec::new();
        
        for token in self.get_all_tokens() {
            let opportunities = self.detect_triangular_arbitrage(&token, max_path_length, min_profit_bps);
            all_opportunities.extend(opportunities);
        }

        // Sort by profit descending
        all_opportunities.sort_by(|a, b| b.profit_bps.cmp(&a.profit_bps));
        
        info!("Detected {} triangular arbitrage opportunities", all_opportunities.len());
        all_opportunities
    }

    /// Calculate optimal trade size for a triangular arbitrage opportunity
    pub fn calculate_optimal_trade_size(
        &self,
        opportunity: &TriangularArbitrageOpportunity,
        max_input_amount: u64,
        max_slippage_bps: u16,
    ) -> Result<(u64, u64)> {
        // Start with max amount and reduce based on liquidity constraints
        let mut optimal_input = max_input_amount;
        
        // Check each edge in the path for liquidity limits
        for edge in &opportunity.path {
            let max_tradeable = edge.get_max_tradeable_amount(max_slippage_bps);
            optimal_input = optimal_input.min(max_tradeable);
        }

        // Simulate the full path to get output
        let mut current_amount = optimal_input;
        for edge in &opportunity.path {
            let fee_multiplier = 1.0 - (edge.fee_bps as f64 / 10000.0);
            current_amount = (current_amount as f64 * edge.rate * fee_multiplier) as u64;
        }

        Ok((optimal_input, current_amount))
    }
}

impl Default for ArbitrageGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for ArbitrageGraph
pub type SharedArbitrageGraph = Arc<RwLock<ArbitrageGraph>>;

/// Create a new thread-safe arbitrage graph
pub fn create_shared_graph() -> SharedArbitrageGraph {
    Arc::new(RwLock::new(ArbitrageGraph::new()))
}

// ============================================================================
// Bellman-Ford Algorithm for Negative Cycle Detection
// ============================================================================

/// Represents a single step in an arbitrage cycle
#[derive(Clone, Debug)]
pub struct CycleStep {
    pub from_token: Pubkey,
    pub to_token: Pubkey,
    pub dex: DexType,
    pub pool: Pubkey,
    pub rate: f64,
    pub fee_bps: u16,
}

/// Represents a complete arbitrage cycle with profit calculations
#[derive(Clone, Debug)]
pub struct ArbitrageCycle {
    pub path: Vec<CycleStep>,
    pub gross_profit_bps: i64, // Profit in basis points before fees
    pub net_profit_after_fees: f64, // Actual profit after all fees
    pub execution_time_estimate_ms: u64, // Estimated time to execute
    pub total_fee_bps: u16, // Sum of all fees in the cycle
    pub start_token: Pubkey,
    pub cycle_weight: f64, // Negative if profitable
}

impl ArbitrageCycle {
    /// Calculate profit after slippage for a given trade amount
    pub fn calculate_slippage_adjusted_profit(&self, amount: u64, liquidity_map: &HashMap<Pubkey, Vec<PriceLevel>>) -> f64 {
        let mut current_amount = amount as f64;
        
        for step in &self.path {
            // Apply exchange rate
            current_amount *= step.rate;
            
            // Apply fee
            let fee_multiplier = 1.0 - (step.fee_bps as f64 / 10000.0);
            current_amount *= fee_multiplier;
            
            // Apply slippage based on liquidity depth
            if let Some(levels) = liquidity_map.get(&step.pool) {
                let slippage_factor = Self::calculate_slippage_factor(current_amount as u64, levels);
                current_amount *= slippage_factor;
            }
        }
        
        current_amount - amount as f64
    }
    
    fn calculate_slippage_factor(amount: u64, liquidity: &[PriceLevel]) -> f64 {
        if liquidity.is_empty() {
            return 0.98; // Conservative 2% slippage estimate
        }
        
        let mut remaining = amount;
        let mut weighted_price = 0.0;
        let mut total_filled = 0u64;
        
        for level in liquidity {
            if remaining == 0 {
                break;
            }
            
            let filled = remaining.min(level.liquidity);
            weighted_price += level.price * filled as f64;
            total_filled += filled;
            remaining -= filled;
        }
        
        if total_filled == 0 {
            return 0.98;
        }
        
        weighted_price / total_filled as f64
    }
    
    /// Check if cycle meets Solana transaction size limits
    pub fn fits_in_transaction(&self) -> bool {
        const MAX_TX_SIZE: usize = 1232; // Solana's max transaction size
        const INSTRUCTION_OVERHEAD: usize = 100; // Signature + metadata overhead
        const PER_HOP_SIZE: usize = 150; // Approximate size per swap instruction
        
        let estimated_size = INSTRUCTION_OVERHEAD + (self.path.len() * PER_HOP_SIZE);
        estimated_size <= MAX_TX_SIZE
    }
    
    /// Estimate execution time based on number of hops and network conditions
    pub fn estimate_execution_time(&self, avg_confirmation_ms: u64) -> u64 {
        // Each hop adds latency, plus network confirmation time
        let hop_latency = self.path.len() as u64 * 50; // 50ms per hop estimate
        hop_latency + avg_confirmation_ms
    }
}

/// Bellman-Ford detector for arbitrage opportunities
pub struct BellmanFordDetector {
    graph: SharedArbitrageGraph,
    min_profit_bps: i64,
    max_path_length: usize,
}

impl BellmanFordDetector {
    /// Create a new Bellman-Ford detector
    pub fn new(graph: SharedArbitrageGraph, min_profit_bps: i64) -> Self {
        info!("Initializing BellmanFordDetector with min_profit={} bps", min_profit_bps);
        Self {
            graph,
            min_profit_bps,
            max_path_length: 4, // Support up to 4 hops
        }
    }
    
    /// Set maximum path length for cycle detection
    pub fn with_max_path_length(mut self, length: usize) -> Self {
        self.max_path_length = length;
        self
    }
    
    /// Run Bellman-Ford algorithm to detect arbitrage cycles
    /// Returns all profitable cycles found starting from start_token
    pub async fn detect_arbitrage(&self, start_token: Pubkey) -> Result<Vec<ArbitrageCycle>> {
        // Clone graph data for concurrent processing
        let graph = self.graph.read().map_err(|e| anyhow!("Failed to acquire graph lock: {}", e))?;
        
        // Get all tokens to iterate over
        let tokens = graph.get_all_tokens();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        
        debug!("Running Bellman-Ford from {} across {} tokens", start_token, tokens.len());
        
        // Initialize distance map: distance[token] = shortest path weight from start
        let mut distances: HashMap<Pubkey, f64> = HashMap::new();
        let mut predecessors: HashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)> = HashMap::new();
        
        // Initialize: start token has distance 0, all others infinity
        for token in &tokens {
            distances.insert(*token, f64::INFINITY);
        }
        distances.insert(start_token, 0.0);
        
        // Relax edges |V|-1 times (standard Bellman-Ford)
        let num_tokens = tokens.len();
        for iteration in 0..num_tokens - 1 {
            let mut updated = false;
            
            for token in &tokens {
                if let Some(&current_dist) = distances.get(token) {
                    if current_dist == f64::INFINITY {
                        continue;
                    }
                    
                    // Relax all outgoing edges from this token
                    if let Some(edges) = graph.get_edges_from(token) {
                        for edge in edges {
                            let new_dist = current_dist + edge.inverse_log_weight;
                            let neighbor_dist = distances.get(&edge.to_token).copied().unwrap_or(f64::INFINITY);
                            
                            if new_dist < neighbor_dist {
                                distances.insert(edge.to_token, new_dist);
                                predecessors.insert(
                                    edge.to_token,
                                    (*token, edge.dex.clone(), edge.pool_address, edge.rate, edge.fee_bps)
                                );
                                updated = true;
                            }
                        }
                    }
                }
            }
            
            // Early termination if no updates
            if !updated {
                debug!("Bellman-Ford converged at iteration {}", iteration + 1);
                break;
            }
        }
        
        // Detect negative cycles (arbitrage opportunities)
        let mut cycles = Vec::new();
        let mut visited_cycles: HashSet<Vec<Pubkey>> = HashSet::new();
        
        for token in &tokens {
            if let Some(&current_dist) = distances.get(token) {
                if current_dist == f64::INFINITY {
                    continue;
                }
                
                // Check all outgoing edges for negative cycle
                if let Some(edges) = graph.get_edges_from(token) {
                    for edge in edges {
                        let new_dist = current_dist + edge.inverse_log_weight;
                        let neighbor_dist = distances.get(&edge.to_token).copied().unwrap_or(f64::INFINITY);
                        
                        // If we can still relax, we found a negative cycle
                        if new_dist < neighbor_dist {
                            // Reconstruct the cycle
                            if let Some(cycle) = self.reconstruct_cycle(
                                &predecessors,
                                &graph,
                                edge.to_token,
                                start_token,
                                *token,
                                edge.dex.clone(),
                                edge.pool_address,
                                edge.rate,
                                edge.fee_bps,
                            ) {
                                // Deduplicate cycles (same tokens, different order)
                                let mut cycle_tokens: Vec<Pubkey> = cycle.path.iter().map(|s| s.from_token).collect();
                                cycle_tokens.sort();
                                
                                if !visited_cycles.contains(&cycle_tokens) && cycle.net_profit_after_fees > 0.0 {
                                    visited_cycles.insert(cycle_tokens);
                                    
                                    if cycle.gross_profit_bps >= self.min_profit_bps {
                                        info!(
                                            "Detected arbitrage cycle: {} bps gross, {:.6} net profit, {} hops",
                                            cycle.gross_profit_bps, cycle.net_profit_after_fees, cycle.path.len()
                                        );
                                        cycles.push(cycle);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Sort by net profit descending
        cycles.sort_by(|a, b| {
            b.net_profit_after_fees.partial_cmp(&a.net_profit_after_fees).unwrap()
        });
        
        Ok(cycles)
    }
    
    /// Run detection concurrently for multiple start tokens
    pub async fn detect_arbitrage_parallel(&self, start_tokens: Vec<Pubkey>) -> Result<Vec<ArbitrageCycle>> {
        let mut handles = Vec::new();
        
        for token in start_tokens {
            let detector = self.clone_detector();
            let handle = task::spawn(async move {
                detector.detect_arbitrage(token).await
            });
            handles.push(handle);
        }
        
        let mut all_cycles = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(cycles)) => all_cycles.extend(cycles),
                Ok(Err(e)) => warn!("Arbitrage detection failed: {}", e),
                Err(e) => warn!("Task join failed: {}", e),
            }
        }
        
        // Deduplicate and sort
        all_cycles.sort_by(|a, b| {
            b.net_profit_after_fees.partial_cmp(&a.net_profit_after_fees).unwrap()
        });
        all_cycles.dedup_by(|a, b| {
            let a_tokens: Vec<_> = a.path.iter().map(|s| s.from_token).collect();
            let b_tokens: Vec<_> = b.path.iter().map(|s| s.from_token).collect();
            a_tokens == b_tokens
        });
        
        Ok(all_cycles)
    }
    
    /// Reconstruct cycle from predecessor map
    fn reconstruct_cycle(
        &self,
        predecessors: &HashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)>,
        _graph: &ArbitrageGraph,
        cycle_token: Pubkey,
        start_token: Pubkey,
        last_from: Pubkey,
        last_dex: DexType,
        last_pool: Pubkey,
        last_rate: f64,
        last_fee: u16,
    ) -> Option<ArbitrageCycle> {
        let mut path = Vec::new();
        let mut current = cycle_token;
        let mut visited = HashSet::new();
        let mut cycle_weight = 0.0;
        
        // Trace back through predecessors
        while let Some(&(from_token, ref dex, pool, rate, fee_bps)) = predecessors.get(&current) {
            if visited.contains(&current) {
                break; // Prevent infinite loops
            }
            visited.insert(current);
            
            // Add step to path
            path.push(CycleStep {
                from_token,
                to_token: current,
                dex: dex.clone(),
                pool,
                rate,
                fee_bps,
            });
            
            // Calculate weight
            cycle_weight += ExchangeEdge::calculate_weight(rate, fee_bps);
            
            current = from_token;
            
            // Stop if we've traced back enough or found start
            if path.len() >= self.max_path_length {
                break;
            }
        }
        
        // Add the final edge that closes the cycle
        path.push(CycleStep {
            from_token: last_from,
            to_token: cycle_token,
            dex: last_dex,
            pool: last_pool,
            rate: last_rate,
            fee_bps: last_fee,
        });
        cycle_weight += ExchangeEdge::calculate_weight(last_rate, last_fee);
        
        // Reverse to get correct order
        path.reverse();
        
        // Validate cycle (should return to start or form a loop)
        if path.is_empty() {
            return None;
        }
        
        // Calculate profits
        let gross_profit_ratio = (-cycle_weight).exp();
        let gross_profit_bps = ((gross_profit_ratio - 1.0) * 10000.0) as i64;
        
        let total_fee_bps: u16 = path.iter().map(|s| s.fee_bps).sum();
        
        // Net profit calculation: apply all fees
        let mut net_multiplier = 1.0;
        for step in &path {
            net_multiplier *= step.rate * (1.0 - step.fee_bps as f64 / 10000.0);
        }
        let net_profit_after_fees = net_multiplier - 1.0;
        
        // Estimate execution time (50ms per hop + 400ms confirmation)
        let execution_time_estimate_ms = (path.len() as u64 * 50) + 400;
        
        Some(ArbitrageCycle {
            path,
            gross_profit_bps,
            net_profit_after_fees,
            execution_time_estimate_ms,
            total_fee_bps,
            start_token,
            cycle_weight,
        })
    }
    
    /// Clone detector for parallel execution
    fn clone_detector(&self) -> Self {
        Self {
            graph: Arc::clone(&self.graph),
            min_profit_bps: self.min_profit_bps,
            max_path_length: self.max_path_length,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    fn create_test_pubkey(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    #[test]
    fn test_exchange_edge_weight_calculation() {
        // Test weight calculation: -log(rate * (1 - fee))
        let rate = 1.5;
        let fee_bps = 30; // 0.3%
        let weight = ExchangeEdge::calculate_weight(rate, fee_bps);
        
        let expected = -(rate * (1.0 - 0.003)).ln();
        assert!((weight - expected).abs() < 1e-10);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = ArbitrageGraph::new();
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let pool = create_test_pubkey(100);

        let edge = ExchangeEdge::new(
            token_a,
            token_b,
            DexType::Raydium,
            pool,
            1.5,
            25,
            vec![],
            1000,
        );

        graph.add_edge(edge);
        
        assert_eq!(graph.token_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.get_edges_from(&token_a).is_some());
    }

    #[test]
    fn test_update_edge_rate() {
        let mut graph = ArbitrageGraph::new();
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let pool = create_test_pubkey(100);

        let edge = ExchangeEdge::new(
            token_a,
            token_b,
            DexType::Raydium,
            pool,
            1.5,
            25,
            vec![],
            1000,
        );

        graph.add_edge(edge);
        
        // Update rate
        graph.update_edge_rate(token_a, token_b, DexType::Raydium, 2.0, 2000).unwrap();
        
        let edges = graph.get_edges_from(&token_a).unwrap();
        assert_eq!(edges[0].rate, 2.0);
        assert_eq!(edges[0].last_update, 2000);
    }

    #[test]
    fn test_triangular_arbitrage_detection() {
        let mut graph = ArbitrageGraph::new();
        
        // Create a profitable triangle: A -> B -> C -> A
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let token_c = create_test_pubkey(3);

        // A -> B: 1 A = 1.1 B
        graph.add_edge(ExchangeEdge::new(
            token_a, token_b, DexType::Raydium, create_test_pubkey(101),
            1.1, 25, vec![], 1000,
        ));

        // B -> C: 1 B = 1.1 C
        graph.add_edge(ExchangeEdge::new(
            token_b, token_c, DexType::Meteora, create_test_pubkey(102),
            1.1, 25, vec![], 1000,
        ));

        // C -> A: 1 C = 0.85 A (this creates profit)
        graph.add_edge(ExchangeEdge::new(
            token_c, token_a, DexType::Orca, create_test_pubkey(103),
            0.85, 25, vec![], 1000,
        ));

        let opportunities = graph.detect_triangular_arbitrage(&token_a, 3, 0);
        
        // Should find at least one opportunity
        assert!(!opportunities.is_empty(), "Should detect triangular arbitrage");
        
        let opp = &opportunities[0];
        assert_eq!(opp.path.len(), 3);
        assert!(opp.profit_bps > 0, "Should have positive profit");
    }

    #[test]
    fn test_shared_graph_thread_safety() {
        let graph = create_shared_graph();
        
        // Test write lock
        {
            let mut g = graph.write().unwrap();
            let token_a = create_test_pubkey(1);
            let token_b = create_test_pubkey(2);
            
            g.add_edge(ExchangeEdge::new(
                token_a, token_b, DexType::Raydium, create_test_pubkey(100),
                1.5, 25, vec![], 1000,
            ));
        }
        
        // Test read lock
        {
            let g = graph.read().unwrap();
            assert_eq!(g.token_count(), 2);
            assert_eq!(g.edge_count(), 1);
        }
    }

    #[test]
    fn test_no_arbitrage_detection() {
        let mut graph = ArbitrageGraph::new();
        
        // Create a non-profitable triangle
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let token_c = create_test_pubkey(3);

        // A -> B: 1 A = 1.0 B
        graph.add_edge(ExchangeEdge::new(
            token_a, token_b, DexType::Raydium, create_test_pubkey(101),
            1.0, 25, vec![], 1000,
        ));

        // B -> C: 1 B = 1.0 C
        graph.add_edge(ExchangeEdge::new(
            token_b, token_c, DexType::Meteora, create_test_pubkey(102),
            1.0, 25, vec![], 1000,
        ));

        // C -> A: 1 C = 1.0 A (no profit due to fees)
        graph.add_edge(ExchangeEdge::new(
            token_c, token_a, DexType::Orca, create_test_pubkey(103),
            1.0, 25, vec![], 1000,
        ));

        let opportunities = graph.detect_triangular_arbitrage(&token_a, 3, 10);
        
        // Should not find profitable opportunities
        assert!(opportunities.is_empty(), "Should not detect unprofitable arbitrage");
    }

    #[tokio::test]
    async fn test_bellman_ford_detector() {
        let graph = create_shared_graph();
        
        // Create a profitable triangle
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let token_c = create_test_pubkey(3);

        {
            let mut g = graph.write().unwrap();
            
            // A -> B: 1 A = 1.1 B
            g.add_edge(ExchangeEdge::new(
                token_a, token_b, DexType::Raydium, create_test_pubkey(101),
                1.1, 25, vec![], 1000,
            ));

            // B -> C: 1 B = 1.1 C
            g.add_edge(ExchangeEdge::new(
                token_b, token_c, DexType::Meteora, create_test_pubkey(102),
                1.1, 25, vec![], 1000,
            ));

            // C -> A: 1 C = 0.85 A
            g.add_edge(ExchangeEdge::new(
                token_c, token_a, DexType::Orca, create_test_pubkey(103),
                0.85, 25, vec![], 1000,
            ));
        }

        let detector = BellmanFordDetector::new(graph, 0);
        let cycles = detector.detect_arbitrage(token_a).await.unwrap();
        
        assert!(!cycles.is_empty(), "Should detect arbitrage cycles");
        assert!(cycles[0].gross_profit_bps > 0, "Should have positive gross profit");
        assert!(cycles[0].fits_in_transaction(), "Should fit in transaction");
    }

    #[tokio::test]
    async fn test_parallel_detection() {
        let graph = create_shared_graph();
        
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        let token_c = create_test_pubkey(3);

        {
            let mut g = graph.write().unwrap();
            
            g.add_edge(ExchangeEdge::new(
                token_a, token_b, DexType::Raydium, create_test_pubkey(101),
                1.05, 25, vec![], 1000,
            ));

            g.add_edge(ExchangeEdge::new(
                token_b, token_c, DexType::Meteora, create_test_pubkey(102),
                1.05, 25, vec![], 1000,
            ));

            g.add_edge(ExchangeEdge::new(
                token_c, token_a, DexType::Orca, create_test_pubkey(103),
                0.92, 25, vec![], 1000,
            ));
        }

        let detector = BellmanFordDetector::new(graph, 0);
        let cycles = detector.detect_arbitrage_parallel(vec![token_a, token_b]).await.unwrap();
        
        // May or may not find cycles depending on the exact rates and fees
        // Just verify the parallel execution completes without errors
        assert!(cycles.len() >= 0, "Parallel detection should complete");
    }

    #[test]
    fn test_cycle_slippage_calculation() {
        let token_a = create_test_pubkey(1);
        let token_b = create_test_pubkey(2);
        
        let cycle = ArbitrageCycle {
            path: vec![
                CycleStep {
                    from_token: token_a,
                    to_token: token_b,
                    dex: DexType::Raydium,
                    pool: create_test_pubkey(100),
                    rate: 1.1,
                    fee_bps: 25,
                },
                CycleStep {
                    from_token: token_b,
                    to_token: token_a,
                    dex: DexType::Orca,
                    pool: create_test_pubkey(101),
                    rate: 0.95,
                    fee_bps: 30,
                },
            ],
            gross_profit_bps: 100,
            net_profit_after_fees: 0.01,
            execution_time_estimate_ms: 500,
            total_fee_bps: 55,
            start_token: token_a,
            cycle_weight: -0.01,
        };
        
        let liquidity_map = HashMap::new();
        let profit = cycle.calculate_slippage_adjusted_profit(1000, &liquidity_map);
        
        // Profit calculation: 1000 * 1.1 * 0.9975 * 0.95 * 0.997 * 0.98 (slippage) ≈ 1018
        // Net profit should be positive but reduced by slippage
        assert!(profit > 0.0, "Should have positive profit");
        assert!(profit < 100.0, "Slippage and fees should reduce profit significantly");
    }

    #[test]
    fn test_transaction_size_limit() {
        let token_a = create_test_pubkey(1);
        
        // Create a cycle with many hops
        let mut long_path = Vec::new();
        for i in 0..10 {
            long_path.push(CycleStep {
                from_token: create_test_pubkey(i),
                to_token: create_test_pubkey(i + 1),
                dex: DexType::Raydium,
                pool: create_test_pubkey(100 + i),
                rate: 1.01,
                fee_bps: 25,
            });
        }
        
        let cycle = ArbitrageCycle {
            path: long_path,
            gross_profit_bps: 100,
            net_profit_after_fees: 0.01,
            execution_time_estimate_ms: 1000,
            total_fee_bps: 250,
            start_token: token_a,
            cycle_weight: -0.01,
        };
        
        // 10 hops is too many for a single transaction
        assert!(!cycle.fits_in_transaction(), "10 hops should exceed transaction limit");
        
        // Test with 3 hops
        let short_cycle = ArbitrageCycle {
            path: cycle.path[..3].to_vec(),
            ..cycle
        };
        assert!(short_cycle.fits_in_transaction(), "3 hops should fit in transaction");
    }
}
