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
    if kind == ResourceKind::Iron || kind == ResourceKind::Coal {
        return GatherResult::Locked;
    }

    if state.amount(kind) >= state.cap(kind) {
        return GatherResult::StorageFull;
    }

    if !state.spend_energy(balance::CLICK_ENERGY_COST) {
        return GatherResult::NoEnergy;
    }

    let yield_amount = balance::CLICK_YIELD + state.dev.gather_yield;
    let added = state.add_resource(kind, yield_amount);
    state.refresh_unlocks();
    GatherResult::Gathered(added)
}

/// Кнопка «Отдых»: восстанавливает энергию и считается для открытия добычи.
pub fn rest(state: &mut GameState) -> f64 {
    let gained = state.add_energy(state.rest_energy());
    state.rest_clicks = state.rest_clicks.saturating_add(1);
    gained
}

/// Один игровой тик: энергия, лесопилка, исследование и угольная яма.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    let dt = dt_seconds.max(0.0);

    let regen = state.energy_regen();
    if regen > 0.0 {
        state.energy = (state.energy + regen * dt).min(state.energy_max);
    }

    let wood = state.dev.wood_per_sec;
    let stone = state.dev.stone_per_sec;
    let iron = state.dev.iron_per_sec;
    let coal = state.dev.coal_per_sec;
    let gold = state.dev.gold_per_sec;

    state.wood += wood * dt;
    state.stone += stone * dt;
    state.iron += iron * dt;
    state.coal += coal * dt;
    state.gold += gold * dt;

    let sawmill = state.sawmill_wood_per_sec() * dt;
    if sawmill > 0.0 {
        state.add_resource(ResourceKind::Wood, sawmill);
    }

    tick_research(state, dt);
    tick_coal_pit(state, dt);
}

/// Продвигает изучение угольной ямы, если назначены исследователи.
fn tick_research(state: &mut GameState, dt: f64) {
    if state.coal_pit_researched {
        return;
    }

    let speed = state.coal_pit_research_speed();
    if speed <= 0.0 {
        return;
    }

    state.coal_pit_research =
        (state.coal_pit_research + speed * dt).min(balance::COAL_PIT_RESEARCH_COST);

    if state.coal_pit_research >= balance::COAL_PIT_RESEARCH_COST {
        state.coal_pit_researched = true;
        state.coal_pit_level = state.coal_pit_level.max(1);
        state.coal_seen = true;
        state.coal_pit_researchers = 0;
    }
}

/// Продвигает партии угольной ямы: тратит дерево, добавляет уголь.
fn tick_coal_pit(state: &mut GameState, dt: f64) {
    if !state.coal_pit_researched || state.coal_pit_workers == 0 {
        return;
    }

    let coal_cap = state.cap(ResourceKind::Coal);
    if state.coal >= coal_cap {
        state.coal_pit_progress = 0.0;
        return;
    }

    if state.wood + 1e-9 < balance::COAL_PIT_WOOD_PER_BATCH {
        return;
    }

    state.coal_pit_progress +=
        state.coal_pit_workers as f64 * dt / balance::COAL_PIT_BATCH_SECONDS;

    while state.coal_pit_progress >= 1.0 {
        state.coal_pit_progress -= 1.0;

        if state.wood + 1e-9 < balance::COAL_PIT_WOOD_PER_BATCH
            || state.coal >= state.cap(ResourceKind::Coal)
        {
            state.coal_pit_progress = 0.0;
            break;
        }

        state.wood -= balance::COAL_PIT_WOOD_PER_BATCH;
        state.add_resource(ResourceKind::Coal, balance::COAL_PIT_COAL_PER_BATCH);
        state.coal_seen = true;
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

/// Строит или улучшает дом для работников.
pub fn build_worker_house(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::worker_house_cost(state.worker_house_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.worker_house_level += 1;
    BuildResult::Built(state.worker_house_level)
}

/// Строит или улучшает дом для изучения.
pub fn build_research_house(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::research_house_cost(state.research_house_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.research_house_level += 1;
    BuildResult::Built(state.research_house_level)
}

/// Строит или улучшает лесопилку.
pub fn build_sawmill(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::sawmill_cost(state.sawmill_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.sawmill_level += 1;
    BuildResult::Built(state.sawmill_level)
}

/// Строит или улучшает угольную яму. Доступна после изучения.
pub fn build_coal_pit(state: &mut GameState) -> BuildResult {
    if !state.coal_pit_researched {
        return BuildResult::Locked;
    }

    let cost = balance::coal_pit_cost(state.coal_pit_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.coal_pit_level += 1;
    BuildResult::Built(state.coal_pit_level)
}

/// Меняет число работников на лесопилке на `delta`, не выходя за лимиты.
pub fn adjust_sawmill_workers(state: &mut GameState, delta: i32) {
    state.sawmill_workers = adjust_count(
        state.sawmill_workers,
        delta,
        state.workers_free(),
        state.sawmill_slots(),
    );
}

/// Меняет число работников на угольной яме на `delta`.
pub fn adjust_coal_pit_workers(state: &mut GameState, delta: i32) {
    if !state.coal_pit_researched {
        return;
    }
    state.coal_pit_workers = adjust_count(
        state.coal_pit_workers,
        delta,
        state.workers_free(),
        state.coal_pit_slots(),
    );
}

/// Меняет число исследователей на изучении угольной ямы.
pub fn adjust_coal_pit_researchers(state: &mut GameState, delta: i32) {
    if state.coal_pit_researched {
        return;
    }
    state.coal_pit_researchers = adjust_count(
        state.coal_pit_researchers,
        delta,
        state.researchers_free(),
        state.researcher_slots(),
    );
}

/// Прибавляет или убавляет счётчик в пределах свободных слотов и ёмкости.
fn adjust_count(current: u32, delta: i32, free: u32, capacity: u32) -> u32 {
    if delta >= 0 {
        let room = capacity.saturating_sub(current).min(free);
        current + (delta as u32).min(room)
    } else {
        current.saturating_sub((-delta) as u32)
    }
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
        assert_eq!(state.energy, 10.0);
    }

    #[test]
    fn rest_gives_one_energy_at_start() {
        let mut state = GameState::default();
        let gained = rest(&mut state);
        assert_eq!(gained, 1.0);
        assert_eq!(state.energy, 1.0);
    }

    #[test]
    fn gather_spends_energy_and_adds_resource() {
        let mut state = rested_state();
        let result = gather(&mut state, ResourceKind::Wood);
        assert_eq!(result, GatherResult::Gathered(1.0));
        assert_eq!(state.wood, 1.0);
        assert_eq!(state.energy, 9.0);
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
        state.wood = 15.0;
        state.stone = 8.0;
        state.stone_seen = true;

        assert_eq!(build_house(&mut state), BuildResult::Built(1));
        assert_eq!(state.house_level, 1);
        assert_eq!(state.energy_regen(), 0.5);
        assert_eq!(state.rest_energy(), 1.5);
        assert_eq!(state.wood, 0.0);
        assert_eq!(state.stone, 0.0);
    }

    #[test]
    fn dev_speed_adds_resources() {
        let mut state = GameState::default();
        state.dev.wood_per_sec = 5.0;
        tick(&mut state, 2.0);
        assert_eq!(state.wood, 10.0);
    }

    #[test]
    fn dev_energy_regen_adds_energy() {
        let mut state = GameState::default();
        state.dev.energy_regen = 2.0;
        tick(&mut state, 3.0);
        assert_eq!(state.energy, 6.0);
    }

    #[test]
    fn sawmill_workers_produce_wood() {
        let mut state = GameState::default();
        state.worker_house_level = 1;
        state.sawmill_level = 1;
        state.sawmill_workers = 1;

        tick(&mut state, 10.0);

        assert_eq!(state.wood, 4.0);
        assert_eq!(state.sawmill_wood_per_sec(), 0.4);
    }

    #[test]
    fn workers_cannot_exceed_slots() {
        let mut state = GameState::default();
        state.worker_house_level = 1;
        state.sawmill_level = 2;

        adjust_sawmill_workers(&mut state, 5);
        assert_eq!(state.sawmill_workers, 1);

        adjust_coal_pit_workers(&mut state, 1);
        assert_eq!(state.coal_pit_workers, 0);

        adjust_sawmill_workers(&mut state, -1);
        assert_eq!(state.sawmill_workers, 0);
    }

    #[test]
    fn research_needs_researcher_and_unlocks_coal_pit() {
        let mut state = GameState::default();
        state.research_house_level = 1;

        tick(&mut state, 10.0);
        assert_eq!(state.coal_pit_research, 0.0);

        adjust_coal_pit_researchers(&mut state, 1);
        assert_eq!(state.coal_pit_researchers, 1);

        tick(
            &mut state,
            balance::COAL_PIT_RESEARCH_COST / balance::RESEARCH_PER_RESEARCHER_PER_SEC,
        );

        assert!(state.coal_pit_researched);
        assert_eq!(state.coal_pit_level, 1);
        assert_eq!(state.coal_pit_researchers, 0);
    }

    #[test]
    fn coal_pit_turns_wood_into_coal() {
        let mut state = GameState::default();
        state.coal_pit_researched = true;
        state.coal_pit_level = 1;
        state.coal_pit_workers = 1;
        state.wood = 10.0;

        tick(&mut state, balance::COAL_PIT_BATCH_SECONDS);

        assert_eq!(state.wood, 9.0);
        assert_eq!(state.coal, 1.0);
        assert!(state.coal_seen);
    }

    #[test]
    fn coal_pit_stops_without_wood() {
        let mut state = GameState::default();
        state.coal_pit_researched = true;
        state.coal_pit_level = 1;
        state.coal_pit_workers = 1;
        state.wood = 0.0;

        tick(&mut state, balance::COAL_PIT_BATCH_SECONDS * 2.0);

        assert_eq!(state.coal, 0.0);
        assert_eq!(state.coal_pit_progress, 0.0);
    }

    #[test]
    fn storage_raises_cap() {
        let mut state = rested_state();
        state.wood = 30.0;
        state.stone = 15.0;
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

    #[test]
    fn dev_settings_are_not_saved() {
        let mut state = GameState::default();
        state.dev.wood_per_sec = 42.0;

        let json = serde_json::to_string(&state).expect("сериализация");
        assert!(!json.contains("wood_per_sec"));

        let restored: GameState = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(restored.dev, crate::game::state::DevSettings::default());
    }
}
