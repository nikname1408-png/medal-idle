use super::state::GameState;

/// Один игровой тик: регенерация энергии.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    state.energy = (state.energy + state.energy_regen * dt_seconds).min(state.energy_max);
}
