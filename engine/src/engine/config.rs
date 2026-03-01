#[derive(Copy, Clone, Debug)]
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
        }
    }
}
#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub weights: ScoringWeights,
    pub threads: usize,
    pub stage1_depth: i32,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            weights: Default::default(),
            threads: num_cpus::get().saturating_sub(1).max(1),
            stage1_depth: 4,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn engine_defaults() {
        let c = EngineConfig::default();
        assert!(c.threads >= 1);
        assert_eq!(c.stage1_depth, 4);
    }
}
