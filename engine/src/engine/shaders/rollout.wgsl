// Keres rollout evaluation shader
// Evaluates board positions using material + positional heuristics.
// Each workgroup thread processes one board from the positions array.

// ── Board encoding (mirrors move_generation.wgsl) ──

struct BoardState {
    squares: array<u32, 81>,
    white_to_move: u32,
    game_over: u32,
    white_wins: u32,
    draw: u32,
    moves_without_capture: u32,
}

// ── Scoring weights (mirrors ScoringWeights in config.rs) ──

struct WeightTable {
    soldier_pts: u32,
    bishop_pts: u32,
    rook_pts: u32,
    paladin_pts: u32,
    guard_pts: u32,
    knight_pts: u32,
    ballista_pts: u32,
    king_pts: u32,
    centrality_wt: u32,
    mobility_wt: u32,
    king_shield_wt: u32,
    threat_wt: u32,
    advance_wt: u32,
    stack_mod: i32,
    smart_depth: u32,
    capture_pct: u32,
    threat_pct: u32,
    _rsv0: u32, _rsv1: u32, _rsv2: u32,
    _rsv3: u32, _rsv4: u32, _rsv5: u32,
    _rsv6: u32, _rsv7: u32, _rsv8: u32,
}

// ── Bindings ──

@group(0) @binding(0) var<storage, read>       positions: array<u32>;
@group(0) @binding(1) var<storage, read_write>  results:   array<f32>;
@group(0) @binding(2) var<uniform>              weights:   WeightTable;
@group(0) @binding(3) var<storage, read_write>  rng_seeds: array<u32>;

// ── Piece constants ──

const PIECE_SOLDIER:  u32 = 1u;
const PIECE_BISHOP:   u32 = 2u;
const PIECE_ROOK:     u32 = 3u;
const PIECE_PALADIN:  u32 = 4u;
const PIECE_GUARD:    u32 = 5u;
const PIECE_KNIGHT:   u32 = 6u;
const PIECE_BALLISTA: u32 = 7u;
const KING_ENCODING:  u32 = 0x38u;  // 0b111000
const BOARD_DIM:      u32 = 9u;
const BOARD_SQ:       u32 = 81u;
const BYTES_PER_POS:  u32 = 84u;    // 83 data + 1 padding

// ── RNG (xorshift32) ──

fn xorshift32(state: u32) -> u32 {
    var s = state;
    s ^= s << 13u;
    s ^= s >> 17u;
    s ^= s << 5u;
    return s;
}

// ── Position data reader ──
// Positions are packed as bytes into u32 storage. Each position is
// BYTES_PER_POS bytes; we need to read individual bytes from the
// u32 array.

fn read_position_byte(board_idx: u32, byte_offset: u32) -> u32 {
    let global_byte = board_idx * BYTES_PER_POS + byte_offset;
    let word_idx = global_byte / 4u;
    let lane = global_byte % 4u;
    let word = positions[word_idx];
    return (word >> (lane * 8u)) & 0xFFu;
}

// ── Piece extraction helpers ──

fn extract_color(encoded: u32) -> u32 {
    return (encoded >> 6u) & 1u;
}

fn extract_top_code(encoded: u32) -> u32 {
    return (encoded >> 3u) & 7u;
}

fn extract_bottom_code(encoded: u32) -> u32 {
    return encoded & 7u;
}

fn is_king_piece(encoded: u32) -> bool {
    return (encoded & 0x3Fu) == KING_ENCODING;
}

// ── Material value lookup ──

fn piece_material(code: u32) -> u32 {
    switch code {
        case 1u: { return weights.soldier_pts; }
        case 2u: { return weights.bishop_pts; }
        case 3u: { return weights.rook_pts; }
        case 4u: { return weights.paladin_pts; }
        case 5u: { return weights.guard_pts; }
        case 6u: { return weights.knight_pts; }
        case 7u: { return weights.ballista_pts; }
        default: { return weights.king_pts; }
    }
}

// ── Manhattan distance from center ──

fn manhattan_from_center(sq_index: u32) -> u32 {
    let col = sq_index % BOARD_DIM;
    let row = sq_index / BOARD_DIM;
    let dx = select(col - 4u, 4u - col, col < 4u);
    let dy = select(row - 4u, 4u - row, row < 4u);
    return dx + dy;
}

// ── Sigmoid function ──

fn sigmoid_keres(x: f32) -> f32 {
    return 1.0 / (1.0 + exp(-x));
}

// ── Entry point ──

@compute @workgroup_size(64)
fn rollout_entry(@builtin(global_invocation_id) gid: vec3<u32>) {
    let board_idx = gid.x;

    // Read game-state flags from byte 81
    let flags_byte = read_position_byte(board_idx, 81u);
    let who_moves = (flags_byte >> 7u) & 1u;  // 1 = white
    let is_over   = (flags_byte >> 6u) & 1u;
    let w_wins    = (flags_byte >> 5u) & 1u;
    let is_draw   = (flags_byte >> 4u) & 1u;

    // Terminal positions: assign fixed scores from WHITE's perspective
    if is_over == 1u {
        if is_draw == 1u {
            results[board_idx] = 0.5;
            return;
        }
        // White wins → 1.0, black wins → 0.0 (always white's perspective)
        results[board_idx] = f32(w_wins);
        return;
    }

    // Non-terminal: material + positional evaluation from WHITE's perspective
    var white_total: f32 = 0.0;
    var black_total: f32 = 0.0;

    for (var sq: u32 = 0u; sq < BOARD_SQ; sq++) {
        let encoded = read_position_byte(board_idx, sq);
        if encoded == 0u { continue; }

        let piece_color = extract_color(encoded);
        let is_white = (piece_color == 1u);

        // Material contribution
        var mat_value: f32 = 0.0;
        if is_king_piece(encoded) {
            mat_value = f32(weights.king_pts);
        } else {
            let bottom = extract_bottom_code(encoded);
            mat_value += f32(piece_material(bottom));
            let top = extract_top_code(encoded);
            if top != 0u {
                mat_value += f32(piece_material(top));
            }
        }

        // Centrality bonus
        let dist = manhattan_from_center(sq);
        let cent_bonus = f32(8u - dist) * f32(weights.centrality_wt);
        mat_value += cent_bonus;

        // Accumulate to appropriate side
        if is_white {
            white_total += mat_value;
        } else {
            black_total += mat_value;
        }
    }

    let diff = white_total - black_total;
    results[board_idx] = sigmoid_keres(diff / 2000.0);
}
