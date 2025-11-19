use arx_engine::engine::{EngineVariant, MinimaxEngine, tournament::*};

fn main() {
    println!("Arx Engine - Quick Variant Test");
    println!("================================\n");

    // List all available variants
    println!("Testing engine variants:");
    for variant in EngineVariant::all() {
        println!("  • {}: {}", variant.name(), variant.description());
    }
    println!();

    // Configure a quick match for testing
    let config = MatchConfig {
        num_games: 2, // Just 2 quick games
        time_per_move_ms: 500, // 500ms per move for speed
        max_moves_per_game: 20, // Max 20 moves for quick test
    };

    println!("Running quick test match (2 games, 500ms/move, max 20 moves)\n");

    // Test match between Aggressive and Defensive
    let variant1 = EngineVariant::Aggressive;
    let variant2 = EngineVariant::Defensive;

    println!("╔════════════════════════════════════════════════════╗");
    println!("  Quick Test: {} vs {}", variant1.name(), variant2.name());
    println!("╚════════════════════════════════════════════════════╝\n");

    let mut engine1 = MinimaxEngine::with_variant(variant1);
    let mut engine2 = MinimaxEngine::with_variant(variant2);

    let result = run_match_with_names(
        &mut engine1,
        &mut engine2,
        config,
        variant1.name().to_string(),
        variant2.name().to_string(),
    );

    result.print_summary();

    println!("\n✓ Tournament system working correctly!");
    println!("\nFor a full tournament, run:");
    println!("  cargo run --example engine_tournament");
}
