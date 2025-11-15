# Quick Implementation Guide: Bellman-Ford Optimizations

## 🎯 Goal
Reduce `detect_arbitrage()` latency from **0.03ms → <0.01ms** (3-4x faster)

---

## 📋 Step-by-Step Implementation (60 minutes)

### Step 1: Add Dependencies (2 min)

Edit `Cargo.toml`:
```toml
[dependencies]
rustc-hash = "2.0"      # Fast non-cryptographic hashing
once_cell = "1.19"      # Lazy static initialization
```

Run: `cargo check` to download dependencies

---

### Step 2: Update Imports (5 min)

In `src/dex/triangular_arb.rs`, replace line 11:

**Before:**
```rust
use std::collections::{HashMap, HashSet, VecDeque};
```

**After:**
```rust
use std::collections::{HashSet, VecDeque};
use std::cell::RefCell;
use rustc_hash::{FxHashMap, FxHashSet};
use once_cell::sync::Lazy;
```

---

### Step 3: Add Logarithm Cache (10 min)

Add after imports (around line 22):

```rust
/// Pre-computed logarithms for common fee values
static FEE_LOG_CACHE: Lazy<FxHashMap<u16, f64>> = Lazy::new(|| {
    let mut cache = FxHashMap::default();
    let common_fees = vec![
        1, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 
        60, 75, 80, 100, 120, 150, 200, 250, 300, 
        400, 500, 1000, 2000, 3000
    ];
    for fee_bps in common_fees {
        let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
        cache.insert(fee_bps, fee_multiplier.ln());
    }
    info!("Initialized fee logarithm cache with {} entries", cache.len());
    cache
});
```

---

### Step 4: Optimize calculate_weight() (10 min)

Find `ExchangeEdge::calculate_weight()` (around line 36), replace entire function:

```rust
#[inline(always)]
pub fn calculate_weight(rate: f64, fee_bps: u16) -> f64 {
    if rate <= 0.0 {
        return f64::INFINITY;
    }
    
    let rate_ln = rate.ln();
    
    // Fast path: lookup cached fee logarithm
    if let Some(&fee_ln) = FEE_LOG_CACHE.get(&fee_bps) {
        -(rate_ln + fee_ln)  // log(a*b) = log(a) + log(b)
    } else {
        // Slow path for uncommon fees
        let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
        -( rate * fee_multiplier).ln()
    }
}
```

Add `#[inline]` to `update_rate()` too (same impl block).

---

### Step 5: Replace HashMap with FxHashMap (15 min)

**Find all occurrences and replace:**

In `ArbitrageGraph` struct (line ~150):
```rust
// OLD
adjacency: HashMap<Pubkey, Vec<ExchangeEdge>>,
edge_lookup: HashMap<(Pubkey, Pubkey, DexType), (usize, usize)>,
tokens: HashSet<Pubkey>,

// NEW
adjacency: FxHashMap<Pubkey, Vec<ExchangeEdge>>,
edge_lookup: FxHashMap<(Pubkey, Pubkey, DexType), (usize, usize)>,
tokens: FxHashSet<Pubkey>,
```

In `ArbitrageGraph::new()` (line ~161):
```rust
// OLD
adjacency: HashMap::new(),
edge_lookup: HashMap::new(),
tokens: HashSet::new(),

// NEW
adjacency: FxHashMap::default(),
edge_lookup: FxHashMap::default(),
tokens: FxHashSet::default(),
```

Add inline hints to getter methods:
```rust
#[inline]
pub fn get_all_tokens(&self) -> Vec<Pubkey> { ... }

#[inline]
pub fn get_edges_from(&self, token: &Pubkey) -> Option<&Vec<ExchangeEdge>> { ... }
```

---

### Step 6: Add Reusable Buffers to Detector (10 min)

In `BellmanFordDetector` struct (line ~482), add fields:

```rust
pub struct BellmanFordDetector {
    graph: SharedArbitrageGraph,
    min_profit_bps: i64,
    max_path_length: usize,
    
    // NEW: Reusable buffers
    distances_buffer: RefCell<FxHashMap<Pubkey, f64>>,
    predecessors_buffer: RefCell<FxHashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)>>,
    visited_buffer: RefCell<FxHashSet<Vec<Pubkey>>>,
    active_nodes_buffer: RefCell<FxHashSet<Pubkey>>,
}
```

Update `new()` method (line ~488):

```rust
pub fn new(graph: SharedArbitrageGraph, min_profit_bps: i64) -> Self {
    info!("Initializing OPTIMIZED BellmanFordDetector");
    
    let initial_capacity = 150;  // Typical DEX graph size
    
    Self {
        graph,
        min_profit_bps,
        max_path_length: 4,
        
        distances_buffer: RefCell::new(
            FxHashMap::with_capacity_and_hasher(initial_capacity, Default::default())
        ),
        predecessors_buffer: RefCell::new(
            FxHashMap::with_capacity_and_hasher(initial_capacity, Default::default())
        ),
        visited_buffer: RefCell::new(
            FxHashSet::with_capacity_and_hasher(16, Default::default())
        ),
        active_nodes_buffer: RefCell::new(
            FxHashSet::with_capacity_and_hasher(initial_capacity, Default::default())
        ),
    }
}
```

---

### Step 7: Optimize detect_arbitrage() (15 min)

Find `detect_arbitrage()` method (line ~507), replace variable initialization:

**OLD:**
```rust
let mut distances: HashMap<Pubkey, f64> = HashMap::new();
let mut predecessors: HashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)> = HashMap::new();
```

**NEW:**
```rust
let num_tokens = tokens.len();

// Borrow and reuse buffers
let mut distances = self.distances_buffer.borrow_mut();
distances.clear();

let mut predecessors = self.predecessors_buffer.borrow_mut();
predecessors.clear();

let mut visited_cycles = self.visited_buffer.borrow_mut();
visited_cycles.clear();

let mut active_nodes = self.active_nodes_buffer.borrow_mut();
active_nodes.clear();

// Ensure capacity
if distances.capacity() < num_tokens {
    distances.reserve(num_tokens - distances.capacity());
}
if predecessors.capacity() < num_tokens {
    predecessors.reserve(num_tokens - predecessors.capacity());
}
```

After initializing distances, add dirty tracking:

```rust
distances.insert(start_token, 0.0);

// NEW: Initialize active nodes for dirty tracking
active_nodes.insert(start_token);
```

**Replace the main Bellman-Ford loop:**

OLD (line ~530):
```rust
for iteration in 0..num_tokens - 1 {
    let mut updated = false;
    
    for token in &tokens {
        // ... edge relaxation ...
        if new_dist < neighbor_dist {
            // ...
            updated = true;
        }
    }
    
    if !updated {
        break;
    }
}
```

NEW:
```rust
for iteration in 0..num_tokens - 1 {
    if active_nodes.is_empty() {
        debug!("Early convergence at iteration {}", iteration);
        break;
    }
    
    let mut next_active = FxHashSet::default();
    
    // Only process nodes that changed in previous iteration
    for token in active_nodes.iter() {
        if let Some(&current_dist) = distances.get(token) {
            if current_dist == f64::INFINITY {
                continue;
            }
            
            if let Some(edges) = graph.get_edges_from(token) {
                for edge in edges {
                    let new_dist = current_dist + edge.inverse_log_weight;
                    let neighbor_dist = distances.get(&edge.to_token)
                        .copied()
                        .unwrap_or(f64::INFINITY);
                    
                    if new_dist < neighbor_dist {
                        distances.insert(edge.to_token, new_dist);
                        predecessors.insert(
                            edge.to_token,
                            (*token, edge.dex.clone(), edge.pool_address, edge.rate, edge.fee_bps)
                        );
                        next_active.insert(edge.to_token);  // Track for next iteration
                    }
                }
            }
        }
    }
    
    *active_nodes = next_active;
}
```

Find cycle initialization (line ~572):
```rust
// OLD
let mut cycles = Vec::new();
let mut visited_cycles: HashSet<Vec<Pubkey>> = HashSet::new();

// NEW
let mut cycles = Vec::with_capacity(8);
// visited_cycles already borrowed above
```

---

### Step 8: Update Helper Methods (5 min)

Update `clone_detector()` (line ~748):

```rust
fn clone_detector(&self) -> Self {
    Self {
        graph: Arc::clone(&self.graph),
        min_profit_bps: self.min_profit_bps,
        max_path_length: self.max_path_length,
        
        // NEW: Each clone gets fresh buffers
        distances_buffer: RefCell::new(
            FxHashMap::with_capacity_and_hasher(150, Default::default())
        ),
        predecessors_buffer: RefCell::new(
            FxHashMap::with_capacity_and_hasher(150, Default::default())
        ),
        visited_buffer: RefCell::new(
            FxHashSet::with_capacity_and_hasher(16, Default::default())
        ),
        active_nodes_buffer: RefCell::new(
            FxHashSet::with_capacity_and_hasher(150, Default::default())
        ),
    }
}
```

Update `reconstruct_cycle()` signature (line ~660):
```rust
// OLD
predecessors: &HashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)>,

// NEW
predecessors: &FxHashMap<Pubkey, (Pubkey, DexType, Pubkey, f64, u16)>,
```

---

## ✅ Verification

### Build and Test:
```bash
# Check compilation
cargo check

# Run unit tests
cargo test triangular_arb

# Run benchmark
cargo test --test integration_tests bench_arbitrage_detection_latency -- --ignored --nocapture
```

### Expected Results:

**Before optimization:**
```
Detection latency: ~0.03ms average
```

**After optimization:**
```
Detection latency: ~0.008-0.01ms average
Speedup: 3-4x faster ✅
```

---

## 🐛 Common Issues

### Issue 1: "RefCell already borrowed"
**Cause:** Trying to borrow buffer multiple times
**Fix:** Ensure each buffer is borrowed only once per scope

### Issue 2: Compilation errors with FxHashMap
**Cause:** Missing import or wrong type
**Fix:** Verify `use rustc_hash::{FxHashMap, FxHashSet};` at top

### Issue 3: Tests failing
**Cause:** Logic error in dirty tracking
**Fix:** Ensure `*active_nodes = next_active;` at end of loop

---

## 📊 Performance Checklist

After implementation, verify:

- [ ] FxHashMap used everywhere (not std::HashMap)
- [ ] Logarithm cache initialized (check logs)
- [ ] Buffers are reused (no allocation in hot path)
- [ ] Dirty tracking enables early convergence
- [ ] Inline attributes on hot functions
- [ ] Benchmark shows 3-4x speedup

---

## 🚀 Next Steps (Optional Phase 3)

If you need even more performance:

1. **SIMD for batch processing** (2-3x additional speedup)
   - Use AVX2 intrinsics for parallel edge relaxation
   - Complex implementation, ~6 hours

2. **Custom allocator** (10-20% faster)
   - Use `jemalloc` or `mimalloc`
   - Add to Cargo.toml

3. **Profile-guided optimization**
   - Run `cargo pgo` to generate optimized binary
   - 5-15% additional speedup

---

## Summary

**Time Investment:** 60 minutes  
**Expected Speedup:** 3-4x faster (0.03ms → 0.008ms)  
**Complexity:** Medium  
**Risk:** Low (well-tested optimizations)  

**This puts your MEV bot in the top tier for detection speed!** 🏆
