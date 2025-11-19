# Engine Tournament System

The Arx Engine includes a tournament framework for testing and comparing different engine configurations. This allows you to:

- Test new engine variants against established ones
- Tune engine parameters by running head-to-head matches
- Benchmark engine improvements objectively
- Find the strongest configuration for different play styles

## Engine Variants

The engine comes with five pre-configured variants, each optimized for different play styles:

### 1. Aggressive
**Description**: Prioritizes attacks, captures, and material gain. Takes risks with king safety.

**Configuration**:
- Material weight: 90% (very high - values captures)
- Territorial weight: 7%
- Mobility weight: 2%
- King safety weight: 1% (very low - takes risks)
- Stack bonus: 25%

**Best for**: Fast, attacking games where material advantage is paramount.

### 2. Defensive
**Description**: Focuses on king safety and solid defense. Avoids risky positions.

**Configuration**:
- Material weight: 75%
- Territorial weight: 3% (very low - stays back)
- Mobility weight: 7%
- King safety weight: 15% (very high - protects the king)
- Stack bonus: 15%

**Best for**: Solid positional play, avoiding blunders, protecting valuable pieces.

### 3. Balanced (Default)
**Description**: Well-rounded approach with balanced priorities.

**Configuration**:
- Material weight: 85%
- Territorial weight: 8%
- Mobility weight: 5%
- King safety weight: 2%
- Stack bonus: 20%

**Best for**: General-purpose play, good starting point for most positions.

### 4. Tactical
**Description**: Deep tactical search with strong capture evaluation. Finds complex combinations.

**Configuration**:
- Max depth: 5 (slightly lower to allow deeper quiescence)
- Time limit: 5000ms (more time for calculations)
- Material weight: 88%
- Territorial weight: 4%
- Mobility weight: 6%
- King safety weight: 2%
- Stack bonus: 30% (high - values tactical opportunities)

**Best for**: Positions with complex tactics, finding combinations and forced sequences.

### 5. Positional
**Description**: Emphasizes territorial control and piece activity. Values long-term advantages.

**Configuration**:
- Material weight: 70% (lower - willing to sacrifice)
- Territorial weight: 15% (high - controls the board)
- Mobility weight: 10% (high - keeps pieces active)
- King safety weight: 5%
- Stack bonus: 20%

**Best for**: Long-term strategic play, outpositioning opponents.

## Running Tournaments

### Quick Test

For a quick test of the tournament system:

```bash
cargo run --example quick_variant_test
```

This runs a 2-game match with fast time controls (500ms per move, max 20 moves per game).

### Full Tournament

For a more comprehensive tournament:

```bash
cargo run --example engine_tournament
```

This runs multiple matches between different variants with realistic time controls.

## Using the Tournament API

### Basic Match

```rust
use arx_engine::engine::{MinimaxEngine, EngineVariant, tournament::*};

// Create two engines with different variants
let mut aggressive = MinimaxEngine::with_variant(EngineVariant::Aggressive);
let mut defensive = MinimaxEngine::with_variant(EngineVariant::Defensive);

// Configure the match
let config = MatchConfig {
    num_games: 10,              // Play 10 games
    time_per_move_ms: 3000,     // 3 seconds per move
    max_moves_per_game: 100,    // Maximum 100 moves
};

// Run the match
let result = run_match(aggressive, defensive, config);

// Print results
result.print_summary();
```

### Custom Tournament

Create your own tournament with custom configurations:

```rust
use arx_engine::engine::{MinimaxEngine, MinimaxConfig, tournament::*};

// Create a custom aggressive configuration
let custom_config = MinimaxConfig {
    max_depth: 7,
    material_weight: 0.95,
    territorial_weight: 0.03,
    mobility_weight: 0.01,
    king_safety_weight: 0.01,
    stack_bonus: 0.30,
    ..Default::default()
};

let mut engine1 = MinimaxEngine::with_config(custom_config);
let mut engine2 = MinimaxEngine::with_variant(EngineVariant::Balanced);

let config = MatchConfig {
    num_games: 20,
    time_per_move_ms: 2000,
    max_moves_per_game: 150,
};

let result = run_match_with_names(
    &mut engine1,
    &mut engine2,
    config,
    "Custom Aggressive".to_string(),
    "Balanced".to_string(),
);

result.print_summary();
```

## Match Statistics

The tournament framework tracks detailed statistics:

### Game-Level Stats
- **Result**: Win/Loss/Draw
- **Number of moves**: Total moves in the game
- **Time per player**: Total thinking time for each player
- **Positions evaluated**: Average positions evaluated per move

### Match-Level Stats
- **Win percentages**: Win rate for each player
- **Draw percentage**: Percentage of drawn games
- **Average game length**: Average number of moves per game
- **Average time per game**: Average thinking time per game
- **Average positions per move**: Search efficiency metrics

## Example Output

```
╔════════════════════════════════════════════════════╗
  Match: Aggressive vs Defensive
╚════════════════════════════════════════════════════╝

Starting match: Aggressive vs Defensive
Configuration:
  Games: 10
  Time per move: 3000ms
  Max moves per game: 100

Game 1/10...
  Result: Aggressive wins!
  Moves: 45
  Aggressive time: 67.50s
  Defensive time: 71.23s

...

═══════════════════════════════════════════════════
Match Results: Aggressive vs Defensive
═══════════════════════════════════════════════════
Total games: 10

Aggressive wins: 6 (60.0%)
Defensive wins: 3 (30.0%)
Draws: 1 (10.0%)

Average game length: 52.3 moves
Average time per game:
  Aggressive: 78.45s
  Defensive: 82.13s
Average positions evaluated per move:
  Aggressive: 245
  Defensive: 198
═══════════════════════════════════════════════════
```

## Tournament Best Practices

### 1. Choose Appropriate Time Controls

- **Fast testing** (500-1000ms per move): Good for quick iteration
- **Normal play** (2000-4000ms per move): Realistic game conditions
- **Deep analysis** (5000-10000ms per move): Maximum strength play

### 2. Run Sufficient Games

- **Minimum**: 6-10 games (alternating colors)
- **Recommended**: 20-50 games for statistical significance
- **Comprehensive**: 100+ games for tournament-level testing

### 3. Alternate Colors

The tournament system automatically alternates which engine plays white/black to ensure fairness, as white typically has a slight advantage.

### 4. Test Multiple Matchups

To find the strongest variant:
- Run round-robin tournaments (all variants vs all others)
- Test each variant against the current best
- Validate improvements against a baseline configuration

### 5. Consider Position Types

Different variants excel in different positions:
- **Aggressive**: Open positions with tactical opportunities
- **Defensive**: Positions where king safety is critical
- **Tactical**: Complex middlegame positions
- **Positional**: Closed positions requiring long-term planning

## Creating Custom Variants

You can create your own variants by directly using `MinimaxConfig`:

```rust
use arx_engine::engine::{MinimaxEngine, MinimaxConfig};

// Ultra-aggressive variant
let ultra_aggressive = MinimaxConfig {
    max_depth: 6,
    use_quiescence: true,
    use_transposition_table: true,
    time_limit_ms: 3000,
    material_weight: 0.98,  // Almost pure material
    territorial_weight: 0.01,
    mobility_weight: 0.01,
    king_safety_weight: 0.0,  // No king safety consideration
    stack_bonus: 0.35,
};

let mut engine = MinimaxEngine::with_config(ultra_aggressive);
```

## Tuning Tips

When tuning engine parameters:

1. **Start with variants**: Begin with a base variant close to your goal
2. **Change one parameter at a time**: Easier to understand impact
3. **Run sufficient games**: At least 20 games per test
4. **Compare to baseline**: Always test against a known-good configuration
5. **Document findings**: Keep notes on what works and what doesn't

## Advanced: Parameter Sweeps

For systematic testing, you can sweep through parameter ranges:

```rust
use arx_engine::engine::{MinimaxEngine, MinimaxConfig, tournament::*};

// Test different material weights
for material_weight in [0.70, 0.75, 0.80, 0.85, 0.90, 0.95] {
    let config = MinimaxConfig {
        material_weight,
        territorial_weight: 0.15 - (material_weight - 0.70),
        mobility_weight: 0.10,
        king_safety_weight: 0.05,
        ..Default::default()
    };
    
    let mut test_engine = MinimaxEngine::with_config(config);
    let mut baseline = MinimaxEngine::with_variant(EngineVariant::Balanced);
    
    let match_config = MatchConfig {
        num_games: 20,
        time_per_move_ms: 2000,
        max_moves_per_game: 100,
    };
    
    let result = run_match_with_names(
        &mut test_engine,
        &mut baseline,
        match_config,
        format!("Material {:.2}", material_weight),
        "Baseline".to_string(),
    );
    
    println!("\nMaterial weight {}: Win rate {:.1}%\n",
             material_weight, result.player1_win_percentage());
}
```

## Tournament Examples

See the `examples/` directory for complete working examples:

- `engine_tournament.rs`: Full tournament between multiple variants
- `quick_variant_test.rs`: Fast testing with minimal configuration

## Contributing

When adding new engine variants or improvements:

1. Run tournaments against existing variants
2. Document the configuration and its intended use case
3. Include performance comparisons in your PR
4. Update this README with your findings

## License

Same as the main project (MIT License).
