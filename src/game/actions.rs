use super::balance::{self, Cost};
use super::items;
use super::state::{EquipError, GameState, ResearchKind, ResourceKind};
use super::trader;

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

/// Ошибка крафта.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    UnknownRecipe,
    NotEnough,
    NoSpace,
}

impl CraftError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownRecipe => "Такого рецепта нет.",
            Self::NotEnough => "Не хватает ингредиентов.",
            Self::NoSpace => "В сумке нет места для результата.",
        }
    }
}

/// Ошибка похода.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpeditionError {
    NoEnergy,
    NoSpace,
}

impl ExpeditionError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NoEnergy => "Не хватает энергии на поход.",
            Self::NoSpace => "В сумке нет места для добычи.",
        }
    }
}

/// Результат перерождения.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrestigeResult {
    Done(u32),
    NotEnough,
    Locked,
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

    let yield_amount =
        (balance::CLICK_YIELD + state.dev.gather_yield) * state.production_multiplier();
    let added = state.add_resource(kind, yield_amount);
    state.stats.clicks = state.stats.clicks.saturating_add(1);
    match kind {
        ResourceKind::Wood => state.stats.wood_gathered += added,
        ResourceKind::Stone => state.stats.stone_gathered += added,
        ResourceKind::Coal => state.stats.coal_produced += added,
        ResourceKind::Iron => state.stats.iron_mined += added,
        ResourceKind::Gold => state.stats.gold_earned += added,
    }
    state.refresh_unlocks();
    GatherResult::Gathered(added)
}

/// Кнопка «Отдых»: восстанавливает энергию и считается для открытия добычи.
pub fn rest(state: &mut GameState) -> f64 {
    let gained = state.add_energy(state.rest_energy());
    state.rest_clicks = state.rest_clicks.saturating_add(1);
    state.stats.rests = state.stats.rests.saturating_add(1);
    gained
}

/// Один игровой тик: энергия, лесопилка, исследование и угольная яма.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    let dt = dt_seconds.max(0.0);
    state.stats.play_seconds += dt;

    let regen = state.energy_regen();
    if regen > 0.0 {
        state.energy = (state.energy + regen * dt).min(state.max_energy());
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

    let sawmill = state.sawmill_wood_per_sec() * state.production_multiplier() * dt;
    if sawmill > 0.0 {
        let added = state.add_resource(ResourceKind::Wood, sawmill);
        state.stats.wood_gathered += added;
    }

    let mine = state.mine_iron_per_sec() * dt;
    if mine > 0.0 {
        let added = state.add_resource(ResourceKind::Iron, mine);
        state.stats.iron_mined += added;
    }

    tick_research(state, dt);
    tick_coal_pit(state, dt);
    tick_forge_fire(state, dt);
    trader::tick_markets(state, dt);
}

/// Продвигает все доступные изучения.
fn tick_research(state: &mut GameState, dt: f64) {
    for kind in ResearchKind::ALL {
        if state.research_done(kind) || !state.research_available(kind) {
            continue;
        }

        let speed = state.research_speed(kind);
        if speed <= 0.0 {
            continue;
        }

        let cost = state.research_cost(kind);
        let progress = (state.research_progress(kind) + speed * dt).min(cost);
        state.set_research_progress(kind, progress);

        if progress >= cost {
            state.complete_research(kind);
        }
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

    state.coal_pit_progress += state.coal_pit_workers as f64 * dt / balance::COAL_PIT_BATCH_SECONDS;

    while state.coal_pit_progress >= 1.0 {
        state.coal_pit_progress -= 1.0;

        if state.wood + 1e-9 < balance::COAL_PIT_WOOD_PER_BATCH
            || state.coal >= state.cap(ResourceKind::Coal)
        {
            state.coal_pit_progress = 0.0;
            break;
        }

        state.wood -= balance::COAL_PIT_WOOD_PER_BATCH;
        let output = balance::COAL_PIT_COAL_PER_BATCH * state.production_multiplier();
        let added = state.add_resource(ResourceKind::Coal, output);
        state.stats.coal_produced += added;
        state.coal_seen = true;
    }
}

/// Гасит огонь кузницы и при необходимости подбрасывает уголь.
fn tick_forge_fire(state: &mut GameState, dt: f64) {
    if state.forge_level == 0 {
        state.forge_fire = 0.0;
        return;
    }

    // Автоподкидка: один уголь, когда огонь полностью погас.
    if state.forge_auto_feed && state.forge_fire <= 0.0 && state.coal >= 1.0 {
        state.coal -= 1.0;
        state.forge_fire = balance::FIRE_SECONDS_PER_COAL.min(state.fire_max());
    }

    state.forge_fire = (state.forge_fire - dt).max(0.0);
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

/// Строит или улучшает «Уют»: повышает максимум энергии.
pub fn build_comfort(state: &mut GameState) -> BuildResult {
    let cost = balance::comfort_cost(state.comfort_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.comfort_level += 1;
    BuildResult::Built(state.comfort_level)
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

/// Строит или улучшает шахту. Доступна после изучения угольной ямы.
pub fn build_mine(state: &mut GameState) -> BuildResult {
    if !state.coal_pit_researched {
        return BuildResult::Locked;
    }

    let cost = balance::mine_cost(state.mine_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.mine_level += 1;
    BuildResult::Built(state.mine_level)
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

/// Строит или улучшает кузницу. Доступна после изучения.
pub fn build_forge(state: &mut GameState) -> BuildResult {
    if !state.forge_researched {
        return BuildResult::Locked;
    }

    let cost = balance::forge_cost(state.forge_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.forge_level += 1;
    BuildResult::Built(state.forge_level)
}

/// Строит или улучшает банк. Доступен после изучения торговца.
pub fn build_bank(state: &mut GameState) -> BuildResult {
    if !state.trader_researched {
        return BuildResult::Locked;
    }

    let cost = balance::bank_cost(state.bank_level);
    if state.wood < cost.wood || state.stone < cost.stone {
        return BuildResult::NotEnough(cost);
    }

    state.wood -= cost.wood;
    state.stone -= cost.stone;
    state.bank_level += 1;
    BuildResult::Built(state.bank_level)
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

/// Меняет число работников на шахте на `delta`.
pub fn adjust_mine_workers(state: &mut GameState, delta: i32) {
    if !state.coal_pit_researched {
        return;
    }
    state.mine_workers = adjust_count(
        state.mine_workers,
        delta,
        state.workers_free(),
        state.mine_slots(),
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

/// Меняет число исследователей на выбранном направлении.
pub fn adjust_researchers(state: &mut GameState, kind: ResearchKind, delta: i32) {
    if state.research_done(kind) || !state.research_available(kind) {
        return;
    }
    let next = adjust_count(
        state.researchers_for(kind),
        delta,
        state.researchers_free(),
        state.researcher_slots(),
    );
    state.set_researchers(kind, next);
}

/// Подбрасывает один уголь в кузницу. Возвращает `false`, если нельзя.
pub fn add_coal_to_fire(state: &mut GameState) -> bool {
    if state.forge_level == 0 || state.coal + 1e-9 < 1.0 {
        return false;
    }
    let max = state.fire_max();
    if state.forge_fire + 1e-9 >= max {
        return false;
    }

    state.coal -= 1.0;
    state.forge_fire = (state.forge_fire + balance::FIRE_SECONDS_PER_COAL).min(max);
    true
}

/// Включает или выключает автоподкидку угля.
pub fn toggle_forge_auto_feed(state: &mut GameState) {
    state.forge_auto_feed = !state.forge_auto_feed;
}

/// Создаёт предмет по рецепту.
pub fn craft(state: &mut GameState, recipe_id: &str) -> Result<(), CraftError> {
    let recipe = items::recipe(recipe_id).ok_or(CraftError::UnknownRecipe)?;

    for ingredient in &recipe.ingredients {
        if state.item_count(ingredient.item_id) < ingredient.quantity {
            return Err(CraftError::NotEnough);
        }
    }
    if !state.can_add_item(recipe.result_id, recipe.result_quantity) {
        return Err(CraftError::NoSpace);
    }

    for ingredient in &recipe.ingredients {
        if !state.remove_item(ingredient.item_id, ingredient.quantity) {
            return Err(CraftError::NotEnough);
        }
    }
    state.add_item(recipe.result_id, recipe.result_quantity);
    Ok(())
}

/// Надевает предмет из сумки.
pub fn equip_item(state: &mut GameState, item_id: &str) -> Result<(), EquipError> {
    state.equip_item(item_id)
}

/// Снимает предмет из слота.
pub fn unequip_slot(state: &mut GameState, slot: items::EquipSlot) {
    state.unequip_slot(slot);
}

/// Короткий поход: тратит энергию и приносит лут.
///
/// Результат детерминированный: рынок и походы используют одно зерно.
pub fn simulate_expedition(state: &mut GameState) -> Result<Vec<(String, u32)>, ExpeditionError> {
    let cost = balance::EXPEDITION_ENERGY_COST;
    if state.energy + 1e-9 < cost {
        return Err(ExpeditionError::NoEnergy);
    }

    let loot_table = ["iron_ore", "wood", "leather", "quartz", "silver"];
    let mut seed = state
        .market_seed
        .wrapping_add(state.stats.clicks.wrapping_mul(31))
        .wrapping_add((state.stats.play_seconds as u64).wrapping_mul(17));
    let index = (next_random(&mut seed) >> 33) as usize % loot_table.len();
    let base = 1 + ((next_random(&mut seed) >> 33) as u32 % 2);
    let bonus = (state.expedition_bonus * 10.0).floor() as u32;
    let quantity = base + bonus;
    let item_id = loot_table[index];

    if !state.can_add_item(item_id, quantity) {
        return Err(ExpeditionError::NoSpace);
    }
    if !state.spend_energy(cost) {
        return Err(ExpeditionError::NoEnergy);
    }
    state.add_item(item_id, quantity);
    state.market_seed = seed;

    Ok(vec![(item_id.to_string(), quantity)])
}

/// Чит для демо: добавляет по горсти базовых материалов.
///
/// Возвращает число добавленных стопок.
pub fn add_demo_resources(state: &mut GameState) -> u32 {
    let mut added = 0;
    for (item_id, quantity) in [
        ("iron_ore", 5),
        ("wood", 5),
        ("leather", 5),
        ("silver", 1),
        ("quartz", 1),
    ] {
        if state.add_item(item_id, quantity) {
            added += 1;
        }
    }
    added
}

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed
}

/// Начинает новый цикл. Возвращает новый номер цикла.
pub fn prestige(state: &mut GameState) -> PrestigeResult {
    if state.forge_level == 0 {
        return PrestigeResult::Locked;
    }
    if state.iron + 1e-9 < balance::CYCLE_IRON_REQUIREMENT {
        return PrestigeResult::NotEnough;
    }

    state.reset_for_cycle();
    PrestigeResult::Done(state.cycles)
}

/// Сколько времени даёт оффлайн за указанное число секунд.
pub fn offline_time(elapsed_seconds: f64) -> f64 {
    let capped = elapsed_seconds.clamp(0.0, balance::OFFLINE_MAX_SECONDS);
    capped / balance::OFFLINE_SECONDS_PER_TIME
}

/// Текущее время в Unix-секундах.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Тратит единицу времени и прокручивает 120 секунд игры.
pub fn time_warp(state: &mut GameState) -> bool {
    if state.time + 1e-9 < 1.0 {
        return false;
    }
    state.time -= 1.0;
    tick(state, balance::TIME_WARP_SECONDS);
    true
}

/// Тратит единицу времени на изучение выбранного направления.
pub fn research_time_warp(state: &mut GameState, kind: ResearchKind) -> bool {
    if state.time + 1e-9 < 1.0 || state.research_done(kind) || !state.research_available(kind) {
        return false;
    }

    state.time -= 1.0;
    let cost = state.research_cost(kind);
    let progress = (state.research_progress(kind) + balance::RESEARCH_TIME_WARP).min(cost);
    state.set_research_progress(kind, progress);
    if progress >= cost {
        state.complete_research(kind);
    }
    true
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
        assert_eq!(
            gather(&mut state, ResourceKind::Stone),
            GatherResult::Locked
        );
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
        let mut state = GameState {
            worker_house_level: 1,
            sawmill_level: 1,
            sawmill_workers: 1,
            ..GameState::default()
        };

        tick(&mut state, 10.0);

        assert_eq!(state.wood, 4.0);
        assert_eq!(state.sawmill_wood_per_sec(), 0.4);
    }

    #[test]
    fn workers_cannot_exceed_slots() {
        let mut state = GameState {
            worker_house_level: 1,
            sawmill_level: 2,
            ..GameState::default()
        };

        adjust_sawmill_workers(&mut state, 5);
        assert_eq!(state.sawmill_workers, 1);

        adjust_coal_pit_workers(&mut state, 1);
        assert_eq!(state.coal_pit_workers, 0);

        adjust_sawmill_workers(&mut state, -1);
        assert_eq!(state.sawmill_workers, 0);
    }

    #[test]
    fn research_needs_researcher_and_unlocks_coal_pit() {
        let mut state = GameState {
            research_house_level: 1,
            ..GameState::default()
        };

        tick(&mut state, 10.0);
        assert_eq!(state.coal_pit_research, 0.0);

        adjust_researchers(&mut state, ResearchKind::CoalPit, 1);
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
        let mut state = GameState {
            coal_pit_researched: true,
            coal_pit_level: 1,
            coal_pit_workers: 1,
            wood: 10.0,
            ..GameState::default()
        };

        tick(&mut state, balance::COAL_PIT_BATCH_SECONDS);

        assert_eq!(state.wood, 9.0);
        assert_eq!(state.coal, 1.0);
        assert!(state.coal_seen);
    }

    #[test]
    fn coal_pit_stops_without_wood() {
        let mut state = GameState {
            coal_pit_researched: true,
            coal_pit_level: 1,
            coal_pit_workers: 1,
            wood: 0.0,
            ..GameState::default()
        };

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

    #[test]
    fn mine_workers_produce_iron() {
        let mut state = GameState {
            coal_pit_researched: true,
            mine_level: 1,
            mine_workers: 1,
            ..GameState::default()
        };

        tick(&mut state, 10.0);

        assert!((state.iron - 2.0).abs() < 1e-9);
        assert!((state.stats.iron_mined - 2.0).abs() < 1e-9);
    }

    #[test]
    fn forge_fire_burns_and_auto_feeds() {
        let mut state = GameState {
            forge_level: 1,
            coal: 1.0,
            ..GameState::default()
        };

        assert!(add_coal_to_fire(&mut state));
        assert!((state.forge_fire - balance::FIRE_SECONDS_PER_COAL).abs() < 1e-9);

        tick(&mut state, 5.0);
        assert!((state.forge_fire - (balance::FIRE_SECONDS_PER_COAL - 5.0)).abs() < 1e-9);

        state.forge_fire = 0.0;
        state.forge_auto_feed = true;
        state.coal = 1.0;
        tick(&mut state, 1.0);

        assert_eq!(state.coal, 0.0);
        assert!((state.forge_fire - (balance::FIRE_SECONDS_PER_COAL - 1.0)).abs() < 1e-9);
    }

    #[test]
    fn craft_consumes_ingredients_and_adds_result() {
        let mut state = GameState::default();

        assert_eq!(craft(&mut state, "iron_ingot"), Ok(()));
        assert_eq!(state.item_count("iron_ore"), 3);
        assert_eq!(state.item_count("wood"), 2);
        assert_eq!(state.item_count("iron_ingot"), 1);
    }

    #[test]
    fn craft_rejects_missing_ingredients() {
        let mut state = GameState::default();
        state.inventory.clear();

        assert_eq!(craft(&mut state, "iron_ingot"), Err(CraftError::NotEnough));
    }

    #[test]
    fn equip_changes_trade_bonus_and_respects_slot() {
        let mut state = GameState::default();
        state.add_item("trader_ring", 1);

        assert_eq!(equip_item(&mut state, "trader_ring"), Ok(()));
        assert_eq!(
            state.equipped(items::EquipSlot::Accessory),
            Some("trader_ring")
        );
        assert!((state.trade_bonus - 0.10).abs() < 1e-6);

        state.add_item("coal_amulet", 1);
        assert_eq!(
            equip_item(&mut state, "coal_amulet"),
            Err(EquipError::SlotOccupied)
        );

        unequip_slot(&mut state, items::EquipSlot::Accessory);
        assert_eq!(equip_item(&mut state, "coal_amulet"), Ok(()));
        assert!((state.trade_bonus - 0.05).abs() < 1e-6);
    }

    #[test]
    fn backpack_raises_inventory_slots() {
        let mut state = GameState::default();
        state.add_item("backpack", 1);
        let before = state.inventory_slots();

        assert_eq!(equip_item(&mut state, "backpack"), Ok(()));
        assert_eq!(state.inventory_slots(), before + 10);
    }

    #[test]
    fn expedition_spends_energy_and_adds_loot() {
        let mut state = GameState {
            energy: 20.0,
            ..GameState::default()
        };
        let before = state.energy;

        let loot = simulate_expedition(&mut state).expect("поход");
        assert_eq!(state.energy, before - balance::EXPEDITION_ENERGY_COST);
        assert_eq!(loot.len(), 1);
        assert!(state.item_count(&loot[0].0) >= loot[0].1);
    }

    #[test]
    fn expedition_rejects_without_energy() {
        let mut state = GameState {
            energy: 0.0,
            ..GameState::default()
        };

        assert_eq!(
            simulate_expedition(&mut state),
            Err(ExpeditionError::NoEnergy)
        );
    }

    #[test]
    fn comfort_raises_energy_cap() {
        let mut state = GameState {
            wood: 1000.0,
            stone: 1000.0,
            ..GameState::default()
        };
        let before = state.max_energy();
        assert_eq!(build_comfort(&mut state), BuildResult::Built(1));
        assert_eq!(state.max_energy(), before + balance::ENERGY_CAP_PER_COMFORT);
        assert_eq!(state.comfort_level, 1);
    }

    #[test]
    fn bank_raises_gold_cap() {
        let mut state = GameState::default();
        let before = state.gold_cap();
        state.bank_level = 1;
        assert_eq!(state.gold_cap(), before + balance::GOLD_CAP_PER_LEVEL);
    }

    #[test]
    fn prestige_resets_progress_and_keeps_cycles() {
        let mut state = GameState {
            forge_level: 1,
            iron: balance::CYCLE_IRON_REQUIREMENT,
            time: 5.0,
            ..GameState::default()
        };

        assert_eq!(prestige(&mut state), PrestigeResult::Done(1));
        assert_eq!(state.cycles, 1);
        assert_eq!(state.forge_level, 0);
        assert_eq!(state.iron, 0.0);
        assert_eq!(state.time, 5.0);
        assert!((state.production_multiplier() - 1.1).abs() < 1e-9);
    }

    #[test]
    fn offline_time_is_capped() {
        assert_eq!(offline_time(0.0), 0.0);
        assert_eq!(offline_time(3600.0), 60.0);
        assert_eq!(
            offline_time(balance::OFFLINE_MAX_SECONDS * 2.0),
            balance::OFFLINE_MAX_SECONDS / balance::OFFLINE_SECONDS_PER_TIME
        );
    }

    #[test]
    fn time_warp_spends_time_and_ticks() {
        let mut state = GameState {
            time: 1.0,
            ..GameState::default()
        };

        assert!(time_warp(&mut state));
        assert_eq!(state.time, 0.0);
        assert!((state.stats.play_seconds - balance::TIME_WARP_SECONDS).abs() < 1e-9);
        assert!(!time_warp(&mut state));
    }

    #[test]
    fn research_time_warp_adds_experience() {
        let mut state = GameState {
            time: 1.0,
            ..GameState::default()
        };

        assert!(research_time_warp(&mut state, ResearchKind::CoalPit));
        assert_eq!(state.time, 0.0);
        assert!((state.coal_pit_research - balance::RESEARCH_TIME_WARP).abs() < 1e-9);
    }
}
