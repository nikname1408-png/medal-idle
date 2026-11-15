//! Доска объявлений: горожане просят ресурсы и предлагают награду.
//!
//! Механика:
//! - 6 квестов висят на доске, обновление раз в 10–20 минут.
//! - Игрок берёт до 3 квестов одновременно.
//! - Таймер 10 минут на выполнение. Просрочка — награда −30% + долг.
//! - Отказ в первую минуту — штраф 200 з. в долг, квест остаётся на доске.
//! - Долг растёт на +40% в минуту, максимум 5000 з.
//! - Пока долг не погашен: −25% к продаже + запрет торга.
//! - Если долг больше 5000 — приходят коллекторы и забирают предметы.

use serde::{Deserialize, Serialize};

use super::i18n;
use super::items;
use super::state::GameState;

/// Автор объявления — торговец или просто горожанин по профессии.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestAuthor {
    /// Торговец: `trader.<id>` в словаре (`marta`, `wanderer`, ...).
    Trader(String),
    /// Профессия: `quest.author.<id>` в словаре (`portniha`, `rybak`, ...).
    Citizen(String),
}

impl QuestAuthor {
    pub fn display_name(&self) -> String {
        match self {
            Self::Trader(id) => super::trader::trader_name_by_id(id),
            Self::Citizen(id) => i18n::t_or(&format!("quest.author.{id}"), "?"),
        }
        .to_string()
    }
}

/// Статус квеста.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestStatus {
    /// Висит на доске, можно взять.
    Available,
    /// Взят игроком, таймер тикает.
    Active,
    /// Выполнен (награда получена).
    Completed,
    /// Провален (просрочен или игрок сдался).
    Failed,
}

/// Один квест.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub author: QuestAuthor,
    pub request: Vec<(String, u32)>,
    pub reward: QuestReward,
    pub status: QuestStatus,
    pub created_at: f64,
    pub expires_at: f64,
    pub declined: bool,
    /// ID шаблона текста (`quest.template.{N}.title` / `.body`).
    #[serde(default)]
    pub template_id: u8,
}

/// Награда квеста: либо золото, либо предметы (нельзя смешивать).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum QuestReward {
    Gold(f64),
    Items(Vec<(String, u32)>),
}

impl QuestReward {
    /// Текст для UI: «120 зл.» или «60 кожи».
    pub fn describe(&self) -> String {
        match self {
            Self::Gold(amount) => {
                let text = format!("{}", *amount as u32);
                i18n::tf("quest.reward.gold", &[&text])
            }
            Self::Items(items) => items
                .iter()
                .map(|(id, qty)| {
                    let name = i18n::catalog::name(id);
                    format!("{qty} {name}")
                })
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Приблизительная ценность в золоте (по базовым ценам).
    pub fn base_value(&self) -> f64 {
        match self {
            Self::Gold(amount) => *amount,
            Self::Items(items) => items
                .iter()
                .filter_map(|(id, qty)| {
                    items::item(id).map(|it| it.base_price as f64 * *qty as f64)
                })
                .sum(),
        }
    }

    /// Проверяет, что игрок может получить награду (рецепты открыты
    /// или это золото).
    pub fn is_reward_available(&self, state: &GameState) -> bool {
        match self {
            Self::Gold(_) => true,
            Self::Items(items) => items.iter().all(|(id, _)| {
                items::item(id)
                    .map(|it| state.smith_level() >= items::item_unlock_level(&it.id))
                    .unwrap_or(false)
            }),
        }
    }
}

/// Время между обновлениями доски (10–20 минут).
pub fn board_refresh_interval() -> f64 {
    10.0 * 60.0 + (rand_from_seed() * 10.0 * 60.0)
}

/// Утилита для псевдослучайности — берём текущее время в секундах.
fn rand_from_seed() -> f64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as f64)
        .unwrap_or(0.0);
    (now.sin().abs() * 1000.0).fract()
}

use super::trader::random_below;

/// Обновляет доску: оставляет активные квесты, генерирует новые
/// до 6 штук. Вызывается из `tick_quests`.
pub fn refresh_board(state: &mut GameState) {
    let now = state.total_play_seconds;
    state.quests.retain(|q| q.status == QuestStatus::Active);

    let need = 6_usize.saturating_sub(state.quests.len());
    let mut seed = state.market_seed;
    for slot in 0..need {
        if let Some(quest) = generate_quest(state, &mut seed, slot) {
            state.quests.push(quest);
        }
    }

    state.market_seed = seed;
    state.quest_board_refresh_at = now + board_refresh_interval();
}

/// Генерирует один квест. Может вернуть `None`, если награда заблокирована
/// и запрос не покрывает её по базовой цене — тогда слот пропускается
/// (лучше показать 5 квестов, чем один неберущийся).
fn generate_quest(state: &GameState, seed: &mut u64, slot: usize) -> Option<Quest> {
    let traders = ["marta", "wanderer", "smith", "miner"];
    let citizens = ["portniha", "rybak", "plotnik", "gorschanin", "starosta"];

    // Автор: 60% торговец, 40% горожанин.
    let author = if random_below(seed, 100) < 60 {
        let idx = random_below(seed, traders.len() as u64) as usize;
        QuestAuthor::Trader(traders[idx].to_string())
    } else {
        let idx = random_below(seed, citizens.len() as u64) as usize;
        QuestAuthor::Citizen(citizens[idx].to_string())
    };

    // Что просят.
    let request_pool = [
        // Базовые материалы
        "cloth",
        "whetstone",
        "brick",
        "jug",
        "candle",
        "wooden_bucket",
        "wooden_shovel",
        "wooden_shield",
        // Ресурсы
        "leather",
        "fiber",
        "silver",
        "gold_ore",
        "gems",
        // Слитки
        "iron_ingot",
        "silver_ingot",
        "gold_ingot",
        // Дешёвая одежда
        "robe",
        "leather_gloves",
        "winter_gloves",
        "beggars_gloves",
        "jacket",
        "archers_jacket",
        "leather_coat",
        "beggars_cloak",
        "leather_hat",
        "wretched_hat",
        "wanderers_hood",
        "poor_boots",
        "leather_boots",
        "hiking_boots",
        "winter_boots",
        "backpack",
        // Среднее снаряжение
        "iron_bucket",
        "sickle",
        "scythe",
        "hammer",
        "iron_shovel",
        "iron_bracers",
        "war_hat",
        "silver_ring",
        "trader_ring",
        "miner_amulet",
        "wanderer_pendant",
        "wanderers_belt",
        "belt_with_bag",
        // Сет кузнеца
        "smith_ring",
        "smith_amulet",
        "smith_belt",
    ];
    // Крафтовый предмет, рецепт которого ещё не открыт, в запрос не попадает:
    // такой квест невозможно выполнить. Ресурсы (кожа, волокно, серебро,
    // камни) добываются в походах и доступны всегда.
    let available_pool: Vec<&'static str> = request_pool
        .into_iter()
        .filter(|id| {
            !items::recipes()
                .iter()
                .any(|recipe| recipe.result_id == *id)
                || state.smith_level() >= items::item_unlock_level(id)
        })
        .collect();
    if available_pool.is_empty() {
        return None;
    }
    let request_id = available_pool[random_below(seed, available_pool.len() as u64) as usize];
    let item = items::item(request_id)?;

    let qty = match item.base_price {
        0..=10 => 20 + random_below(seed, 41) as u32,
        11..=50 => 10 + random_below(seed, 21) as u32,
        51..=200 => 5 + random_below(seed, 16) as u32,
        _ => 2 + random_below(seed, 9) as u32,
    };

    let request = vec![(request_id.to_string(), qty)];
    let request_value = item.base_price as f64 * qty as f64;

    // Что дают: 55% золото / 35% ресурсы / 10% готовый предмет.
    let roll = random_below(seed, 100);
    let reward = if roll < 55 {
        let mult = 0.9 + (random_below(seed, 41) as f64 / 100.0);
        QuestReward::Gold((request_value * mult).round().max(1.0))
    } else if roll < 90 {
        let count = 1 + random_below(seed, 2) as usize;
        let target_value = request_value * (1.0 + random_below(seed, 31) as f64 / 100.0);
        let per_item = target_value / count as f64;
        let crafting_pool = [
            "leather",
            "fiber",
            "silver",
            "gold_ore",
            "gems",
            "iron_ingot",
            "silver_ingot",
            "gold_ingot",
            "quartz",
        ];
        let mut items_vec = Vec::new();
        for _ in 0..count {
            let rid = crafting_pool[random_below(seed, crafting_pool.len() as u64) as usize];
            let ritem = items::item(rid)?;
            let rqty = (per_item / ritem.base_price as f64).round().max(1.0) as u32;
            items_vec.push((rid.to_string(), rqty));
        }
        QuestReward::Items(items_vec)
    } else {
        let gear_pool = [
            "robe",
            "leather_gloves",
            "winter_gloves",
            "beggars_gloves",
            "jacket",
            "leather_hat",
            "poor_boots",
            "leather_boots",
            "silver_ring",
            "trader_ring",
            "miner_amulet",
            "wanderer_pendant",
        ];
        let rid = gear_pool[random_below(seed, gear_pool.len() as u64) as usize];
        QuestReward::Items(vec![(rid.to_string(), 1)])
    };

    // Если награда — экипировка, а рецепт не открыт — можно взять, только
    // если запрос по базовой цене не меньше награды (игрок «переплачивает»
    // своим трудом, но не обходит ×5-наценку торговца).
    if let QuestReward::Items(_) = &reward {
        let reward_value = reward.base_value();
        let overpay = request_value >= reward_value;
        if !reward.is_reward_available(state) && !overpay {
            return None;
        }
    }

    let template_id = random_below(seed, 8) as u8;
    let id = format!("q{slot}_{}", state.total_play_seconds as u64);

    Some(Quest {
        id,
        author,
        request,
        reward,
        status: QuestStatus::Available,
        created_at: state.total_play_seconds,
        expires_at: 0.0,
        declined: false,
        template_id,
    })
}

/// Игровой тик: обновляет доску, проверяет просрочку, растит долг.
pub fn tick_quests(state: &mut GameState) {
    let now = state.total_play_seconds;

    // 1. Обновление доски.
    if state.quest_board_refresh_at <= 0.0 {
        // Свежая игра без сохранения: доска должна появиться сама,
        // через 10 секунд после старта.
        state.quest_board_refresh_at = now + 10.0;
    } else if now >= state.quest_board_refresh_at {
        refresh_board(state);
    }

    // 2. Просрочка активных квестов.
    let mut expired = Vec::new();
    for q in state.quests.iter_mut() {
        if q.status == QuestStatus::Active && q.expires_at > 0.0 && now >= q.expires_at {
            q.status = QuestStatus::Failed;
            let penalty = q.reward.base_value() * 0.2;
            state.debt += penalty;
            if state.debt_tick_at <= 0.0 {
                state.debt_tick_at = now + 60.0;
            }
            expired.push(q.id.clone());
        }
    }
    for quest_id in expired {
        state.remove_quest_letter(&quest_id);
    }

    // 3. Рост долга: +40% в минуту, максимум 5000.
    if state.debt > 0.0 {
        if state.debt_tick_at <= 0.0 {
            state.debt_tick_at = now + 60.0;
        } else if now >= state.debt_tick_at {
            state.debt = (state.debt * 1.4).min(5000.0);
            state.debt_tick_at = now + 60.0;
        }
    } else {
        state.debt_tick_at = 0.0;
    }
}

/// Ошибка при работе с квестом.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestError {
    /// Квест не найден.
    NotFound,
    /// Квест уже не Available (взят или выполнен).
    NotAvailable,
    /// Уже 3 активных квеста.
    TooManyActive,
    /// Рецепт награды не открыт, и запрос не покрывает её по цене.
    RewardLocked,
    /// Не хватает предметов для сдачи.
    NotEnoughItems,
    /// Отказаться можно только в первую минуту.
    TooLateToDecline,
    /// Уже отказывались от этого квеста.
    AlreadyDeclined,
}

impl QuestError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotFound => i18n::t("quest.err.not_found"),
            Self::NotAvailable => i18n::t("quest.err.not_available"),
            Self::TooManyActive => i18n::t("quest.err.too_many"),
            Self::RewardLocked => i18n::t("quest.err.reward_locked"),
            Self::NotEnoughItems => i18n::t("quest.err.not_enough"),
            Self::TooLateToDecline => i18n::t("quest.err.too_late"),
            Self::AlreadyDeclined => i18n::t("quest.err.already_declined"),
        }
    }
}

/// Максимум активных квестов у игрока.
pub const MAX_ACTIVE_QUESTS: usize = 3;
/// Время на выполнение активного квеста (секунды).
pub const QUEST_DURATION: f64 = 10.0 * 60.0;
/// Штраф за отказ в первую минуту.
pub const DECLINE_PENALTY: f64 = 200.0;
/// Порог, после которого отказаться нельзя.
pub const DECLINE_WINDOW: f64 = 60.0;

/// Взять квест с доски.
pub fn take_quest(state: &mut GameState, quest_id: &str) -> Result<(), QuestError> {
    let now = state.total_play_seconds;

    let active_count = state
        .quests
        .iter()
        .filter(|q| q.status == QuestStatus::Active)
        .count();
    if active_count >= MAX_ACTIVE_QUESTS {
        return Err(QuestError::TooManyActive);
    }

    // Сначала читаем квест неизменяемо: проверка награды принимает
    // `&GameState` и не даст заём, пока квест взят мутабельно.
    let (request, reward) = {
        let quest = state
            .quests
            .iter()
            .find(|q| q.id == quest_id)
            .ok_or(QuestError::NotFound)?;
        if quest.status != QuestStatus::Available {
            return Err(QuestError::NotAvailable);
        }
        if quest.declined {
            return Err(QuestError::AlreadyDeclined);
        }
        (quest.request.clone(), quest.reward.clone())
    };

    // Проверка награды: если рецепт награды не открыт, разрешаем только
    // если запрос по базовой цене >= награды (игрок «переплачивает»).
    if !reward.is_reward_available(state) {
        let request_value: f64 = request
            .iter()
            .filter_map(|(id, qty)| items::item(id).map(|it| it.base_price as f64 * *qty as f64))
            .sum();
        let reward_value = reward.base_value();
        if request_value < reward_value {
            return Err(QuestError::RewardLocked);
        }
    }

    // Проверяем, что у игрока есть все нужные предметы. Если нет — он
    // может взять квест и докрафтить, но сразу сдать не сможет. Это ок:
    // таймер 10 минут, за это время можно многое успеть.
    let quest = state
        .quests
        .iter_mut()
        .find(|q| q.id == quest_id)
        .ok_or(QuestError::NotFound)?;
    quest.status = QuestStatus::Active;
    quest.expires_at = now + QUEST_DURATION;

    // Письмо в колокольчик: детали заказа и кнопки «Сдать» / «Отклонить».
    state.add_quest_letter(quest_id);

    Ok(())
}

/// Сдать квест: списать предметы, выдать награду.
pub fn complete_quest(state: &mut GameState, quest_id: &str) -> Result<(), QuestError> {
    // Клонируем запрос и награду: дальше меняем `state`, а заём на квест
    // из `state.quests` этого не позволит.
    let (request, reward) = {
        let quest = state
            .quests
            .iter()
            .find(|q| q.id == quest_id)
            .ok_or(QuestError::NotFound)?;
        if quest.status != QuestStatus::Active {
            return Err(QuestError::NotAvailable);
        }
        (quest.request.clone(), quest.reward.clone())
    };

    // Проверяем наличие всех предметов.
    for (id, qty) in &request {
        if state.amount_of(id) < *qty {
            return Err(QuestError::NotEnoughItems);
        }
    }

    // Списываем.
    for (id, qty) in &request {
        if super::state::is_active_resource(id) {
            state.spend_active(id, *qty);
        } else {
            state.remove_item(id, *qty);
        }
    }

    // Выдаём награду.
    match reward {
        QuestReward::Gold(amount) => {
            let before = state.gold;
            state.gold = (state.gold + amount).min(state.gold_cap());
            state.stats.gold_earned += state.gold - before;
        }
        QuestReward::Items(list) => {
            for (id, qty) in list {
                state.add_item(&id, qty);
            }
        }
    }

    // Удаляем выполненный квест из списка.
    state.quests.retain(|q| q.id != quest_id);
    state.remove_quest_letter(quest_id);

    Ok(())
}

/// Отказаться от квеста (только в первую минуту после взятия).
pub fn decline_quest(state: &mut GameState, quest_id: &str) -> Result<(), QuestError> {
    let now = state.total_play_seconds;

    let quest = state
        .quests
        .iter_mut()
        .find(|q| q.id == quest_id)
        .ok_or(QuestError::NotFound)?;

    if quest.status != QuestStatus::Active {
        return Err(QuestError::NotAvailable);
    }

    // Таймер: `expires_at - QUEST_DURATION` — момент взятия.
    // Отказ разрешён в первые DECLINE_WINDOW секунд.
    let started_at = quest.expires_at - QUEST_DURATION;
    if now - started_at > DECLINE_WINDOW {
        return Err(QuestError::TooLateToDecline);
    }

    // Возвращаем на доску со штрафом.
    quest.status = QuestStatus::Available;
    quest.expires_at = 0.0;
    quest.declined = true;

    state.debt += DECLINE_PENALTY;
    if state.debt_tick_at <= 0.0 {
        state.debt_tick_at = now + 60.0;
    }

    state.remove_quest_letter(quest_id);

    Ok(())
}

/// Сколько процентов долга коллекторы забирают за один визит.
const COLLECTOR_TAKE_PERCENT: f64 = 0.15;
/// Максимум стоимости, которую могут забрать за визит (золото).
const COLLECTOR_MAX_TAKE: f64 = 500.0;
/// Интервал между визитами коллекторов (секунды).
const COLLECTOR_INTERVAL: f64 = 600.0;

/// Проверяет, не пора ли коллекторам прийти. Если долг есть — забирают
/// предметы из инвентаря на часть суммы.
pub fn tick_collectors(state: &mut GameState) {
    let now = state.total_play_seconds;

    if state.debt <= 0.0 {
        state.collector_tick_at = 0.0;
        return;
    }

    // Первый раз — ставим таймер, но не приходим.
    if state.collector_tick_at <= 0.0 {
        state.collector_tick_at = now + COLLECTOR_INTERVAL;
        return;
    }

    if now < state.collector_tick_at {
        return;
    }

    // Пора забирать.
    let target_value = (state.debt * COLLECTOR_TAKE_PERCENT).min(COLLECTOR_MAX_TAKE);
    let (taken_items, taken_value) = take_items_for_value(state, target_value);

    if taken_value > 0.0 {
        state.debt = (state.debt - taken_value).max(0.0);
        state.pending_event = Some(super::events::PendingEvent {
            payload: super::events::EventPayload::CollectorsArrived {
                gold_taken: taken_value,
                items: taken_items,
            },
        });
    }

    // Если долг всё ещё есть — следующий визит через COLLECTOR_INTERVAL.
    if state.debt > 0.0 {
        state.collector_tick_at = now + COLLECTOR_INTERVAL;
    } else {
        state.collector_tick_at = 0.0;
    }
}

/// Забирает предметы из инвентаря на указанную сумму. Идёт от самых дорогих
/// к самым дешёвым, пока не наберёт. Возвращает `(что_забрали, сумма)`.
fn take_items_for_value(state: &mut GameState, target: f64) -> (Vec<(String, u32)>, f64) {
    // Собираем все предметы с ценой > 0, сортируем по убыванию цены за штуку.
    let mut candidates: Vec<(String, u32, u32)> = state
        .inventory
        .iter()
        .filter(|(_, qty)| **qty > 0)
        .filter_map(|(id, qty)| items::item(id).map(|it| (id.clone(), *qty, it.base_price)))
        .filter(|(_, _, price)| *price > 0)
        .collect();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.2));

    let mut taken: Vec<(String, u32)> = Vec::new();
    let mut taken_value = 0.0;

    for (id, qty, price) in candidates {
        if taken_value >= target {
            break;
        }
        let need = target - taken_value;
        let unit_price = price as f64;
        let want = (need / unit_price).ceil() as u32;
        let take = want.min(qty);
        if take == 0 {
            continue;
        }
        state.remove_item(&id, take);
        taken.push((id, take));
        taken_value += unit_price * take as f64;
    }

    (taken, taken_value)
}

/// Оплата долга золотом из руки.
pub fn pay_debt(state: &mut GameState, amount: f64) -> f64 {
    let amount = amount.max(0.0).min(state.debt).min(state.gold);
    state.gold -= amount;
    state.debt -= amount;
    if state.debt <= 0.0 {
        state.debt = 0.0;
        state.debt_tick_at = 0.0;
        state.collector_tick_at = 0.0;
    }
    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    #[test]
    fn refresh_board_generates_six_quests() {
        let mut state = GameState::default();
        refresh_board(&mut state);
        assert_eq!(state.quests.len(), 6);
        for q in &state.quests {
            assert_eq!(q.status, QuestStatus::Available);
            assert!(!q.request.is_empty());
            assert!(q.reward.base_value() > 0.0);
        }
    }

    #[test]
    fn board_refreshes_after_interval() {
        let mut state = GameState::default();
        state.quest_board_refresh_at = 10.0;
        state.total_play_seconds = 0.0;
        tick_quests(&mut state); // не должен обновить
        assert!(state.quests.is_empty());
        state.total_play_seconds = 11.0;
        tick_quests(&mut state); // обновит
        assert_eq!(state.quests.len(), 6);
    }

    #[test]
    fn fresh_game_board_appears_on_its_own() {
        // Новая игра приходит без миграции сохранения: доска всё равно
        // должна наполниться сама.
        let mut state = GameState::default();
        tick_quests(&mut state);
        assert!(state.quests.is_empty());
        state.total_play_seconds = 11.0;
        tick_quests(&mut state);
        assert_eq!(state.quests.len(), 6);
    }

    #[test]
    fn requests_respect_smith_level() {
        // На первом уровне кузнеца доска не должна просить предметы,
        // рецепт которых ещё закрыт.
        let mut state = GameState::default();
        state.smith_level = 1;
        refresh_board(&mut state);
        for quest in &state.quests {
            for (id, _) in &quest.request {
                let craftable = items::recipes().iter().any(|r| r.result_id == id.as_str());
                if craftable {
                    assert!(
                        items::item_unlock_level(id) <= 1,
                        "запрос {id} недоступен на 1-м уровне"
                    );
                }
            }
        }
    }

    #[test]
    fn taking_a_quest_leaves_a_letter_and_declining_removes_it() {
        let mut state = GameState::default();
        refresh_board(&mut state);

        let mut taken = None;
        for quest in state.quests.clone() {
            if take_quest(&mut state, &quest.id).is_ok() {
                taken = Some(quest.id);
                break;
            }
        }
        let quest_id = taken.expect("ни один заказ не взялся");
        assert!(
            state
                .letters
                .iter()
                .any(|letter| letter.quest.as_deref() == Some(quest_id.as_str())),
            "в колокольчике должно появиться письмо о заказе"
        );

        decline_quest(&mut state, &quest_id).expect("отказ");
        assert!(
            !state
                .letters
                .iter()
                .any(|letter| letter.quest.as_deref() == Some(quest_id.as_str())),
            "письмо отклонённого заказа должно исчезнуть"
        );
    }

    #[test]
    fn expired_quest_grows_debt() {
        let mut state = GameState::default();
        refresh_board(&mut state);
        let quest = state.quests.first_mut().unwrap();
        quest.status = QuestStatus::Active;
        quest.expires_at = 5.0;
        let reward_value = quest.reward.base_value();
        state.total_play_seconds = 10.0;
        tick_quests(&mut state);
        assert!(state.debt > 0.0);
        assert!((state.debt - reward_value * 0.2).abs() < 1e-9);
    }

    #[test]
    fn debt_grows_per_minute_up_to_cap() {
        let mut state = GameState::default();
        state.debt = 100.0;
        state.debt_tick_at = 60.0;
        state.total_play_seconds = 60.0;
        tick_quests(&mut state);
        assert!((state.debt - 140.0).abs() < 1e-9);
        // Через 100 минут долг упрётся в 5000.
        for _ in 0..200 {
            state.total_play_seconds += 60.0;
            tick_quests(&mut state);
        }
        assert!((state.debt - 5000.0).abs() < 1e-9);
    }

    #[test]
    fn take_quest_sets_active_with_timer() {
        let mut state = GameState::default();
        refresh_board(&mut state);
        let id = state.quests[0].id.clone();
        assert_eq!(take_quest(&mut state, &id), Ok(()));
        let quest = state.quests.iter().find(|q| q.id == id).unwrap();
        assert_eq!(quest.status, QuestStatus::Active);
        assert!((quest.expires_at - (state.total_play_seconds + QUEST_DURATION)).abs() < 1e-9);
    }

    #[test]
    fn cannot_take_more_than_three() {
        let mut state = GameState::default();
        refresh_board(&mut state);
        let ids: Vec<String> = state.quests.iter().map(|q| q.id.clone()).collect();
        for id in ids.iter().take(3) {
            assert_eq!(take_quest(&mut state, id), Ok(()));
        }
        assert_eq!(
            take_quest(&mut state, &ids[3]),
            Err(QuestError::TooManyActive)
        );
    }

    #[test]
    fn complete_quest_spends_items_and_pays() {
        let mut state = GameState::default();
        state.quests.push(Quest {
            id: "test".to_string(),
            author: QuestAuthor::Citizen("portniha".to_string()),
            request: vec![("cloth".to_string(), 5)],
            reward: QuestReward::Gold(100.0),
            status: QuestStatus::Active,
            created_at: 0.0,
            expires_at: 600.0,
            declined: false,
            template_id: 0,
        });
        state.add_item("cloth", 10);
        let before_gold = state.gold;

        assert_eq!(complete_quest(&mut state, "test"), Ok(()));
        assert_eq!(state.item_count("cloth"), 5);
        assert_eq!(state.gold, before_gold + 100.0);
        assert!(state.quests.iter().all(|q| q.id != "test"));
    }

    #[test]
    fn complete_quest_fails_without_items() {
        let mut state = GameState::default();
        state.quests.push(Quest {
            id: "test".to_string(),
            author: QuestAuthor::Citizen("portniha".to_string()),
            request: vec![("cloth".to_string(), 5)],
            reward: QuestReward::Gold(100.0),
            status: QuestStatus::Active,
            created_at: 0.0,
            expires_at: 600.0,
            declined: false,
            template_id: 0,
        });
        assert_eq!(
            complete_quest(&mut state, "test"),
            Err(QuestError::NotEnoughItems)
        );
    }

    #[test]
    fn decline_within_first_minute_costs_200() {
        let mut state = GameState::default();
        state.quests.push(Quest {
            id: "test".to_string(),
            author: QuestAuthor::Citizen("portniha".to_string()),
            request: vec![("cloth".to_string(), 1)],
            reward: QuestReward::Gold(10.0),
            status: QuestStatus::Active,
            created_at: 0.0,
            expires_at: QUEST_DURATION,
            declined: false,
            template_id: 0,
        });
        state.total_play_seconds = 30.0;
        assert_eq!(decline_quest(&mut state, "test"), Ok(()));
        assert_eq!(state.debt, DECLINE_PENALTY);
        let quest = state.quests.iter().find(|q| q.id == "test").unwrap();
        assert_eq!(quest.status, QuestStatus::Available);
        assert!(quest.declined);
    }

    #[test]
    fn decline_after_first_minute_fails() {
        let mut state = GameState::default();
        state.quests.push(Quest {
            id: "test".to_string(),
            author: QuestAuthor::Citizen("portniha".to_string()),
            request: vec![("cloth".to_string(), 1)],
            reward: QuestReward::Gold(10.0),
            status: QuestStatus::Active,
            created_at: 0.0,
            expires_at: QUEST_DURATION,
            declined: false,
            template_id: 0,
        });
        state.total_play_seconds = 120.0;
        assert_eq!(
            decline_quest(&mut state, "test"),
            Err(QuestError::TooLateToDecline)
        );
    }

    #[test]
    fn debt_sell_multiplier_applies() {
        let mut state = GameState::default();
        assert_eq!(state.debt_sell_multiplier(), 1.0);
        state.debt = 100.0;
        assert_eq!(state.debt_sell_multiplier(), 0.75);
        assert!(!state.can_haggle());
    }

    #[test]
    fn collectors_take_items_and_reduce_debt() {
        let mut state = GameState::default();
        state.debt = 1000.0;
        state.collector_tick_at = 100.0;
        state.total_play_seconds = 100.0;
        state.add_item("iron_ingot", 50);

        tick_collectors(&mut state);

        // Должны забрать 15% от 1000 = 150 зл. Один iron_ingot = 24 зл.
        // Значит забирают ceil(150/24) = 7 штук, стоимость 168 зл.
        assert!(state.debt < 1000.0);
        assert!(state.item_count("iron_ingot") < 50);
        assert!(state.pending_event.is_some());
    }

    #[test]
    fn paying_debt_clears_state() {
        let mut state = GameState::default();
        state.gold = 500.0;
        state.debt = 300.0;
        let paid = pay_debt(&mut state, 300.0);
        assert!((paid - 300.0).abs() < 1e-9);
        assert_eq!(state.debt, 0.0);
        assert_eq!(state.gold, 200.0);
        assert!(state.can_haggle());
    }

    #[test]
    fn paying_debt_partial_keeps_state() {
        let mut state = GameState::default();
        state.gold = 100.0;
        state.debt = 300.0;
        let paid = pay_debt(&mut state, 300.0);
        // Списалось только 100 (сколько было).
        assert!((paid - 100.0).abs() < 1e-9);
        assert!((state.debt - 200.0).abs() < 1e-9);
        assert_eq!(state.gold, 0.0);
    }
}
