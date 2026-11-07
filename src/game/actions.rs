use super::balance;
use super::state::{GameState, ResourceKind};

/// Результат клика по ресурсу.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GatherResult {
    /// Успешно добыто указанное количество.
    Gathered(f64),
    /// Не хватило энергии.
    NoEnergy,
    /// Ресурс уже упёрся в лимит склада.
    StorageFull,
}

/// Клик по ресурсу: тратит энергию и добавляет ресурс.
pub fn gather(state: &mut GameState, kind: ResourceKind) -> GatherResult {
    if state.amount(kind) >= state.cap(kind) {
        return GatherResult::StorageFull;
    }

    if !state.spend_energy(balance::CLICK_ENERGY_COST) {
        return GatherResult::NoEnergy;
    }

    let added = state.add_resource(kind, balance::CLICK_YIELD);
    GatherResult::Gathered(added)
}

/// Кнопка «Отдых»: восстанавливает энергию. Возвращает прибавку.
pub fn rest(state: &mut GameState) -> f64 {
    state.add_energy(balance::REST_ENERGY)
}

/// Один игровой тик: регенерация энергии.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    state.energy = (state.energy + state.energy_regen * dt_seconds).min(state.energy_max);
}
