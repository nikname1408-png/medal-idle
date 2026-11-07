use super::balance;

/// Всё состояние игры. Логика не зависит от Dioxus.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub energy: f64,
    pub energy_max: f64,
    pub energy_regen: f64,

    pub wood: f64,
    pub stone: f64,
    pub iron: f64,
    pub coal: f64,
    pub gold: f64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            energy: balance::START_ENERGY,
            energy_max: balance::START_ENERGY_MAX,
            energy_regen: balance::START_ENERGY_REGEN,
            wood: 0.0,
            stone: 0.0,
            iron: 0.0,
            coal: 0.0,
            gold: 0.0,
        }
    }
}

impl GameState {
    /// Доля заполнения полосы энергии от 0.0 до 1.0.
    pub fn energy_ratio(&self) -> f64 {
        if self.energy_max <= 0.0 {
            0.0
        } else {
            (self.energy / self.energy_max).clamp(0.0, 1.0)
        }
    }
}
