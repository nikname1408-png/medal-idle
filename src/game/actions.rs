use super::balance::{self, Cost};
use super::events;
use super::expedition;
use super::i18n;
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

/// Почему карту локации не удалось изучить.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudyMapError {
    /// Предмет не является картой локации.
    NotMap,
    /// Такой карты нет в сумке.
    Missing,
    /// Локация уже открыта — карту тратить не нужно.
    AlreadyOpen,
}

impl StudyMapError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotMap => i18n::t("err.not_map"),
            Self::Missing => i18n::t("err.map_missing"),
            Self::AlreadyOpen => i18n::t("err.map_open"),
        }
    }
}

/// Почему чертёж паровой машины не удалось изучить.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudyBlueprintError {
    /// Предмет не является чертежом паровой машины.
    NotBlueprint,
    /// Чертёж уже изучен — рецепт открыт.
    AlreadyStudied,
    /// Чертежа нет в сумке.
    Missing,
}

impl StudyBlueprintError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotBlueprint => i18n::t("err.not_blueprint"),
            Self::AlreadyStudied => i18n::t("err.blueprint_studied"),
            Self::Missing => i18n::t("err.blueprint_missing"),
        }
    }
}

/// Предложение постройки, которое показывается игроку в центре экрана.
///
/// Тексты берутся из словаря: `letter.<id>.title` и `letter.<id>.body`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfferDef {
    pub id: &'static str,
    pub cost: Cost,
}

impl OfferDef {
    /// Заголовок предложения на текущем языке.
    pub fn title(&self) -> &'static str {
        i18n::t_or(&format!("letter.offer_{}.title", self.id), "")
    }

    /// Текст предложения на текущем языке.
    pub fn body(&self) -> &'static str {
        i18n::t_or(&format!("letter.offer_{}.body", self.id), "")
    }
}

pub const OFFER_COAL_PIT: &str = "coal_pit";
pub const OFFER_FORGE: &str = "forge";
pub const OFFER_BANK: &str = "bank";

/// Описание предложения по его ID.
pub fn offer_def(id: &str) -> Option<OfferDef> {
    match id {
        OFFER_COAL_PIT => Some(OfferDef {
            id: OFFER_COAL_PIT,
            cost: balance::coal_pit_cost(0),
        }),
        OFFER_FORGE => Some(OfferDef {
            id: OFFER_FORGE,
            cost: balance::forge_cost(0),
        }),
        OFFER_BANK => Some(OfferDef {
            id: OFFER_BANK,
            cost: balance::bank_cost(0),
        }),
        _ => None,
    }
}

/// Показывает предложение, если экран свободен и письма с ним ещё нет.
fn push_offer(state: &mut GameState, id: &'static str) -> bool {
    if state.pending_offer.is_some() {
        return false;
    }
    if state
        .letters
        .iter()
        .any(|letter| letter.offer.as_deref() == Some(id))
    {
        return false;
    }
    state.pending_offer = Some(id.to_string());
    true
}

/// Проверяет условия новых предложений построек.
fn check_offers(state: &mut GameState) {
    if state.house_level > 0 && !state.coal_pit_unlocked {
        push_offer(state, OFFER_COAL_PIT);
    }
    if state.coal_pit_level > 0 && !state.forge_unlocked {
        push_offer(state, OFFER_FORGE);
    }
    if !state.bank_seen
        && !state.bank_unlocked
        && state.gold + 1e-9 >= balance::BANK_OFFER_GOLD
        && push_offer(state, OFFER_BANK)
    {
        state.bank_seen = true;
    }
}

/// Оплачивает открытие постройки из текущего предложения.
/// При успехе закрывает письмо. Уровень остаётся 0 — строить нужно отдельно.
pub fn build_pending_offer(state: &mut GameState) -> BuildResult {
    let Some(id) = state.pending_offer.take() else {
        return BuildResult::Locked;
    };
    let result = match id.as_str() {
        OFFER_COAL_PIT => unlock_coal_pit(state),
        OFFER_FORGE => unlock_forge(state),
        OFFER_BANK => unlock_bank(state),
        _ => BuildResult::Locked,
    };
    if matches!(result, BuildResult::Built(_)) {
        state
            .letters
            .retain(|letter| letter.offer.as_deref() != Some(id.as_str()));
    } else {
        state.pending_offer = Some(id);
    }
    result
}

/// Откладывает предложение в колокольчик («Позже»).
pub fn defer_pending_offer(state: &mut GameState) {
    let Some(id) = state.pending_offer.take() else {
        return;
    };
    let Some(def) = offer_def(&id) else {
        return;
    };
    state.add_letter(&format!("offer_{}", def.id), Some(def.id));
}

/// Ошибка крафта.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    UnknownRecipe,
    NotEnough,
    NoSpace,
    /// Не хватает огня кузницы.
    NoFire,
    /// Чертёж паровой машины ещё не изучен.
    BlueprintLocked,
    /// Прототип паровой машины уже собран.
    AlreadyBuilt,
    /// Уровень кузнеца ниже, чем требует рецепт. Внутри — нужный уровень.
    Locked(u32),
}

impl CraftError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownRecipe => i18n::t("err.unknown_recipe"),
            Self::NotEnough => i18n::t("err.not_enough"),
            Self::NoSpace => i18n::t("err.no_space"),
            Self::NoFire => i18n::t("err.no_fire"),
            Self::BlueprintLocked => i18n::t("err.blueprint_locked"),
            Self::AlreadyBuilt => i18n::t("err.already_built"),
            Self::Locked(_) => i18n::t("err.recipe_locked"),
        }
    }
}

/// Результат перерождения.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrestigeResult {
    Done(u32),
    /// Не добрал дерева и камня до порога цикла.
    NotEnough,
    /// Стена R = 5: шахта не построена до нужного уровня.
    WallMine,
    /// Стена R = 10: мало камня сверх порога.
    WallStone,
}

/// Хватает ли ресурсов, предметов, энергии и золота на постройку.
pub fn can_afford(state: &GameState, cost: Cost) -> bool {
    state.active_amount("wood") as f64 >= cost.wood
        && state.active_amount("stone") as f64 >= cost.stone
        && state.gold + 1e-9 >= cost.gold
        && state.energy + 1e-9 >= cost.energy
        && state.item_count("board") >= cost.boards
        && state.item_count("iron_ingot") >= cost.iron_ingots
}

/// Списывает стоимость постройки. Возвращает `false`, если не хватает.
fn spend_cost(state: &mut GameState, cost: Cost) -> bool {
    if !can_afford(state, cost) {
        return false;
    }
    state.spend_active("wood", cost.wood as u32);
    state.spend_active("stone", cost.stone as u32);
    state.gold = (state.gold - cost.gold).max(0.0);
    state.energy = (state.energy - cost.energy).max(0.0);
    state.remove_item("board", cost.boards);
    state.remove_item("iron_ingot", cost.iron_ingots);
    true
}

/// Клик по ресурсу: тратит энергию и добавляет ресурс.
pub fn gather(state: &mut GameState, kind: ResourceKind) -> GatherResult {
    if !state.gathering_unlocked() {
        return GatherResult::Locked;
    }
    if kind == ResourceKind::Stone && !state.stone_unlocked() {
        return GatherResult::Locked;
    }
    if kind == ResourceKind::Fire || kind == ResourceKind::Coal {
        return GatherResult::Locked;
    }

    if state.amount(kind) >= state.cap(kind) {
        return GatherResult::StorageFull;
    }

    // Дебафф «Кривые руки» удорожает клик.
    if !state.spend_energy(balance::CLICK_ENERGY_COST * state.energy_cost_multiplier()) {
        return GatherResult::NoEnergy;
    }

    // Ручная добыча теперь умножается на X, как «Отдых» и «Труд».
    let yield_amount = (balance::CLICK_YIELD + state.dev.gather_yield)
        * (1.0 + state.gather_bonus as f64)
        * state.income_multiplier();
    let added = state.add_produced(kind, yield_amount);
    state.stats.clicks = state.stats.clicks.saturating_add(1);
    match kind {
        ResourceKind::Coal => state.stats.coal_produced += added,
        ResourceKind::Gold => state.stats.gold_earned += added,
        ResourceKind::Wood | ResourceKind::Stone | ResourceKind::Fire => {}
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

/// Изучает карту локации: тратит одну карту и открывает поход.
///
/// Неважно, куплена карта или найдена в походе — открывает именно изучение.
/// Если локация уже открыта, карта не тратится.
pub fn study_map(state: &mut GameState, item_id: &str) -> Result<&'static str, StudyMapError> {
    let Some(location_id) = expedition::location_for_map(item_id) else {
        return Err(StudyMapError::NotMap);
    };
    if state.location_unlocked(location_id) {
        return Err(StudyMapError::AlreadyOpen);
    }
    if !state.remove_item(item_id, 1) {
        return Err(StudyMapError::Missing);
    }
    state.unlock_location(location_id);
    Ok(location_id)
}

/// Изучает чертёж паровой машины: тратит одну штуку и открывает рецепт
/// прототипа в кузнице.
pub fn study_blueprint(state: &mut GameState, item_id: &str) -> Result<(), StudyBlueprintError> {
    if item_id != items::STEAM_BLUEPRINT_ID {
        return Err(StudyBlueprintError::NotBlueprint);
    }
    if state.steam_blueprint_studied {
        return Err(StudyBlueprintError::AlreadyStudied);
    }
    if !state.remove_item(items::STEAM_BLUEPRINT_ID, 1) {
        return Err(StudyBlueprintError::Missing);
    }
    state.steam_blueprint_studied = true;
    Ok(())
}

/// Виден ли рецепт в сетке крафта.
///
/// Рецепт прототипа скрыт, пока чертёж не изучен, и исчезает после сборки:
/// машина собирается один раз.
pub fn recipe_visible(state: &GameState, recipe: &items::Recipe) -> bool {
    if recipe.id == items::STEAM_PROTOTYPE_RECIPE {
        state.steam_blueprint_studied && !state.steam_prototype_built
    } else {
        true
    }
}

/// Закрывает модальное окно «Прототип собран».
pub fn dismiss_steam_event(state: &mut GameState) {
    state.steam_prototype_event = false;
}

/// Переход в Эпоху Пара: паровая машина начинает работать на игрока.
///
/// Возвращает `false`, если прототип ещё не собран или эра уже начата.
pub fn enter_steam_era(state: &mut GameState) -> bool {
    if !state.steam_prototype_built || state.steam_era {
        return false;
    }
    state.steam_era = true;
    state.add_letter("steam_era", None);
    true
}

/// Один игровой тик: энергия, лесопилка, исследование и угольная яма.
pub fn tick(state: &mut GameState, dt_seconds: f64) {
    let dt = dt_seconds.max(0.0);
    state.stats.play_seconds += dt;
    state.total_play_seconds += dt;

    let regen = state.energy_regen();
    if regen > 0.0 {
        state.energy = (state.energy + regen * dt).min(state.max_energy());
    }

    let wood = state.dev.wood_per_sec;
    let stone = state.dev.stone_per_sec;
    let iron = state.dev.iron_per_sec;
    let coal = state.dev.coal_per_sec;
    let gold = state.dev.gold_per_sec;

    state.add_resource(ResourceKind::Wood, wood * dt);
    state.add_resource(ResourceKind::Stone, stone * dt);
    state.add_resource(ResourceKind::Gold, gold * dt);
    // Руда и уголь — инвентарные предметы, а не активные склады.
    if iron > 0.0 {
        let added = state.add_item("iron", (iron * dt).round().max(0.0) as u32);
        if added {
            state.stats.iron_mined += iron * dt;
        }
    }
    if coal > 0.0 {
        let added = state.add_active("coal", (coal * dt).round().max(0.0) as u32);
        state.stats.coal_produced += added as f64;
    }

    // Множитель цикла применяется здесь один раз и только к «Труду».
    let income = state.income_multiplier();

    let sawmill = state.sawmill_wood_per_sec() * income * dt;
    if sawmill > 0.0 {
        state.add_produced(ResourceKind::Wood, sawmill);
    }

    let mine = state.mine_stone_per_sec() * income * dt;
    if mine > 0.0 {
        state.add_produced(ResourceKind::Stone, mine);
    }

    tick_research(state, dt);
    tick_coal_pit(state, dt);
    tick_forge_fire(state, dt);
    trader::tick_markets(state, dt);
    crate::game::quests::tick_quests(state);
    crate::game::quests::tick_collectors(state);
    check_offers(state);
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

    let coal_cap = state.active_cap("coal") as f64;
    if state.active_amount("coal") as f64 >= coal_cap {
        state.coal_pit_progress = 0.0;
        return;
    }

    if state.active_amount("wood") as f64 + 1e-9 < balance::COAL_PIT_WOOD_PER_BATCH {
        return;
    }

    state.coal_pit_progress += state.coal_pit_workers as f64 * dt / balance::COAL_PIT_BATCH_SECONDS;

    while state.coal_pit_progress >= 1.0 {
        state.coal_pit_progress -= 1.0;

        if state.active_amount("wood") as f64 + 1e-9 < balance::COAL_PIT_WOOD_PER_BATCH
            || state.active_amount("coal") as f64 >= state.active_cap("coal") as f64
        {
            state.coal_pit_progress = 0.0;
            break;
        }

        state.spend_active("wood", balance::COAL_PIT_WOOD_PER_BATCH as u32);
        let output = balance::COAL_PIT_COAL_PER_BATCH * state.income_multiplier();
        let added = state.add_active("coal", output.round().max(0.0) as u32);
        state.stats.coal_produced += added as f64;
        state.coal_seen = true;
    }
}

/// Гасит огонь кузницы и при включённом «Авто» подбрасывает уголь.
fn tick_forge_fire(state: &mut GameState, dt: f64) {
    if state.forge_level == 0 {
        state.current_fire = 0;
        state.forge_auto_timer = 0.0;
        return;
    }

    // Автоподкидка: один уголь раз в две секунды, пока огонь не полон.
    if state.forge_auto_feed {
        state.forge_auto_timer += dt.max(0.0);
        while state.forge_auto_timer + 1e-9 >= balance::FORGE_AUTO_FEED_SECONDS {
            state.forge_auto_timer -= balance::FORGE_AUTO_FEED_SECONDS;
            if state.current_fire < state.fire_max() && state.active_amount("coal") >= 1 {
                state.spend_active("coal", 1);
                state.current_fire =
                    (state.current_fire + state.fire_per_coal()).min(state.fire_max());
            }
        }
    } else {
        state.forge_auto_timer = 0.0;
    }

    state.current_fire = (state.current_fire as f64 - dt.max(0.0)).max(0.0) as u32;
}

/// Строит или улучшает свой дом.
pub fn build_house(state: &mut GameState) -> BuildResult {
    if !state.stone_unlocked() {
        return BuildResult::Locked;
    }

    let cost = balance::house_cost(state.house_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.house_level += 1;
    if state.house_level == 1 {
        push_offer(state, OFFER_COAL_PIT);
    }
    BuildResult::Built(state.house_level)
}

/// Строит или улучшает склад.
pub fn build_storage(state: &mut GameState) -> BuildResult {
    if !state.stone_unlocked() {
        return BuildResult::Locked;
    }

    let cost = balance::storage_cost(state.storage_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.storage_level += 1;
    BuildResult::Built(state.storage_level)
}

/// Строит или улучшает «Уют»: повышает максимум энергии.
pub fn build_comfort(state: &mut GameState) -> BuildResult {
    let cost = balance::comfort_cost(state.comfort_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.comfort_level += 1;
    BuildResult::Built(state.comfort_level)
}

/// Строит или улучшает дом для работников.
pub fn build_worker_house(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::worker_house_cost(state.worker_house_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.worker_house_level += 1;
    BuildResult::Built(state.worker_house_level)
}

/// Строит или улучшает дом для изучения (легаси, UI удалён).
#[allow(dead_code)]
pub fn build_research_house(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::research_house_cost(state.research_house_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.research_house_level += 1;
    BuildResult::Built(state.research_house_level)
}

/// Строит или улучшает лесопилку.
pub fn build_sawmill(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::sawmill_cost(state.sawmill_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.sawmill_level += 1;
    BuildResult::Built(state.sawmill_level)
}

/// Строит или улучшает шахту. Открывается вместе с лесопилкой.
pub fn build_mine(state: &mut GameState) -> BuildResult {
    if state.sawmill_level == 0 {
        return BuildResult::Locked;
    }

    let cost = balance::mine_cost(state.mine_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.mine_level += 1;
    if state.mine_level == 3 {
        events::schedule_thematic(state, "mine_foreman", 5.0 * 60.0, 10.0 * 60.0);
    }
    BuildResult::Built(state.mine_level)
}

/// Оплата открытия угольной ямы (письмо). Уровень остаётся 0.
pub(crate) fn unlock_coal_pit(state: &mut GameState) -> BuildResult {
    if state.house_level == 0 || state.coal_pit_unlocked {
        return BuildResult::Locked;
    }
    let cost = balance::coal_pit_cost(0);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.coal_pit_unlocked = true;
    state.coal_seen = true;
    BuildResult::Built(0)
}

/// Строит или улучшает угольную яму. Доступна после оплаты открытия.
pub fn build_coal_pit(state: &mut GameState) -> BuildResult {
    if !state.coal_pit_unlocked {
        return BuildResult::Locked;
    }

    let cost = balance::coal_pit_cost(state.coal_pit_level + 1);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.coal_pit_level += 1;
    state.coal_pit_researched = true;
    state.coal_seen = true;
    if state.coal_pit_level == 1 {
        push_offer(state, OFFER_FORGE);
    }
    BuildResult::Built(state.coal_pit_level)
}

/// Оплата открытия кузницы (письмо). Уровень остаётся 0.
pub(crate) fn unlock_forge(state: &mut GameState) -> BuildResult {
    if state.coal_pit_level == 0 || state.forge_unlocked {
        return BuildResult::Locked;
    }
    let cost = balance::forge_cost(0);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.forge_unlocked = true;
    BuildResult::Built(0)
}

/// Строит или улучшает кузницу. Доступна после оплаты открытия.
pub fn build_forge(state: &mut GameState) -> BuildResult {
    if !state.forge_unlocked {
        return BuildResult::Locked;
    }

    let cost = balance::forge_cost(state.forge_level + 1);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.forge_level += 1;
    state.forge_researched = true;
    // Action-based триггер: на 3-м уровне кузницы через 5–10 минут
    // заходит странник. Событие — просто текст, ничего не даёт.
    if state.forge_level == 3 {
        events::schedule_thematic(state, "forge_stranger", 5.0 * 60.0, 10.0 * 60.0);
    }
    BuildResult::Built(state.forge_level)
}

/// Оплата открытия банка (письмо). Уровень остаётся 0.
pub(crate) fn unlock_bank(state: &mut GameState) -> BuildResult {
    if state.bank_unlocked {
        return BuildResult::Locked;
    }
    let cost = balance::bank_cost(0);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.bank_unlocked = true;
    BuildResult::Built(0)
}

/// Строит или улучшает банк. Доступен после оплаты открытия.
pub fn build_bank(state: &mut GameState) -> BuildResult {
    if !state.bank_unlocked {
        return BuildResult::Locked;
    }

    let cost = balance::bank_cost(state.bank_level + 1);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.bank_level += 1;
    state.trader_researched = true;
    if state.bank_level == 1 {
        events::schedule_thematic(state, "bank_visitor", 5.0 * 60.0, 10.0 * 60.0);
    }
    BuildResult::Built(state.bank_level)
}

/// Строит или улучшает амбар. Открывается на 8 уровне склада.
pub fn build_barn(state: &mut GameState) -> BuildResult {
    if !state.barn_unlocked() {
        return BuildResult::Locked;
    }

    let cost = balance::barn_cost(state.barn_level);
    if !spend_cost(state, cost) {
        return BuildResult::NotEnough(cost);
    }
    state.barn_level += 1;
    BuildResult::Built(state.barn_level)
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
    if state.sawmill_level == 0 {
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
    if state.house_level == 0 {
        return;
    }
    state.coal_pit_workers = adjust_count(
        state.coal_pit_workers,
        delta,
        state.workers_free(),
        state.coal_pit_slots(),
    );
}

/// Меняет число исследователей на выбранном направлении (легаси, UI удалён).
#[allow(dead_code)]
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
    if state.forge_level == 0 || state.active_amount("coal") < 1 {
        return false;
    }
    let max = state.fire_max();
    if state.current_fire as f64 + 1e-9 >= max as f64 {
        return false;
    }

    state.spend_active("coal", 1);
    state.current_fire = (state.current_fire + state.fire_per_coal()).min(max);
    true
}

/// Включает или выключает автоподкидку угля.
pub fn toggle_forge_auto_feed(state: &mut GameState) {
    state.forge_auto_feed = !state.forge_auto_feed;
}

/// Огонь, который реально потратит рецепт: с учётом скидки наручей.
pub fn fire_cost(state: &GameState, recipe: &items::Recipe) -> u32 {
    if recipe.fire_cost == 0 {
        return 0;
    }
    let discount = state.fire_discount.clamp(0.0, 0.9) as f64;
    (recipe.fire_cost as f64 * (1.0 - discount)).ceil() as u32
}

/// Сколько раз рецепт можно повторить подряд.
///
/// Ограничение — ингредиенты, место в сумке (одна ячейка на предмет, не более
/// [`items::ITEM_STACK_LIMIT`] штук) и огонь кузницы.
pub fn affordable_crafts(state: &GameState, recipe: &items::Recipe) -> u32 {
    // Проверяем уровень кузнеца. Если не хватает — доступно 0 крафтов.
    if state.smith_level() < items::recipe_min_smith_level(recipe.id) {
        return 0;
    }

    if recipe.fire_cost > 0 && state.forge_level == 0 {
        return 0;
    }
    if recipe.id == items::STEAM_PROTOTYPE_RECIPE {
        return u32::from(can_build_steam_prototype(state, recipe));
    }
    let mut limit = u32::MAX;
    // Огонь уже входит в ингредиенты рецепта и берётся из `current_fire`.
    for ingredient in &recipe.ingredients {
        let available = state.ingredient_amount(ingredient.item_id);
        limit = limit.min(available / ingredient.quantity.max(1));
    }
    // Каждый вид предмета — одна ячейка; второй вид требует свободной ячейки,
    // а количество в ячейке ограничено лимитом стака. Свободные ячейки
    // считаем с учётом того, что ингредиенты при списании освободят свои:
    // иначе игрок с забитым ингредиентами рюкзаком не смог бы крафтить.
    let free_slots = state.inventory_free_slots_with_freed(recipe);
    let existing = state.item_count(recipe.result_id);
    let room = if existing > 0 {
        state.item_capacity_left(recipe.result_id)
    } else {
        if free_slots == 0 {
            0
        } else {
            items::ITEM_STACK_LIMIT
        }
    };
    limit = limit.min(room / recipe.result_quantity.max(1));
    limit
}

/// Хватает ли всего для сборки прототипа паровой машины.
///
/// Место в сумке не проверяется: прототип не кладут в рюкзак.
fn can_build_steam_prototype(state: &GameState, recipe: &items::Recipe) -> bool {
    if !state.steam_blueprint_studied || state.steam_prototype_built {
        return false;
    }
    if fire_cost(state, recipe) > state.current_fire {
        return false;
    }
    recipe
        .ingredients
        .iter()
        .filter(|ingredient| ingredient.item_id != "fire")
        .all(|ingredient| state.ingredient_amount(ingredient.item_id) >= ingredient.quantity)
}

/// Сборка прототипа паровой машины: ресурсы и огонь списываются, а сам
/// прототип в сумку не попадает — вместо этого включается событие.
fn craft_steam_prototype(
    state: &mut GameState,
    recipe: &items::Recipe,
    times: u32,
) -> Result<u32, CraftError> {
    if !state.steam_blueprint_studied {
        return Err(CraftError::BlueprintLocked);
    }
    if state.steam_prototype_built {
        return Err(CraftError::AlreadyBuilt);
    }
    if times == 0 {
        return Ok(0);
    }
    if fire_cost(state, recipe) > state.current_fire {
        return Err(CraftError::NoFire);
    }
    for ingredient in &recipe.ingredients {
        if ingredient.item_id != "fire"
            && state.ingredient_amount(ingredient.item_id) < ingredient.quantity
        {
            return Err(CraftError::NotEnough);
        }
    }

    let fire = fire_cost(state, recipe);
    for ingredient in &recipe.ingredients {
        if ingredient.item_id == "fire" {
            continue;
        }
        if !state.spend_ingredient(ingredient.item_id, ingredient.quantity) {
            return Err(CraftError::NotEnough);
        }
    }
    if !state.spend_ingredient("fire", fire) {
        return Err(CraftError::NoFire);
    }

    state.steam_prototype_built = true;
    state.steam_prototype_event = true;

    // Даем финальный опыт (базовый опыт прототипа × множитель X).
    state.add_smith_xp(balance::SMITH_XP_STEAM_PROTOTYPE * state.income_multiplier());

    Ok(1)
}

/// Создаёт предмет по рецепту один раз.
#[cfg_attr(not(test), allow(dead_code))]
pub fn craft(state: &mut GameState, recipe_id: &str) -> Result<(), CraftError> {
    craft_times(state, recipe_id, 1).map(|_| ())
}

/// Создаёт предмет по рецепту `times` раз подряд.
///
/// Если на все повторы не хватает, не делает ничего.
pub fn craft_times(state: &mut GameState, recipe_id: &str, times: u32) -> Result<u32, CraftError> {
    let recipe = items::recipe(recipe_id).ok_or(CraftError::UnknownRecipe)?;

    // Блокируем крафт, если уровень недостаточен.
    let min_level = items::recipe_min_smith_level(recipe.id);
    if state.smith_level() < min_level {
        return Err(CraftError::Locked(min_level));
    }

    if recipe.id == items::STEAM_PROTOTYPE_RECIPE {
        return craft_steam_prototype(state, recipe, times);
    }
    if times == 0 {
        return Ok(0);
    }
    if recipe.fire_cost > 0 && state.forge_level == 0 {
        return Err(CraftError::NoFire);
    }

    // Сколько повторов реально хватает по всем ограничениям.
    let available = affordable_crafts(state, recipe);
    let times = times.min(available);
    if times == 0 {
        if fire_cost(state, recipe) > state.current_fire {
            return Err(CraftError::NoFire);
        }
        for ingredient in &recipe.ingredients {
            if state.ingredient_amount(ingredient.item_id) < ingredient.quantity {
                return Err(CraftError::NotEnough);
            }
        }
        return Err(CraftError::NoSpace);
    }

    // Считаем опыт за 1 крафт (с учётом X и бонуса экипировки).
    let base_price = items::item(recipe.result_id)
        .map(|item| item.base_price)
        .unwrap_or(10);
    let income = state.income_multiplier();
    let xp_per_craft = balance::smith_xp_per_craft(recipe.id, base_price, recipe.result_quantity)
        * income
        * state.smith_xp_bonus as f64;

    // Бросок «сэкономить ингредиент»: если сработал — предмет не тратится.
    let discount = state.ingredient_discount.clamp(0.0, 0.9) as f64;
    let mut seed = state.market_seed;

    for _ in 0..times {
        for ingredient in &recipe.ingredients {
            let saved = discount > 0.0
                && ingredient.item_id != "fire"
                && super::trader::random_below(&mut seed, 100) < (discount * 100.0) as u64;
            if !saved && !state.spend_ingredient(ingredient.item_id, ingredient.quantity) {
                state.market_seed = seed;
                return Err(CraftError::NotEnough);
            }
        }
        state.market_seed = seed;
        if !state.add_item(recipe.result_id, recipe.result_quantity) {
            return Err(CraftError::NoSpace);
        }
        state.add_smith_xp(xp_per_craft);
    }
    Ok(times)
}

/// Надевает предмет из сумки.
pub fn equip_item(state: &mut GameState, item_id: &str) -> Result<(), EquipError> {
    state.equip_item(item_id)
}

/// Снимает предмет из слота.
pub fn unequip_slot(state: &mut GameState, slot: items::EquipSlot) {
    state.unequip_slot(slot);
}

/// Кладёт предмет из гардероба в рюкзак (если есть место).
/// Возвращает `true`, если удалось.
pub fn take_from_wardrobe(state: &mut GameState, item_id: &str) -> bool {
    if state.wardrobe_count(item_id) == 0 {
        return false;
    }
    if !state.can_add_item(item_id, 1) {
        return false;
    }
    if state.remove_from_wardrobe(item_id, 1) {
        state.add_item(item_id, 1);
        true
    } else {
        false
    }
}

/// Чит для демо: добавляет по горсти базовых материалов.
///
/// Возвращает число добавленных стопок.
#[allow(dead_code)]
pub fn add_demo_resources(state: &mut GameState) -> u32 {
    let mut added = 0;
    for (key, quantity) in [
        ("wood", 50),
        ("stone", 50),
        ("leather", 5),
        ("silver", 1),
        ("quartz", 1),
        ("clay", 20),
    ] {
        let ok = if crate::game::state::is_active_resource(key) {
            state.add_active(key, quantity) > 0
        } else {
            state.add_item(key, quantity)
        };
        if ok {
            added += 1;
        }
    }
    added
}

pub(crate) fn next_random(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed
}

/// Ошибка разбора.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismantleError {
    /// Предмет не найден.
    UnknownItem,
    /// Этого предмета нет в рюкзаке.
    NotOwned,
    /// Такой предмет нельзя разобрать (не экипировка, карта, чертёж).
    NotDismantlable,
    /// Нет места в рюкзаке для ингредиентов.
    NoSpace,
}

impl DismantleError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownItem => i18n::t("err.unknown_item"),
            Self::NotOwned => i18n::t("err.not_owned"),
            Self::NotDismantlable => i18n::t("err.not_dismantlable"),
            Self::NoSpace => i18n::t("err.dismantle_no_space"),
        }
    }
}

/// Разбирает предмет из рюкзака. Возвращает список полученных ингредиентов.
///
/// Возвращается 50% от рецепта (с округлением вниз, но не меньше 1 для
/// тех ингредиентов, которые в рецепте были). Огонь не возвращается.
pub fn dismantle_item(
    state: &mut GameState,
    item_id: &str,
) -> Result<Vec<(String, u32)>, DismantleError> {
    let _item = items::item(item_id).ok_or(DismantleError::UnknownItem)?;
    if !items::is_dismantlable(item_id) {
        return Err(DismantleError::NotDismantlable);
    }
    if state.item_count(item_id) == 0 {
        return Err(DismantleError::NotOwned);
    }

    // Ищем рецепт, где этот предмет — результат.
    let recipe = items::recipes()
        .iter()
        .find(|r| r.result_id == item_id)
        .ok_or(DismantleError::NotDismantlable)?;

    // Считаем, что вернём.
    let mut refund: Vec<(String, u32)> = Vec::new();
    for ingredient in &recipe.ingredients {
        if ingredient.item_id == "fire" {
            continue;
        }
        // 50% с округлением вниз, но не меньше 1, если ингредиент был.
        let amount =
            ((ingredient.quantity as f64 * balance::DISMANTLE_REFUND).floor() as u32).max(1);
        refund.push((ingredient.item_id.to_string(), amount));
    }

    // Проверяем, что всё поместится. Ингредиенты могут освободить ячейку
    // самого предмета, поэтому считаем «с учётом освобождения».
    let mut temp_state = state.clone();
    temp_state.remove_item(item_id, 1);
    for (id, qty) in &refund {
        if !temp_state.can_add_item(id, *qty) {
            return Err(DismantleError::NoSpace);
        }
        temp_state.add_item(id, *qty);
    }

    // Всё помещается — разбираем.
    state.remove_item(item_id, 1);
    for (id, qty) in &refund {
        state.add_item(id, *qty);
    }

    Ok(refund)
}

/// Начинает новый цикл. Возвращает новый номер цикла.
///
/// Жёсткий гейт: за цикл нужно добыть дерево и камень до порога, пройти
/// стены R = 5 (шахта) и R = 10 (камень сверх порога).
pub fn prestige(state: &mut GameState) -> PrestigeResult {
    if state.cycle_gathered() + 1e-9 < state.cycle_threshold() {
        return PrestigeResult::NotEnough;
    }
    if !state.cycle_wall_mine_met() {
        return PrestigeResult::WallMine;
    }
    if !state.cycle_wall_stone_met() {
        return PrestigeResult::WallStone;
    }

    state.reset_for_cycle();
    // Сюжетное письмо в новом цикле: через 30–40 минут.
    events::schedule_thematic(state, "rebirth_letter", 30.0 * 60.0, 40.0 * 60.0);
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
    // Ускорение времени ускоряет и события: прокручиваем их по секундам,
    // иначе таймер фоновых встреч стоял бы на месте.
    let seconds = balance::TIME_WARP_SECONDS.round().max(0.0) as u32;
    for _ in 0..seconds {
        crate::game::events::tick(state, 1.0);
    }
    true
}

/// Тратит единицу времени на изучение выбранного направления (легаси, UI удалён).
#[allow(dead_code)]
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
fn unlock_coal_pit_for_test(state: &mut GameState) -> BuildResult {
    unlock_coal_pit(state)
}

#[cfg(test)]
fn unlock_forge_for_test(state: &mut GameState) -> BuildResult {
    unlock_forge(state)
}

#[cfg(test)]
fn unlock_bank_for_test(state: &mut GameState) -> BuildResult {
    unlock_bank(state)
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
        assert_eq!(state.active_amount("wood"), 1);
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
        state.add_active("wood", 15);
        state.add_active("stone", 8);
        state.stone_seen = true;

        assert_eq!(build_house(&mut state), BuildResult::Built(1));
        assert_eq!(state.house_level, 1);
        assert_eq!(state.energy_regen(), 0.5);
        assert_eq!(state.rest_energy(), 1.5);
        assert_eq!(state.active_amount("wood"), 0);
        assert_eq!(state.active_amount("stone"), 0);
    }

    #[test]
    fn dev_speed_adds_resources() {
        let mut state = GameState::default();
        state.dev.wood_per_sec = 5.0;
        tick(&mut state, 2.0);
        assert_eq!(state.active_amount("wood"), 10);
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

        assert_eq!(state.active_amount("wood"), 5);
        assert!(
            (state.sawmill_wood_per_sec() - balance::SAWMILL_WOOD_PER_WORKER_PER_SEC).abs() < 1e-9
        );
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
            ..GameState::default()
        };
        state.add_active("wood", 10);

        tick(&mut state, balance::COAL_PIT_BATCH_SECONDS);

        assert_eq!(state.active_amount("wood"), 9);
        assert_eq!(state.active_amount("coal"), 1);
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

        assert_eq!(state.active_amount("coal"), 0);
        assert_eq!(state.coal_pit_progress, 0.0);
    }

    #[test]
    fn storage_raises_cap() {
        let mut state = rested_state();
        state.add_active("wood", 30);
        state.add_active("stone", 15);
        state.stone_seen = true;

        let before = state.cap(ResourceKind::Wood);
        assert_eq!(build_storage(&mut state), BuildResult::Built(1));
        assert_eq!(
            state.cap(ResourceKind::Wood),
            before + balance::STORAGE_CAP_PER_LEVEL
        );
    }

    #[test]
    fn unlock_prices_match_the_new_design() {
        // Кузница: 200 дерева, 100 камня, 300 энергии, 500 золота.
        let forge = balance::forge_cost(0);
        assert_eq!(forge.wood, 200.0);
        assert_eq!(forge.stone, 100.0);
        assert_eq!(forge.energy, 300.0);
        assert_eq!(forge.gold, 500.0);
        assert_eq!(forge.iron_ingots, 0);

        // Угольная яма: 300 золота.
        let coal = balance::coal_pit_cost(0);
        assert_eq!(coal.gold, 300.0);
        assert_eq!(coal.wood, 0.0);
        assert_eq!(coal.iron_ingots, 0);

        // Банк: 25 железных слитков и 500 золота.
        let bank = balance::bank_cost(0);
        assert_eq!(bank.gold, 500.0);
        assert_eq!(bank.iron_ingots, 25);
        assert_eq!(bank.wood, 0.0);
        assert_eq!(bank.stone, 0.0);

        // Открытие всех трёх построек укладывается в стартовый лимит золота без банка.
        for (name, cost) in [
            ("угольная яма", balance::coal_pit_cost(0)),
            ("кузница", balance::forge_cost(0)),
            ("банк", balance::bank_cost(0)),
        ] {
            assert!(cost.gold <= balance::BASE_GOLD_CAP, "{name}");
        }
    }

    #[test]
    fn first_level_prices_match_the_new_design() {
        // Постройка 0→1: кузница — 300 дерева, 250 камня.
        let forge = balance::forge_cost(1);
        assert_eq!(forge.wood, 300.0);
        assert_eq!(forge.stone, 250.0);
        assert_eq!(forge.gold, 0.0);
        assert_eq!(forge.energy, 0.0);

        // Угольная яма — 180 дерева, 100 камня.
        let coal = balance::coal_pit_cost(1);
        assert_eq!(coal.wood, 180.0);
        assert_eq!(coal.stone, 100.0);
        assert_eq!(coal.gold, 0.0);

        // Банк — 300 дерева, 200 камня, 300 золота.
        let bank = balance::bank_cost(1);
        assert_eq!(bank.wood, 300.0);
        assert_eq!(bank.stone, 200.0);
        assert_eq!(bank.gold, 300.0);
        assert_eq!(bank.iron_ingots, 0);
    }

    #[test]
    fn upgrades_after_first_level_use_wood_and_stone() {
        for level in 2..=5 {
            for cost in [
                balance::coal_pit_cost(level),
                balance::forge_cost(level),
                balance::bank_cost(level),
            ] {
                assert_eq!(cost.gold, 0.0, "уровень {level}");
                assert_eq!(cost.energy, 0.0, "уровень {level}");
                assert_eq!(cost.boards, 0, "уровень {level}");
                assert_eq!(cost.iron_ingots, 0, "уровень {level}");
                assert!(cost.wood > 0.0 && cost.stone > 0.0, "уровень {level}");
            }
        }
    }

    #[test]
    fn storage_upgrade_always_fits_into_storage() {
        for level in 0..=13 {
            let cost = balance::storage_cost(level);
            let capacity = GameState {
                storage_level: level,
                ..GameState::default()
            }
            .cap(ResourceKind::Wood);
            assert!(
                cost.wood <= capacity && cost.stone <= capacity,
                "склад {level}: нужно {} / {}, вмещается {capacity}",
                cost.wood,
                cost.stone
            );
        }
    }

    #[test]
    fn barn_price_uses_wood_boards_and_gold() {
        let cost = balance::barn_cost(0);
        assert_eq!(cost.wood, balance::BARN_BASE_COST_WOOD);
        assert_eq!(cost.boards, balance::BARN_BASE_COST_BOARDS);
        assert_eq!(cost.stone, 0.0);
        assert!(cost.gold > 0.0);
        for level in 0..=20 {
            let cost = balance::barn_cost(level);
            assert!(
                cost.boards <= crate::game::items::ITEM_STACK_LIMIT,
                "амбар {level}"
            );
        }
    }

    #[test]
    fn forge_upgrades_fit_into_storage_you_can_build() {
        // Игрок жаловался: при росте ×1.8 кузница 8-го уровня стоила 5102
        // дерева, а склад 10-го давал 3600 — улучшение было не купить.
        // Рост ×1.55: кузница 10 стоит 4997 дерева и 3331 камня, а склад
        // 10-го уровня даёт 5100, то есть улучшение всегда помещается.
        let cap_at_storage_10 = balance::BASE_STORAGE_CAP + 10.0 * balance::STORAGE_CAP_PER_LEVEL;
        for level in 2..=10 {
            let cost = balance::forge_cost(level);
            assert!(
                cost.wood <= cap_at_storage_10 && cost.stone <= cap_at_storage_10,
                "кузница {level}: {} дерева / {} камня не влезают в склад ({}), \
                 нужен амбар или другой рост цены",
                cost.wood,
                cost.stone,
                cap_at_storage_10
            );
        }
    }

    #[test]
    fn save_roundtrip_keeps_state() {
        let mut state = rested_state();
        state.add_active("wood", 3);
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
    fn mine_unlocks_together_with_sawmill() {
        let mut state = GameState::default();
        // Без лесопилки шахта закрыта даже при построенной угольной яме.
        state.coal_pit_level = 1;
        assert_eq!(build_mine(&mut state), BuildResult::Locked);

        // С лесопилкой шахта строится и без угольной ямы.
        state.coal_pit_level = 0;
        state.sawmill_level = 1;
        state.add_active("wood", 100);
        state.add_active("stone", 100);
        state.gold = 1_000.0;
        assert_eq!(build_mine(&mut state), BuildResult::Built(1));
    }

    #[test]
    fn mine_workers_produce_stone() {
        let mut state = GameState {
            coal_pit_researched: true,
            mine_level: 1,
            mine_workers: 1,
            ..GameState::default()
        };

        tick(&mut state, 10.0);

        // Шахта даёт камень, а не железо.
        assert!((state.active_amount("stone") as f64 - 5.0).abs() < 1e-9);
        assert!((state.stats.stone_gathered - 5.0).abs() < 1e-9);
        assert_eq!(state.active_amount("iron"), 0);
    }

    #[test]
    fn forge_fire_burns_and_auto_feeds() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        state.add_active("coal", 1);

        assert!(add_coal_to_fire(&mut state));
        assert_eq!(state.current_fire, balance::FIRE_SECONDS_PER_COAL);

        tick(&mut state, 5.0);
        assert_eq!(state.current_fire, balance::FIRE_SECONDS_PER_COAL - 5);

        state.current_fire = 0;
        state.forge_auto_feed = true;
        state.add_active("coal", 1);
        // «Авто» подбрасывает уголь раз в две секунды, а не мгновенно.
        tick(&mut state, 1.0);
        assert_eq!(state.active_amount("coal"), 1);
        assert_eq!(state.current_fire, 0);

        tick(&mut state, 1.0);
        assert_eq!(state.active_amount("coal"), 0);
        assert_eq!(state.current_fire, balance::FIRE_SECONDS_PER_COAL - 1);
    }

    #[test]
    fn craft_consumes_ingredients_and_adds_result() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        state.add_item("iron", 5);
        state.add_active("coal", 1);
        state.current_fire = balance::FIRE_SECONDS_PER_COAL;

        assert_eq!(craft(&mut state, "iron_ingot"), Ok(()));
        let spent_fire = balance::FIRE_SECONDS_PER_COAL - state.current_fire;
        assert_eq!(state.item_count("iron"), 3);
        // Уголь в рецепте слитка больше не нужен: только руда и огонь.
        assert_eq!(state.active_amount("coal"), 1);
        assert_eq!(spent_fire, 15);
        assert_eq!(state.item_count("iron_ingot"), 1);
    }

    #[test]
    fn stone_block_takes_fifty_stone() {
        let mut state = GameState::default();
        state.inventory.clear();
        state.active_resources.clear();
        state.add_active("stone", 49);
        assert_eq!(craft(&mut state, "stone_block"), Err(CraftError::NotEnough));

        state.add_active("stone", 1);
        assert_eq!(craft(&mut state, "stone_block"), Ok(()));
        assert_eq!(state.active_amount("stone"), 0);
        assert_eq!(state.item_count("stone_block"), 1);
    }

    #[test]
    fn craft_rejects_missing_ingredients() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        state.inventory.clear();
        state.active_resources.clear();
        state.current_fire = balance::FIRE_SECONDS_PER_COAL;

        assert_eq!(craft(&mut state, "iron_ingot"), Err(CraftError::NotEnough));
    }

    #[test]
    fn equip_changes_trade_bonus_and_respects_slot() {
        let mut state = GameState::default();
        state.add_item("trader_ring", 1);

        assert_eq!(equip_item(&mut state, "trader_ring"), Ok(()));
        assert_eq!(
            state.equipped(items::EquipSlot::Accessory1),
            Some("trader_ring")
        );
        // 10% к цене продажи + 5% к золоту.
        assert!((state.trade_bonus - 0.15).abs() < 1e-6);

        // Второй аксессуар уходит во второй слот, а не упирается в занятый.
        state.add_item("miner_amulet", 1);
        assert_eq!(equip_item(&mut state, "miner_amulet"), Ok(()));
        assert_eq!(
            state.equipped(items::EquipSlot::Accessory2),
            Some("miner_amulet")
        );
        assert!((state.gather_bonus - 0.15).abs() < 1e-6);

        // Третий аксессуар уже некуда надеть.
        state.add_item("silver_ring", 1);
        assert_eq!(
            equip_item(&mut state, "silver_ring"),
            Err(EquipError::SlotOccupied)
        );

        // Сняли первый — место снова свободно.
        unequip_slot(&mut state, items::EquipSlot::Accessory1);
        assert_eq!(
            state.equipped(items::EquipSlot::Accessory2),
            Some("miner_amulet")
        );
        assert!((state.trade_bonus).abs() < 1e-6);
        assert_eq!(equip_item(&mut state, "silver_ring"), Ok(()));
        assert_eq!(
            state.equipped(items::EquipSlot::Accessory1),
            Some("silver_ring")
        );
        assert!((state.trade_bonus - 0.03).abs() < 1e-6);
    }

    #[test]
    fn craft_times_is_limited_by_ingredients_and_fire() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        state.add_item("iron", 5);
        state.add_active("coal", 1);
        state.current_fire = 100;
        let recipe = items::recipe("iron_ingot").unwrap();

        // Слиток — 2 руды и 15 огня, уголь не нужен: из 5 руды и 100 огня
        // выходит два слитка, третий упирается в руду.
        assert_eq!(affordable_crafts(&state, recipe), 2);
        assert_eq!(craft_times(&mut state, "iron_ingot", 10), Ok(2));
        assert_eq!(state.item_count("iron_ingot"), 2);
    }

    #[test]
    fn tools_are_craftable_with_ingredients_and_fire() {
        for (recipe_id, ingredients) in [
            ("sickle", [("iron_ingot", 3), ("board", 1)]),
            ("scythe", [("iron_ingot", 3), ("board", 2)]),
            ("hammer", [("board", 2), ("iron_ingot", 1)]),
        ] {
            let mut state = GameState {
                forge_level: 1,
                current_fire: 100,
                smith_level: balance::SMITH_MAX_LEVEL,
                ..GameState::default()
            };
            for (item_id, quantity) in ingredients {
                state.add_item(item_id, quantity);
            }
            assert_eq!(craft_times(&mut state, recipe_id, 1), Ok(1));
            assert_eq!(state.item_count(recipe_id), 1, "{recipe_id} не создан");
        }
    }

    #[test]
    fn smith_level_blocks_recipes_and_crafting_grants_xp() {
        // «Роба» требует 6-го уровня кузнеца.
        let mut state = GameState::default();
        assert_eq!(craft(&mut state, "robe"), Err(CraftError::Locked(6)));
        assert_eq!(state.item_count("robe"), 0);

        let mut state = GameState {
            smith_level: 6,
            ..GameState::default()
        };
        state.add_item("cloth", 5);
        let before = state.smith_xp;
        assert_eq!(craft(&mut state, "robe"), Ok(()));
        assert!(
            state.smith_xp > before,
            "крафт должен начислять опыт кузнеца"
        );
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
    fn crafting_into_an_existing_stack_needs_no_free_slot() {
        let mut state = GameState::default();
        // Заняты все ячейки: свечи лежат в своей, фибра — в своей.
        for item_id in [
            "wretched_hat",
            "leather_hat",
            "winter_gloves",
            "poor_boots",
            "jug",
            "whetstone",
            "brick",
            "cloth",
        ] {
            state.add_item(item_id, 1);
        }
        state.add_item("candle", 6);
        state.add_item("fiber", 1);
        state.add_active("coal", 10);
        assert_eq!(state.inventory_free_slots(), 0, "рюкзак должен быть полон");

        let recipe = items::recipe("candle").expect("рецепт свечи");
        assert_eq!(
            affordable_crafts(&state, recipe),
            1,
            "свеча уже в сумке — новая ячейка не нужна"
        );
        assert_eq!(craft_times(&mut state, "candle", 1), Ok(1));
        assert_eq!(state.item_count("candle"), 7);
    }

    #[test]
    fn comfort_raises_energy_cap() {
        let mut state = GameState {
            ..GameState::default()
        };
        state.add_active("wood", 1000);
        state.add_active("stone", 1000);
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
            cycles: 4,
            ..GameState::default()
        };
        state.cycle_wood = state.cycle_threshold();
        state.time = 5.0;

        assert_eq!(prestige(&mut state), PrestigeResult::Done(5));
        assert_eq!(state.cycles, 5);
        assert_eq!(state.cycle_wood, 0.0);
        assert_eq!(state.cycle_stone, 0.0);
        assert_eq!(state.time, 5.0);
        assert!((state.income_multiplier() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn prestige_resets_smith_level_and_applies_bonus() {
        let mut state = GameState {
            cycles: 4,
            smith_level: 15,
            smith_xp: 100.0,
            ..GameState::default()
        };
        state.cycle_wood = state.cycle_threshold();

        assert_eq!(prestige(&mut state), PrestigeResult::Done(5));
        // Бонус за 4 цикла — 20 опыта: 10 закрывает 1 → 2, остаток 10 из 40.
        assert_eq!(state.smith_level, 2);
        assert!((state.smith_xp - 10.0).abs() < 1e-9);
    }

    #[test]
    fn income_multiplier_is_five_on_tenth_cycle() {
        let state = GameState {
            cycles: 10,
            ..GameState::default()
        };
        assert!((state.income_multiplier() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn tick_applies_income_multiplier_once() {
        let mut state = GameState {
            cycles: 10,
            worker_house_level: 1,
            sawmill_level: 1,
            sawmill_workers: 2,
            mine_level: 1,
            mine_workers: 1,
            ..GameState::default()
        };

        // 2 работника × 0.5/с × X(10)=5 → 5 дерева в секунду, за 2 с — 10.
        // Камень: 1 работник × 0.5/с × 5 → 5 за 2 с.
        tick(&mut state, 2.0);

        assert!((state.active_amount("wood") as f64 - 10.0).abs() < 1e-9);
        assert!((state.active_amount("stone") as f64 - 5.0).abs() < 1e-9);
        assert!((state.cycle_wood - 10.0).abs() < 1e-9);
        assert!((state.cycle_stone - 5.0).abs() < 1e-9);
    }

    #[test]
    fn gather_scales_with_cycles() {
        let mut state = GameState {
            cycles: 10,
            ..rested_state()
        };

        assert_eq!(
            gather(&mut state, ResourceKind::Wood),
            GatherResult::Gathered(5.0)
        );
        assert_eq!(state.active_amount("wood"), 5);
    }

    #[test]
    fn rest_scales_with_cycles_but_energy_cap_does_not() {
        let mut state = GameState {
            cycles: 10,
            ..GameState::default()
        };

        // «Отдых» × X: 1.0 × 5 = 5.
        assert!((state.rest_energy() - 5.0).abs() < 1e-9);
        // Максимум энергии без X: 100.
        assert!((state.max_energy() - 100.0).abs() < 1e-9);

        let gained = rest(&mut state);
        assert!((gained - 5.0).abs() < 1e-9);
    }

    #[test]
    fn passive_energy_regen_scales_with_cycles() {
        let state = GameState {
            cycles: 10,
            house_level: 1,
            ..GameState::default()
        };

        // Дом даёт 0.5/с, X на R = 10 равен 5 → 2.5/с.
        assert!((state.energy_regen() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn prestige_needs_cycle_threshold() {
        let mut state = GameState::default();

        assert_eq!(prestige(&mut state), PrestigeResult::NotEnough);
        assert_eq!(state.cycles, 0);

        state.cycle_wood = state.cycle_threshold() - 1.0;
        assert_eq!(prestige(&mut state), PrestigeResult::NotEnough);
    }

    #[test]
    fn prestige_wall_needs_mine_after_fifth_cycle() {
        let mut state = GameState {
            cycles: balance::CYCLE_WALL_LEVEL,
            mine_level: balance::CYCLE_WALL_MINE_LEVEL - 1,
            ..GameState::default()
        };
        state.cycle_wood = state.cycle_threshold();

        assert_eq!(prestige(&mut state), PrestigeResult::WallMine);

        state.mine_level = balance::CYCLE_WALL_MINE_LEVEL;
        assert_eq!(prestige(&mut state), PrestigeResult::Done(6));
    }

    #[test]
    fn prestige_wall_needs_extra_stone_after_tenth_cycle() {
        let mut state = GameState {
            cycles: balance::CYCLE_WALL_STONE_LEVEL,
            mine_level: balance::CYCLE_WALL_MINE_LEVEL,
            ..GameState::default()
        };
        state.cycle_stone = balance::CYCLE_WALL_EXTRA_STONE - 1.0;
        state.cycle_wood = state.cycle_threshold();

        assert_eq!(prestige(&mut state), PrestigeResult::WallStone);

        state.cycle_stone = balance::CYCLE_WALL_EXTRA_STONE;
        assert_eq!(prestige(&mut state), PrestigeResult::Done(11));
    }

    #[test]
    fn cycle_progress_counts_wood_and_stone() {
        let mut state = rested_state();
        gather(&mut state, ResourceKind::Wood);
        gather(&mut state, ResourceKind::Wood);
        gather(&mut state, ResourceKind::Stone);

        assert!((state.cycle_gathered() - 3.0).abs() < 1e-9);
        assert!(state.cycle_progress() > 0.0);
        assert!(state.cycle_progress() < 1.0);
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

    #[test]
    fn study_blueprint_unlocks_the_prototype_recipe() {
        let mut state = GameState::default();
        assert!(!recipe_visible(
            &state,
            items::recipe(items::STEAM_PROTOTYPE_RECIPE).expect("рецепт")
        ));

        assert_eq!(
            study_blueprint(&mut state, "wood"),
            Err(StudyBlueprintError::NotBlueprint)
        );
        assert_eq!(
            study_blueprint(&mut state, items::STEAM_BLUEPRINT_ID),
            Err(StudyBlueprintError::Missing)
        );

        state.add_item(items::STEAM_BLUEPRINT_ID, 2);
        assert_eq!(
            study_blueprint(&mut state, items::STEAM_BLUEPRINT_ID),
            Ok(())
        );
        assert_eq!(state.item_count(items::STEAM_BLUEPRINT_ID), 1);
        assert!(state.steam_blueprint_studied);
        assert!(recipe_visible(
            &state,
            items::recipe(items::STEAM_PROTOTYPE_RECIPE).expect("рецепт")
        ));

        // Повторный чертёж не тратится.
        assert_eq!(
            study_blueprint(&mut state, items::STEAM_BLUEPRINT_ID),
            Err(StudyBlueprintError::AlreadyStudied)
        );
        assert_eq!(state.item_count(items::STEAM_BLUEPRINT_ID), 1);
    }

    #[test]
    fn steam_prototype_is_built_but_not_stored() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        state.add_item("iron_ingot", 100);
        state.add_item("silver", 50);
        state.add_item("gems", 10);
        state.add_item("quartz", 50);
        state.current_fire = balance::STEAM_PROTOTYPE_FIRE;

        // Без изученного чертежа рецепта нет.
        assert_eq!(
            craft(&mut state, items::STEAM_PROTOTYPE_RECIPE),
            Err(CraftError::BlueprintLocked)
        );
        assert_eq!(state.item_count("iron_ingot"), 100);

        state.add_item(items::STEAM_BLUEPRINT_ID, 1);
        assert_eq!(
            study_blueprint(&mut state, items::STEAM_BLUEPRINT_ID),
            Ok(())
        );

        assert_eq!(craft(&mut state, items::STEAM_PROTOTYPE_RECIPE), Ok(()));
        assert!(state.steam_prototype_built);
        assert!(state.steam_prototype_event);
        // Ресурсы и огонь списаны, предмет в сумку не попал.
        assert_eq!(state.item_count("iron_ingot"), 0);
        assert_eq!(state.item_count("silver"), 0);
        assert_eq!(state.item_count("gems"), 0);
        assert_eq!(state.item_count("quartz"), 0);
        assert_eq!(state.current_fire, 0);
        assert_eq!(state.item_count(items::STEAM_PROTOTYPE_ID), 0);
        assert!(!recipe_visible(
            &state,
            items::recipe(items::STEAM_PROTOTYPE_RECIPE).expect("рецепт")
        ));
        assert_eq!(
            craft(&mut state, items::STEAM_PROTOTYPE_RECIPE),
            Err(CraftError::AlreadyBuilt)
        );
    }

    #[test]
    fn steam_prototype_needs_all_ingredients() {
        let mut state = GameState {
            forge_level: 1,
            steam_blueprint_studied: true,
            ..GameState::default()
        };
        state.add_item("iron_ingot", 99);
        state.add_item("silver", 50);
        state.add_item("gems", 10);
        state.add_item("quartz", 50);
        state.current_fire = balance::STEAM_PROTOTYPE_FIRE;
        assert_eq!(
            craft(&mut state, items::STEAM_PROTOTYPE_RECIPE),
            Err(CraftError::NotEnough)
        );

        state.add_item("iron_ingot", 1);
        state.current_fire = balance::STEAM_PROTOTYPE_FIRE - 1;
        assert_eq!(
            craft(&mut state, items::STEAM_PROTOTYPE_RECIPE),
            Err(CraftError::NoFire)
        );
        assert!(!state.steam_prototype_built);
    }

    #[test]
    fn entering_the_steam_era_raises_income_once() {
        let mut state = GameState::default();
        assert!(!enter_steam_era(&mut state));

        state.steam_prototype_built = true;
        let before = state.income_multiplier();
        assert!(enter_steam_era(&mut state));
        assert!(state.steam_era);
        assert!(
            (state.income_multiplier() - (before + balance::STEAM_ERA_INCOME_BONUS)).abs() < 1e-9
        );
        assert!(!enter_steam_era(&mut state));
    }

    #[test]
    fn fractional_gathering_accumulates_over_time() {
        let mut state = GameState {
            worker_house_level: 1,
            sawmill_level: 1,
            coal_pit_level: 1,
            mine_level: 1,
            sawmill_workers: 1,
            mine_workers: 1,
            ..GameState::default()
        };

        // Один работник даёт 0.5/с: за первую секунду целой единицы нет.
        tick(&mut state, 1.0);
        assert_eq!(state.active_amount("wood"), 0);
        assert_eq!(state.active_amount("stone"), 0);

        // За вторую секунду доля догоняет до целого.
        tick(&mut state, 1.0);
        assert_eq!(state.active_amount("wood"), 1);
        assert_eq!(state.active_amount("stone"), 1);

        tick(&mut state, 2.0);
        assert_eq!(state.active_amount("wood"), 2);
        assert_eq!(state.active_amount("stone"), 2);
    }

    #[test]
    fn house_offer_becomes_letter_and_reopens() {
        let mut state = GameState {
            stone_seen: true,
            ..GameState::default()
        };
        state.add_active("wood", 50);
        state.add_active("stone", 50);

        assert_eq!(build_house(&mut state), BuildResult::Built(1));
        assert_eq!(state.pending_offer.as_deref(), Some(OFFER_COAL_PIT));

        // «Позже» убирает блок с экрана и кладёт его письмом в колокольчик.
        defer_pending_offer(&mut state);
        assert!(state.pending_offer.is_none());
        assert_eq!(state.unread_letters(), 1);

        // Клик по письму снова открывает предложение.
        state.open_letter("offer_coal_pit");
        assert_eq!(state.unread_letters(), 0);
        assert_eq!(state.pending_offer.as_deref(), Some(OFFER_COAL_PIT));
    }

    #[test]
    fn bank_offer_appears_at_eight_hundred_gold() {
        let mut state = GameState {
            gold: balance::BANK_OFFER_GOLD - 1.0,
            storage_level: 5,
            ..GameState::default()
        };
        tick(&mut state, 0.1);
        assert!(state.pending_offer.is_none());

        state.gold = balance::BANK_OFFER_GOLD;
        tick(&mut state, 0.1);
        assert_eq!(state.pending_offer.as_deref(), Some(OFFER_BANK));
        assert!(state.bank_seen);

        // Открытие банка требует 25 железных слитков и 500 золота.
        let cost = balance::bank_cost(0);
        state.add_item("iron_ingot", cost.iron_ingots);
        // Золота достаточно (800 >= 500).
        assert_eq!(build_pending_offer(&mut state), BuildResult::Built(0));
        assert!(state.pending_offer.is_none());
        assert!(state.bank_unlocked);
        assert_eq!(state.bank_level, 0);
    }

    #[test]
    fn equipped_item_leaves_inventory_and_returns_on_unequip() {
        let mut state = GameState::default();
        state.add_item("trader_ring", 2);

        assert_eq!(equip_item(&mut state, "trader_ring"), Ok(()));
        assert_eq!(state.item_count("trader_ring"), 1);

        unequip_slot(&mut state, items::EquipSlot::Accessory1);
        assert_eq!(state.item_count("trader_ring"), 2);
    }

    #[test]
    fn barn_unlocks_at_storage_level_eight_and_raises_cap() {
        let mut state = GameState {
            storage_level: balance::BARN_UNLOCK_STORAGE_LEVEL - 1,
            ..GameState::default()
        };
        let cost = balance::barn_cost(0);
        state.add_active("wood", cost.wood as u32);
        state.add_item("board", cost.boards);
        state.gold = cost.gold;

        assert_eq!(build_barn(&mut state), BuildResult::Locked);

        state.storage_level = balance::BARN_UNLOCK_STORAGE_LEVEL;
        let before = state.active_cap("wood");
        assert_eq!(build_barn(&mut state), BuildResult::Built(1));
        assert_eq!(
            state.active_cap("wood"),
            before + balance::BARN_CAP_PER_LEVEL as u32
        );
    }

    #[test]
    fn forge_unlock_does_not_build_first_level() {
        let mut state = GameState {
            coal_pit_level: 1,
            gold: 10_000.0,
            energy: 10_000.0,
            storage_level: 2,
            ..GameState::default()
        };
        state.add_active("wood", 200);
        state.add_active("stone", 100);

        // Открытие: 200/100/300⚡/500💰 → уровень 0.
        assert_eq!(unlock_forge_for_test(&mut state), BuildResult::Built(0));
        assert!(state.forge_unlocked);
        assert_eq!(state.forge_level, 0);
        // Ресурсы списаны.
        assert_eq!(state.active_amount("wood"), 0);
        assert_eq!(state.active_amount("stone"), 0);
        assert!(state.gold < 10_000.0);

        // Постройка 0→1: 300/250 → уровень 1.
        state.add_active("wood", 300);
        state.add_active("stone", 250);
        assert_eq!(build_forge(&mut state), BuildResult::Built(1));
        assert_eq!(state.forge_level, 1);
    }

    #[test]
    fn coal_pit_unlock_does_not_build_first_level() {
        let mut state = GameState {
            house_level: 1,
            gold: 1_000.0,
            storage_level: 2,
            ..GameState::default()
        };

        // Открытие: 300 золота → уровень 0.
        assert_eq!(unlock_coal_pit_for_test(&mut state), BuildResult::Built(0));
        assert!(state.coal_pit_unlocked);
        assert_eq!(state.coal_pit_level, 0);

        // Постройка 0→1: 180/100 → уровень 1.
        state.add_active("wood", 180);
        state.add_active("stone", 100);
        assert_eq!(build_coal_pit(&mut state), BuildResult::Built(1));
        assert_eq!(state.coal_pit_level, 1);
        assert!(state.coal_pit_researched);
    }

    #[test]
    fn bank_unlock_does_not_build_first_level() {
        let mut state = GameState {
            gold: 10_000.0,
            storage_level: 2,
            ..GameState::default()
        };
        state.add_item("iron_ingot", 25);

        // Открытие: 25 🔩 / 500 💰 → уровень 0.
        assert_eq!(unlock_bank_for_test(&mut state), BuildResult::Built(0));
        assert!(state.bank_unlocked);
        assert_eq!(state.bank_level, 0);
        assert_eq!(state.item_count("iron_ingot"), 0);
        assert!(state.gold < 10_000.0);

        // Постройка 0→1: 300 🪵 / 200 🪨 / 300 💰 → уровень 1.
        state.add_active("wood", 300);
        state.add_active("stone", 200);
        assert_eq!(state.active_amount("wood"), 300);
        assert_eq!(state.active_amount("stone"), 200);
        assert_eq!(build_bank(&mut state), BuildResult::Built(1));
        assert_eq!(state.bank_level, 1);
        assert!(state.trader_researched);
    }

    #[test]
    fn map_study_opens_location_and_spends_one_map() {
        let mut state = GameState::default();
        state.add_item("map_forest", 2);
        assert!(!state.location_unlocked("deep_forest"));

        assert_eq!(study_map(&mut state, "map_forest"), Ok("deep_forest"));
        assert!(state.location_unlocked("deep_forest"));
        assert_eq!(state.item_count("map_forest"), 1);

        // Повторное изучение уже открытой локации карту не тратит.
        assert_eq!(
            study_map(&mut state, "map_forest"),
            Err(StudyMapError::AlreadyOpen)
        );
        assert_eq!(state.item_count("map_forest"), 1);
    }

    #[test]
    fn bought_map_opens_the_same_way() {
        // Карта из торговца — обычный предмет в сумке: открывает изучение.
        let mut state = GameState::default();
        state.add_item("map_mines", 1);
        assert_eq!(study_map(&mut state, "map_mines"), Ok("deep_mines"));
        assert!(state.location_unlocked("deep_mines"));
        assert_eq!(state.item_count("map_mines"), 0);
    }

    #[test]
    fn study_map_rejects_non_maps_and_missing_maps() {
        let mut state = GameState::default();
        assert_eq!(study_map(&mut state, "wood"), Err(StudyMapError::NotMap));
        assert_eq!(
            study_map(&mut state, "map_ruins"),
            Err(StudyMapError::Missing)
        );
        assert!(!state.location_unlocked("ancient_ruins"));
    }

    #[test]
    fn dismantle_returns_half_of_ingredients() {
        let mut state = GameState::default();
        // Рецепт archers_jacket: cloth 5, leather 3, без огня.
        state.add_item("archers_jacket", 1);
        let refund = dismantle_item(&mut state, "archers_jacket").expect("разбор");
        // 50%: 5 → 2, 3 → 1.
        assert_eq!(refund.len(), 2);
        assert_eq!(refund[0], ("cloth".to_string(), 2));
        assert_eq!(refund[1], ("leather".to_string(), 1));
        assert_eq!(state.item_count("archers_jacket"), 0);
        assert_eq!(state.item_count("cloth"), 2);
        assert_eq!(state.item_count("leather"), 1);
    }

    #[test]
    fn dismantle_rejects_non_dismantlable() {
        let mut state = GameState::default();
        state.add_item("board", 5);
        assert_eq!(
            dismantle_item(&mut state, "board"),
            Err(DismantleError::NotDismantlable)
        );
        state.add_item("map_forest", 1);
        assert_eq!(
            dismantle_item(&mut state, "map_forest"),
            Err(DismantleError::NotDismantlable)
        );
    }

    #[test]
    fn wooden_gear_and_buckets_can_be_dismantled() {
        let mut state = GameState::default();
        state.add_item("wooden_shield", 1);
        assert!(dismantle_item(&mut state, "wooden_shield").is_ok());
        state.add_item("wooden_bucket", 1);
        assert!(dismantle_item(&mut state, "wooden_bucket").is_ok());
        // Промежуточный материал разбирать по-прежнему нельзя.
        state.add_item("iron_ingot", 1);
        assert_eq!(
            dismantle_item(&mut state, "iron_ingot"),
            Err(DismantleError::NotDismantlable)
        );
    }

    #[test]
    fn dismantle_rejects_not_owned() {
        let mut state = GameState::default();
        assert_eq!(
            dismantle_item(&mut state, "leather_hat"),
            Err(DismantleError::NotOwned)
        );
    }

    #[test]
    fn dismantle_fire_cost_not_refunded() {
        let mut state = GameState::default();
        // Рецепт iron_armor: iron_ingot 15, leather 4, огонь 200.
        state.add_item("iron_armor", 1);
        let refund = dismantle_item(&mut state, "iron_armor").expect("разбор");
        assert_eq!(refund.len(), 2, "огонь не должен возвращаться");
        assert_eq!(refund[0], ("iron_ingot".to_string(), 7));
        assert_eq!(refund[1], ("leather".to_string(), 2));
    }
}
