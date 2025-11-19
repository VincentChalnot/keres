use arx_engine::engine::{EngineVariant, MinimaxEngine, tournament::*};

fn main() {
    println!("Arx Engine - Variant Tournament");
    println!("================================\n");

    // List all available variants
    println!("Available engine variants:");
    for variant in EngineVariant::all() {
        println!("  • {}: {}", variant.name(), variant.description());
    }
    println!();

    // Configure the match
    let config = MatchConfig {
        num_games: 6, // Play 6 games to alternate colors
        time_per_move_ms: 2000, // 2 seconds per move
        max_moves_per_game: 100, // Maximum 100 moves per game
    };

    // Run matches between different variants
    run_tournament_match(EngineVariant::Aggressive, EngineVariant::Defensive, config.clone());
    run_tournament_match(EngineVariant::Balanced, EngineVariant::Tactical, config.clone());
    run_tournament_match(EngineVariant::Positional, EngineVariant::Aggressive, config.clone());

    println!("\nTournament complete!");
    println!("\nTo run your own custom matches:");
    println!("  1. Choose two engine variants");
    println!("  2. Configure the match parameters");
    println!("  3. Call run_match() to see the results");
}

fn run_tournament_match(variant1: EngineVariant, variant2: EngineVariant, config: MatchConfig) {
    println!("\n╔════════════════════════════════════════════════════╗");
    println!("  Match: {} vs {}", variant1.name(), variant2.name());
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
}
