use super::balance::{self, Cost};
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
    /// Ресурс ещё не открыт.
    Locked,
}

/// Результат попытки постройки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildResult {
    /// Построено/улучшено до указанного уровня.
    Built(u32),
    /// Не хватает ресурсов; возвращается требуемая стоимость.
    NotEnough(Cost),
    /// Постройка ещё не открыта.
    Locked,
    /// Достигнут максимальный уровень (заготовка на будущее).
    #[allow(dead_code)]
    MaxLevel,
}

/// Клик по ресурсу: тратит энергию и добавляет ресурс.
pub fn gather(state: &mut GameState, kind: ResourceKind) -> GatherResult {
    if !state.gathering_unlocked() {
        return GatherResult::Locked;
    }
    if kind == ResourceKind::Stone && !state.stone_unlocked() {
        return GatherResult::Locked;
    }
    if kind == ResourceKind::Iron {
        return GatherResult::Locked;
    }

    if state.amount(kind) >= state.cap(kind) {
        return GatherResult::StorageFull;
    }

    if !state.spend_energy(balance::CLICK_ENERGY_COST) {
        return GatherResult::NoEnergy;
    }

    let added = state.add_resource(kind, balance::CLICK_YIELD);
    state.refresh_unlocks();
    GatherResult::Gathered(added)
}

/// Кнопка «Отдых»: восстанавливает энергию и считается для открытия добычи.
pub fn rest(state: &mut GameState) -> f64 {
    let gained = state.add_energy(balance::REST_ENERGY);
    state.rest_clicks = state.rest_clicks.saturating_add(1);
    gained
}

/// Один игровой тик: регенерация энергии от дома.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    let regen = state.energy_regen();
    if regen > 0.0 {
        state.energy = (state.energy + regen * dt_seconds).min(state.energy_max);
    }
}

/// Строит или улучшает свой дом.
pub fn build_house(state: &mut GameState) -> BuildResult {
    if !state.stone_unlocked() {
        return BuildResult::Locked;
    }

    let cost = balance::house_cost(state.house_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.house_level += 1;
    BuildResult::Built(state.house_level)
}

/// Строит или улучшает склад.
pub fn build_storage(state: &mut GameState) -> BuildResult {
    if !state.stone_unlocked() {
        return BuildResult::Locked;
    }

    let cost = balance::storage_cost(state.storage_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.storage_level += 1;
    BuildResult::Built(state.storage_level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::state::Theme;

    fn rested_state() -> GameState {
        let mut state = GameState::default();
        for _ in 0..balance::REST_CLICKS_TO_UNLOCK_GATHER {
            rest(&mut state);
        }
        state
    }

    #[test]
    fn energy_does_not_regen_without_house() {
        let mut state = GameState::default();
        tick(&mut state, 10.0);
        assert_eq!(state.energy, 0.0);
        assert_eq!(state.energy_regen(), 0.0);
    }

    #[test]
    fn ten_rests_unlock_gathering() {
        let state = rested_state();
        assert!(state.gathering_unlocked());
        assert_eq!(state.energy, 100.0);
    }

    #[test]
    fn gather_spends_energy_and_adds_resource() {
        let mut state = rested_state();
        let result = gather(&mut state, ResourceKind::Wood);
        assert_eq!(result, GatherResult::Gathered(1.0));
        assert_eq!(state.wood, 1.0);
        assert_eq!(state.energy, 99.0);
    }

    #[test]
    fn stone_unlocks_after_two_wood() {
        let mut state = rested_state();
        gather(&mut state, ResourceKind::Wood);
        assert!(!state.stone_unlocked());
        gather(&mut state, ResourceKind::Wood);
        assert!(state.stone_unlocked());
    }

    #[test]
    fn stone_is_locked_before_two_wood() {
        let mut state = rested_state();
        assert_eq!(gather(&mut state, ResourceKind::Stone), GatherResult::Locked);
    }

    #[test]
    fn house_costs_resources_and_gives_regen() {
        let mut state = rested_state();
        state.wood = 10.0;
        state.stone = 5.0;
        state.stone_seen = true;

        assert_eq!(build_house(&mut state), BuildResult::Built(1));
        assert_eq!(state.house_level, 1);
        assert_eq!(state.energy_regen(), 1.0);
        assert_eq!(state.wood, 0.0);
        assert_eq!(state.stone, 0.0);
    }

    #[test]
    fn storage_raises_cap() {
        let mut state = rested_state();
        state.wood = 20.0;
        state.stone = 10.0;
        state.stone_seen = true;

        let before = state.cap(ResourceKind::Wood);
        assert_eq!(build_storage(&mut state), BuildResult::Built(1));
        assert_eq!(
            state.cap(ResourceKind::Wood),
            before + balance::STORAGE_CAP_PER_LEVEL
        );
    }

    #[test]
    fn save_roundtrip_keeps_state() {
        let mut state = rested_state();
        state.wood = 3.0;
        state.theme = Theme::Dark;

        let json = serde_json::to_string(&state).expect("сериализация");
        let restored: GameState = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(state, restored);
    }
}
