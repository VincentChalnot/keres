#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScoringWeights {
    pub soldier_pts: u32,
    pub bishop_pts: u32,
    pub rook_pts: u32,
    pub paladin_pts: u32,
    pub guard_pts: u32,
    pub knight_pts: u32,
    pub ballista_pts: u32,
    pub king_pts: u32,
    pub centrality_wt: u32,
    pub mobility_wt: u32,
    pub king_shield_wt: u32,
    pub threat_wt: u32,
    pub advance_wt: u32,
    pub stack_mod: i32,
    pub smart_depth: u32,
    pub capture_pct: u32,
    pub threat_pct: u32,
    pub _fill: [u32; 9],
}
impl ScoringWeights {
    pub fn material_value(&self, disc: u32) -> u32 {
        let lut = [0u32, self.soldier_pts, self.bishop_pts, self.rook_pts,
                   self.paladin_pts, self.guard_pts, self.knight_pts, self.ballista_pts];
        if (disc as usize) < lut.len() { lut[disc as usize] } else { self.king_pts }
    }
}
impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            soldier_pts: 100, bishop_pts: 300, rook_pts: 500,
            paladin_pts: 300, guard_pts: 300, knight_pts: 300,
            ballista_pts: 500, king_pts: 10_000,
            centrality_wt: 10, mobility_wt: 15, king_shield_wt: 50,
            threat_wt: 40, advance_wt: 30, stack_mod: 0,
            smart_depth: 3, capture_pct: 70, threat_pct: 60,
            _fill: [0; 9],
        }
    }
}
#[derive(Clone, Debug)]
pub struct TreeParams { pub vl_penalty: u32, pub uct_c: f32, pub max_nodes: usize }
impl Default for TreeParams {
    fn default() -> Self { Self { vl_penalty: 10, uct_c: 1.414, max_nodes: 1_000_000 } }
}
#[derive(Clone, Debug)]
pub struct DispatchParams { pub batch_cap: usize, pub wait_limit_ms: u64, pub playout_depth: u32 }
impl Default for DispatchParams {
    fn default() -> Self { Self { batch_cap: 256, wait_limit_ms: 50, playout_depth: 15 } }
}
#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub tree: TreeParams, pub dispatch: DispatchParams,
    pub weights: ScoringWeights, pub threads: usize, pub iterations: usize,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self { tree: Default::default(), dispatch: Default::default(),
               weights: Default::default(), threads: 8, iterations: 10_000 }
    }
}
impl EngineConfig {
    pub fn tree_params_copy(&self) -> TreeParams { self.tree.clone() }
    pub fn dispatch_params_copy(&self) -> DispatchParams { self.dispatch.clone() }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn weight_struct_byte_count() { assert_eq!(core::mem::size_of::<ScoringWeights>(), 104); }
    #[test] fn bytemuck_cast_roundtrip() {
        let w = ScoringWeights::default();
        let w2: &ScoringWeights = bytemuck::from_bytes(bytemuck::bytes_of(&w));
        assert_eq!(w.soldier_pts, w2.soldier_pts);
        assert_eq!(w.king_pts, w2.king_pts);
    }
    #[test] fn engine_defaults() {
        let c = EngineConfig::default();
        assert_eq!(c.threads, 8); assert_eq!(c.iterations, 10_000);
        assert!((c.tree.uct_c - 1.414f32).abs() < 0.01);
        assert_eq!(c.dispatch.batch_cap, 256);
    }
    #[test] fn param_copies_independent() {
        let c = EngineConfig::default();
        let mut t = c.tree_params_copy(); t.vl_penalty = 42;
        assert_ne!(t.vl_penalty, c.tree.vl_penalty);
    }
}
