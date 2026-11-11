//! Походы по локациям.
//!
//! Поход длится, пока есть энергия: каждую секунду пути тратится расход
//! локации. Каждую секунду с шансом [`balance::EXPEDITION_LOOT_CHANCE`]
//! находится лут; затем разыгрывается редкость (обычный/необычный/редкий/
//! эпический/легендарный). Если нужной редкости в локации нет, она понижается
//! до максимально доступной. Карты новых локаций падают только в последней
//! открытой локации. Ресурсы склада (дерево, камень) уходят на склад, остальное
//! — в рюкзак; если рюкзак полон, поход прерывается.
//!
//! Модуль не зависит от Dioxus: логику можно проверять через `cargo test`.

use super::actions::next_random;
use super::balance;
use super::state::GameState;

/// Ошибка похода.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum ExpeditionError {
    UnknownLocation,
    NoEnergy,
}

/// Редкость лута.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Вес редкости в промилле.
    pub fn weight(self) -> u32 {
        match self {
            Self::Common => balance::EXPEDITION_COMMON_WEIGHT,
            Self::Uncommon => balance::EXPEDITION_UNCOMMON_WEIGHT,
            Self::Rare => balance::EXPEDITION_RARE_WEIGHT,
            Self::Epic => balance::EXPEDITION_EPIC_WEIGHT,
            Self::Legendary => balance::EXPEDITION_LEGENDARY_WEIGHT,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Common => "Обычный",
            Self::Uncommon => "Необычный",
            Self::Rare => "Редкий",
            Self::Epic => "Эпический",
            Self::Legendary => "Легендарный",
        }
    }

    /// Цвет в интерфейсе.
    pub fn color(self) -> &'static str {
        match self {
            Self::Common => "#cfc7b0",
            Self::Uncommon => "#7fcf7a",
            Self::Rare => "#5aa8e6",
            Self::Epic => "#b777e8",
            Self::Legendary => "#e6a53f",
        }
    }
}

/// Одна строка таблицы лута локации.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootEntry {
    /// ID ресурса или предмета.
    pub item_id: &'static str,
    pub rarity: Rarity,
    pub min_quantity: u32,
    pub max_quantity: u32,
}

/// Описание локации похода.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpeditionLocation {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Расход энергии в секунду.
    pub energy_per_second: f64,
    /// Требуемый предмет (кирка/свеча); `None` — без требования.
    pub requires_item: Option<(&'static str, &'static str)>,
    /// Таблица лута с редкостями.
    pub loot: &'static [LootEntry],
    /// Карта, которая падает в этой локации как обычный лут (`None` — нет).
    pub map_reward: Option<&'static str>,
}

impl ExpeditionLocation {
    /// Локация тёмная: в пути тратится свеча.
    pub fn requires_candle(&self) -> bool {
        self.requires_item
            .is_some_and(|(item_id, _)| item_id == "candle")
    }

    /// Таблица лута с учётом карты: карта — редкий лут локации.
    pub fn table(&self) -> Vec<LootEntry> {
        let mut loot = self.loot.to_vec();
        if let Some(map_id) = self.map_reward {
            loot.push(LootEntry {
                item_id: map_id,
                rarity: Rarity::Rare,
                min_quantity: 1,
                max_quantity: 1,
            });
        }
        loot
    }

    /// Есть ли в локации лут заданной редкости.
    pub fn has_rarity(&self, rarity: Rarity) -> bool {
        self.loot.iter().any(|entry| entry.rarity == rarity)
            || (self.map_reward.is_some() && rarity == Rarity::Rare)
    }

    /// Понижает редкость до максимально доступной в локации.
    fn downgrade(&self, rarity: Rarity) -> Option<Rarity> {
        let mut candidate = rarity;
        loop {
            if self.has_rarity(candidate) {
                return Some(candidate);
            }
            candidate = match candidate {
                Rarity::Legendary => Rarity::Epic,
                Rarity::Epic => Rarity::Rare,
                Rarity::Rare => Rarity::Uncommon,
                Rarity::Uncommon => Rarity::Common,
                Rarity::Common => return None,
            };
        }
    }
}

const fn entry(
    item_id: &'static str,
    rarity: Rarity,
    min_quantity: u32,
    max_quantity: u32,
) -> LootEntry {
    LootEntry {
        item_id,
        rarity,
        min_quantity,
        max_quantity,
    }
}

/// Таблица лута «Заброшенного поля».
const FIELD_LOOT: [LootEntry; 3] = [
    entry("wood", Rarity::Common, 10, 70),
    entry("clay", Rarity::Common, 3, 8),
    entry("fiber", Rarity::Uncommon, 2, 5),
];

/// Таблица лута «Скалистого оврага».
const RAVINE_LOOT: [LootEntry; 3] = [
    entry("stone", Rarity::Common, 10, 70),
    entry("iron", Rarity::Common, 2, 10),
    entry("quartz", Rarity::Uncommon, 1, 3),
];

/// Таблица лута «Дремучего леса».
const DEEP_FOREST_LOOT: [LootEntry; 3] = [
    entry("wood", Rarity::Common, 50, 100),
    entry("leather", Rarity::Uncommon, 2, 6),
    entry("fiber", Rarity::Uncommon, 5, 15),
];

/// Таблица лута «Глубоких шахт».
const DEEP_MINES_LOOT: [LootEntry; 4] = [
    entry("stone", Rarity::Common, 80, 120),
    entry("iron", Rarity::Common, 5, 15),
    entry("quartz", Rarity::Uncommon, 2, 5),
    entry("silver", Rarity::Rare, 1, 3),
];

/// Таблица лута «Забытой каменоломни».
const FORGOTTEN_QUARRY_LOOT: [LootEntry; 5] = [
    entry("stone", Rarity::Common, 100, 150),
    entry("iron", Rarity::Common, 10, 20),
    entry("silver", Rarity::Rare, 2, 7),
    entry("gold_ore", Rarity::Epic, 1, 2),
    entry("gems", Rarity::Legendary, 1, 1),
];

/// Таблица лута «Древних руин».
const ANCIENT_RUINS_LOOT: [LootEntry; 6] = [
    entry("stone", Rarity::Common, 50, 100),
    entry("iron", Rarity::Common, 10, 22),
    entry("quartz", Rarity::Uncommon, 5, 10),
    entry("silver", Rarity::Rare, 2, 7),
    entry("gold_ore", Rarity::Epic, 1, 4),
    entry("gems", Rarity::Legendary, 1, 2),
];

/// Все локации походов по цепочке открытия.
pub const LOCATIONS: [ExpeditionLocation; 6] = [
    ExpeditionLocation {
        id: "field",
        name: "Заброшенное поле",
        description: "Бывшее крестьянское поле: дерево, глина и волокно.",
        energy_per_second: 5.0,
        requires_item: None,
        loot: &FIELD_LOOT,
        map_reward: Some("map_forest"),
    },
    ExpeditionLocation {
        id: "ravine",
        name: "Скалистый овраг",
        description: "Каменистый овраг: камень, железная руда и кварц.",
        energy_per_second: 7.5,
        requires_item: None,
        loot: &RAVINE_LOOT,
        map_reward: Some("map_forest"),
    },
    ExpeditionLocation {
        id: "deep_forest",
        name: "Дремучий лес",
        description: "Дальний лес: много дерева, кожа и волокно.",
        energy_per_second: 10.0,
        requires_item: None,
        loot: &DEEP_FOREST_LOOT,
        map_reward: Some("map_mines"),
    },
    ExpeditionLocation {
        id: "deep_mines",
        name: "Глубокие шахты",
        description: "Глубокие штольни: камень, железная руда и кварц. Нужна свеча.",
        energy_per_second: 15.0,
        requires_item: Some(("candle", "Свеча")),
        loot: &DEEP_MINES_LOOT,
        map_reward: Some("map_quarry"),
    },
    ExpeditionLocation {
        id: "forgotten_quarry",
        name: "Забытая каменоломня",
        description: "Старая каменоломня: камень и железная руда. Нужна свеча.",
        energy_per_second: 17.5,
        requires_item: Some(("candle", "Свеча")),
        loot: &FORGOTTEN_QUARRY_LOOT,
        map_reward: Some("map_ruins"),
    },
    ExpeditionLocation {
        id: "ancient_ruins",
        name: "Древние руины",
        description: "Разрушенный город: кварц, серебро и золотая руда. Нужен факел или свеча.",
        energy_per_second: 20.0,
        requires_item: Some(("candle", "Факел или свеча")),
        loot: &ANCIENT_RUINS_LOOT,
        map_reward: None,
    },
];

/// Точка входа похода. С начала открыты только поле и овраг: лес и шахты
/// открываются картами.
pub const STARTING_LOCATIONS: [&str; 2] = ["field", "ravine"];

/// Находит локацию по идентификатору.
pub fn location(id: &str) -> Option<&'static ExpeditionLocation> {
    LOCATIONS.iter().find(|location| location.id == id)
}

/// Карта, которой открывается локация.
pub fn unlock_map(location_id: &str) -> Option<&'static str> {
    match location_id {
        "field" | "ravine" => None,
        "deep_forest" => Some("map_forest"),
        "deep_mines" => Some("map_mines"),
        "forgotten_quarry" => Some("map_quarry"),
        "ancient_ruins" => Some("map_ruins"),
        _ => None,
    }
}

/// Локация, карту которой даёт предмет.
pub fn location_for_map(map_id: &str) -> Option<&'static str> {
    match map_id {
        "map_forest" => Some("deep_forest"),
        "map_mines" => Some("deep_mines"),
        "map_quarry" => Some("forgotten_quarry"),
        "map_ruins" => Some("ancient_ruins"),
        _ => None,
    }
}

/// Одна находка за поход.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loot {
    pub item_id: String,
    pub quantity: u32,
    /// `true` — ресурс ушёл на склад, `false` — в рюкзак.
    pub active: bool,
}

/// Итог похода.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpeditionReport {
    /// Найденное и реально добавленное.
    pub loot: Vec<Loot>,
    /// Рюкзак заполнился и поход прервался.
    pub interrupted: bool,
    /// Энергия закончилась и поход прервался.
    pub out_of_energy: bool,
    /// В тёмной локации догорела последняя свеча.
    pub out_of_candle: bool,
    /// Часть добычи не поместилась на склад и сгорела.
    pub overflow: bool,
}

/// Итог одного тика похода.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpeditionTick {
    /// Найденное за этот тик.
    pub loot: Vec<Loot>,
    /// Найдено за время шага, сек (для счётчика времени в пути).
    pub seconds: f64,
    /// Рюкзак заполнился — поход прерван.
    pub interrupted: bool,
    /// Энергия закончилась — поход прерван.
    pub out_of_energy: bool,
    /// В тёмной локации догорела последняя свеча.
    pub out_of_candle: bool,
    /// Часть добычи не поместилась на склад и сгорела.
    pub overflow: bool,
}

impl ExpeditionTick {
    /// Поход пора заканчивать.
    pub fn is_over(&self) -> bool {
        self.interrupted || self.out_of_energy || self.out_of_candle
    }
}

/// Бросок `0..modulo`. Берём старшие биты: у ЛКГ младшие сильно коррелируют,
/// и на них шансы «плывут» (карта выпадала в 1.3 раза чаще задуманного).
fn roll(seed: &mut u64, modulo: u64) -> u64 {
    (next_random(seed) >> 33) % modulo
}

/// Разбрасывает броски по редкости и возвращает выпавшую.
fn roll_rarity(seed: &mut u64) -> Rarity {
    let mut pick = roll(seed, 1000);
    for rarity in [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ] {
        let weight = rarity.weight() as u64;
        if pick < weight {
            return rarity;
        }
        pick -= weight;
    }
    Rarity::Common
}

/// Случайная строка таблицы лута нужной редкости.
fn roll_loot_entry(
    location: &ExpeditionLocation,
    rarity: Rarity,
    seed: &mut u64,
) -> Option<LootEntry> {
    // Карта не участвует в обычном выборе: её шанс считается отдельно,
    // иначе каждый новый редкий предмет в таблице разбавлял бы карту.
    let candidates: Vec<LootEntry> = location
        .table()
        .into_iter()
        .filter(|entry| entry.rarity == rarity && Some(entry.item_id) != location.map_reward)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let index = roll(seed, candidates.len() as u64) as usize;
    Some(candidates[index])
}

/// Карта падает в рюкзак и ждёт, пока её изучат в сумке.
fn grant_map(state: &mut GameState, tick: &mut ExpeditionTick, map_id: &'static str) {
    if !state.add_item(map_id, 1) {
        return;
    }
    push_loot(&mut tick.loot, map_id, 1, false);
}

/// Один шаг похода длительностью `dt` секунд.
///
/// Энергия тратится пропорционально времени; когда она кончается, поход
/// останавливается сам.
pub fn step_expedition(state: &mut GameState, location_id: &str, dt: f64) -> ExpeditionTick {
    let mut tick = ExpeditionTick::default();
    let Some(location) = location(location_id) else {
        tick.out_of_energy = true;
        return tick;
    };

    // Тёмные локации тратят свечу: 1 свеча на CANDLE_SECONDS_PER_ITEM секунд.
    if location.requires_candle() {
        for _ in 0..dt.max(0.0).round() as u32 {
            // Начинается новая свеча — сразу списываем её из сумки.
            if state.expedition_candle_timer == 0 && !state.remove_item("candle", 1) {
                tick.out_of_candle = true;
                return tick;
            }
            state.expedition_candle_timer += 1;
            if state.expedition_candle_timer >= balance::CANDLE_SECONDS_PER_ITEM {
                state.expedition_candle_timer = 0;
            }
        }
    }

    let seconds = dt.max(0.0) + state.expedition_timer;
    let rolls = seconds.floor() as u32;
    state.expedition_timer = seconds - rolls as f64;

    // Зимние перчатки и куртка снижают расход энергии в пути.
    let energy_discount = state.expedition_energy_discount.clamp(0.0, 0.9) as f64;
    let energy_cost = location.energy_per_second * (1.0 - energy_discount);
    if !state.spend_energy(energy_cost * dt.max(0.0)) {
        tick.out_of_energy = true;
        return tick;
    }
    tick.seconds = dt.max(0.0);
    if rolls == 0 {
        return tick;
    }

    let bonus = 1.0 + state.expedition_bonus as f64;
    let loot_chance = balance::EXPEDITION_LOOT_CHANCE as u64;
    // Карта падает только в последней открытой локации.
    let is_current = state.is_current_location(location.id);

    for _ in 0..rolls {
        // Каждый бросок продвигает зерно, поэтому тики не повторяют друг друга.
        state.market_seed = state.market_seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut seed = state.market_seed;

        // Особый лут Древних руин: отдельная проверка 1% в секунду.
        if location.id == "ancient_ruins"
            && roll(&mut seed, 100) < balance::EXPEDITION_STEAM_BLUEPRINT_CHANCE as u64
        {
            if !state.add_item("steam_blueprint", 1) {
                tick.interrupted = true;
                return tick;
            }
            push_loot(&mut tick.loot, "steam_blueprint", 1, false);
        }

        if roll(&mut seed, 100) >= loot_chance {
            continue;
        }

        let rarity = roll_rarity(&mut seed);
        let Some(rarity) = location.downgrade(rarity) else {
            continue;
        };

        // Карта локации падает только в последней открытой локации и редко.
        // Проверка идёт до выбора предмета, чтобы шанс карты не зависел
        // от того, сколько других редких предметов в таблице.
        if rarity == Rarity::Rare && is_current {
            if let Some(map_id) = location.map_reward {
                if roll(&mut seed, 100) < balance::EXPEDITION_MAP_CHANCE as u64 {
                    // Карта — одна штука, а не стак ресурса.
                    grant_map(state, &mut tick, map_id);
                    continue;
                }
            }
        }

        let Some(entry) = roll_loot_entry(location, rarity, &mut seed) else {
            continue;
        };

        let span = (entry.max_quantity - entry.min_quantity + 1) as u64;
        let base = entry.min_quantity as f64 + roll(&mut seed, span) as f64;
        let quantity = (base * bonus).round() as u32;

        if super::state::is_active_resource(entry.item_id) {
            // Излишки сверх лимита склада сгорают.
            let added = state.add_active(entry.item_id, quantity);
            if added < quantity {
                tick.overflow = true;
            }
            if added > 0 {
                push_loot(&mut tick.loot, entry.item_id, added, true);
            }
        } else {
            let added = state.item_capacity_left(entry.item_id).min(quantity);
            if added == 0 {
                // Рюкзак полон: поход прерывается, уже найденное остаётся.
                tick.interrupted = true;
                return tick;
            }
            if !state.add_item(entry.item_id, added) {
                tick.overflow = true;
            }
            push_loot(&mut tick.loot, entry.item_id, added, false);
        }
    }

    tick
}

/// Продвигает активный поход в игровом тике. Возвращает `true`, если что-то
/// изменилось (нашлась добыча, открылась локация, поход закончился).
pub fn tick_active_expedition(state: &mut GameState, dt: f64) -> bool {
    // Законченный поход (кнопка «Закончить», мало энергии, полный рюкзак) больше
    // не двигается: иначе после восстановления энергии он «оживал» и продолжал
    // тратить энергию и выдавать добычу.
    let Some(location_id) = state
        .active_expedition
        .as_ref()
        .filter(|active| !active.done)
        .map(|active| active.location_id.clone())
    else {
        return false;
    };

    // Защита от зависшего похода после сворачивания игры: без запаса энергии
    // поход закрывается, а не крутится вхолостую.
    if state.energy <= balance::EXPEDITION_MIN_ENERGY {
        if let Some(active) = state.active_expedition.as_mut() {
            active.notice = Some("Поход прерван: мало энергии.".to_string());
            active.done = true;
        }
        return true;
    }

    // Ограничиваем шаг: после паузы не должно проскакивать много секунд сразу.
    let tick = step_expedition(state, &location_id, dt.clamp(0.0, 5.0));
    let mut changed = false;

    if let Some(active) = state.active_expedition.as_mut() {
        if !tick.loot.is_empty() {
            for loot in &tick.loot {
                active.push_found(&loot.item_id, loot.quantity, loot.active);
            }
            changed = true;
        }
        if let Some(map_id) = tick
            .loot
            .iter()
            .map(|loot| loot.item_id.as_str())
            .find(|item_id| location_for_map(item_id).is_some())
        {
            let name = location_for_map(map_id)
                .and_then(location)
                .map(|location| location.name)
                .unwrap_or("новая локация");
            active.notice = Some(format!("Найдена карта: «{name}». Изучи её в сумке."));
            changed = true;
        }
        if tick.overflow {
            active.notice = Some("Склад полон — часть добычи сгорела.".to_string());
            changed = true;
        }
        active.elapsed = active.elapsed.saturating_add(dt.max(0.0).round() as u32);
        // Свеча кончилась в тёмной локации — поход тоже прерывается.
        if tick.out_of_candle {
            active.notice = Some("Поход прерван: свеча догорела.".to_string());
            changed = true;
        } else if tick.out_of_energy {
            active.notice = Some("Поход прерван: энергия закончилась.".to_string());
        }
        if tick.interrupted {
            active.notice = Some("Поход прерван: рюкзак полон.".to_string());
        }
        if tick.is_over() {
            // Поход окончен; запись остаётся ради итоговой сводки.
            active.done = true;
            changed = true;
        }
    }
    changed
}

/// Отправляет игрока в поход на `seconds` секунд одним разом.
///
/// Используется в тестах и для быстрой проверки; интерфейс тикает по секундам.
#[cfg_attr(not(test), allow(dead_code))]
pub fn run_expedition(
    state: &mut GameState,
    location_id: &str,
    seconds: u32,
) -> Result<ExpeditionReport, ExpeditionError> {
    let Some(location) = location(location_id) else {
        return Err(ExpeditionError::UnknownLocation);
    };
    if state.energy + 1e-9 < location.energy_per_second {
        return Err(ExpeditionError::NoEnergy);
    }

    let mut report = ExpeditionReport::default();
    for _ in 0..seconds {
        let tick = step_expedition(state, location_id, 1.0);
        for loot in tick.loot {
            push_loot(&mut report.loot, &loot.item_id, loot.quantity, loot.active);
        }
        report.overflow |= tick.overflow;
        if tick.interrupted {
            report.interrupted = true;
            break;
        }
        if tick.out_of_candle {
            report.out_of_candle = true;
            report.out_of_energy = true;
            break;
        }
        if tick.out_of_energy {
            report.out_of_energy = true;
            break;
        }
    }
    Ok(report)
}

/// Сливает одинаковые находки в одну строку.
fn push_loot(loot: &mut Vec<Loot>, item_id: &str, quantity: u32, active: bool) {
    if let Some(entry) = loot
        .iter_mut()
        .find(|entry| entry.item_id == item_id && entry.active == active)
    {
        entry.quantity += quantity;
    } else {
        loot.push(Loot {
            item_id: item_id.to_string(),
            quantity,
            active,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Энергии хватит на много часов пути.
    fn endless_state() -> GameState {
        GameState {
            energy: 1_000_000.0,
            ..GameState::default()
        }
    }

    /// Энергии и места на складе хватит на долгий поход.
    fn endlessly_rich() -> GameState {
        let mut state = GameState {
            energy: 1_000_000.0,
            storage_level: 100,
            ..GameState::default()
        };
        // Темные локации тратят свечу: 1 штука на 200 секунд пути.
        state.add_item("candle", 10_000);
        state
    }

    #[test]
    fn active_expedition_runs_in_game_tick() {
        use crate::game::state::ActiveExpedition;

        let mut state = endless_state();
        state.active_expedition = Some(ActiveExpedition::new("field"));
        for _ in 0..30 {
            tick_active_expedition(&mut state, 1.0);
        }

        let active = state.active_expedition.as_ref().expect("поход идёт");
        assert_eq!(active.elapsed, 30);
        assert!(!active.done);
        assert!(state.energy < 1_000_000.0);
    }

    #[test]
    fn unknown_location_is_rejected() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        assert_eq!(
            run_expedition(&mut state, "nowhere", 10),
            Err(ExpeditionError::UnknownLocation)
        );
        assert_eq!(state.energy, 100.0);
    }

    #[test]
    fn expedition_requires_energy() {
        let mut state = GameState {
            energy: 0.0,
            ..GameState::default()
        };

        assert_eq!(
            run_expedition(&mut state, "field", 10),
            Err(ExpeditionError::NoEnergy)
        );
    }

    #[test]
    fn expedition_spends_location_energy_per_second() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        run_expedition(&mut state, "ravine", 10).expect("поход");
        let spent = 100.0 - state.energy;
        assert!((spent - 7.5 * 10.0).abs() < 1e-9);
    }

    #[test]
    fn expedition_stops_when_energy_runs_out() {
        let mut state = GameState {
            energy: 5.0 * 3.0,
            ..GameState::default()
        };

        let report = run_expedition(&mut state, "field", 100).expect("поход");
        assert!(report.out_of_energy);
        assert!(state.energy.abs() < 1e-9);
    }

    #[test]
    fn rarity_downgrades_to_location_max() {
        // В каменоломне есть все редкости, включая драгоценные камни,
        // поэтому понижать ничего не нужно.
        let quarry = location("forgotten_quarry").expect("локация");
        assert_eq!(quarry.downgrade(Rarity::Legendary), Some(Rarity::Legendary));
        assert_eq!(quarry.downgrade(Rarity::Epic), Some(Rarity::Epic));

        let ruins = location("ancient_ruins").expect("локация");
        assert_eq!(ruins.downgrade(Rarity::Legendary), Some(Rarity::Legendary));

        // В шахтах своя редкая строка — серебро, поэтому эпический лут
        // понижается до редкого.
        let mines = location("deep_mines").expect("локация");
        assert_eq!(mines.downgrade(Rarity::Epic), Some(Rarity::Rare));
    }

    #[test]
    fn field_drops_only_its_own_loot() {
        let mut state = endless_state();
        let mut found = 0;

        for seed in 0..2_000u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "field", 60).expect("поход");
            for loot in &report.loot {
                found += 1;
                assert!(
                    matches!(
                        loot.item_id.as_str(),
                        "wood" | "clay" | "fiber" | "map_forest"
                    ),
                    "неожиданный дроп: {}",
                    loot.item_id
                );
            }
            state.active_resources.clear();
            state.inventory.clear();
            state.unlocked_locations.clear();
        }

        assert!(found > 0, "за 2000 походов не нашлось ни одной добычи");
    }

    #[test]
    fn map_drops_only_in_current_location() {
        let mut state = endless_state();
        state.unlock_location("deep_forest");
        // Текущая локация — лес, поэтому поле карт не сыплет.
        let mut any_map = false;
        for seed in 0..2_000u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "field", 60).expect("поход");
            if report.loot.iter().any(|loot| loot.item_id == "map_forest") {
                any_map = true;
            }
            state.active_resources.clear();
            state.inventory.clear();
        }
        assert!(!any_map, "карта выпала не в текущей локации");
    }

    #[test]
    fn quarry_map_drops_but_opens_only_when_studied() {
        // Карта — редкая находка: меряем средний интервал до первой карты,
        // а не каждый отдельный бросок.
        let mut total = 0.0;
        let mut found = 0;
        for seed in 0..300u64 {
            let mut state = endlessly_rich();
            state.unlock_location("deep_mines"); // ← добавлено
            state.market_seed = seed;
            // Шахты — текущая локация цепочки: карта каменоломни падает только там.
            for second in 1..=20_000u32 {
                let tick = step_expedition(&mut state, "deep_mines", 1.0);
                assert!(!tick.is_over(), "поход прервался на {second} с");
                if tick.loot.iter().any(|loot| loot.item_id == "map_quarry") {
                    found += 1;
                    total += second as f64;
                    assert!(state.item_count("map_quarry") >= 1);
                    assert!(
                        !state.location_unlocked("forgotten_quarry"),
                        "карта не должна открывать локацию сама"
                    );
                    // Изучение тратит одну карту и открывает каменоломню.
                    let opened =
                        crate::game::actions::study_map(&mut state, "map_quarry").expect("карта");
                    assert_eq!(opened, "forgotten_quarry");
                    assert!(state.location_unlocked("forgotten_quarry"));
                    assert!(!state.location_unlocked("ancient_ruins"));
                    break;
                }
                // Рюкзак не должен переполняться и рвать долгий замер,
                // но свечи нужны на весь путь.
                state.inventory.retain(|item_id, _| item_id == "candle");
            }
        }

        assert!(found > 0, "карта каменоломни ни разу не выпала");
        let average = total / found as f64;
        // По концепту карта — примерно раз в 1400 секунд пути.
        assert!(
            (1_000.0..=2_000.0).contains(&average),
            "карта падает не по концепту: раз в {average:.0} с"
        );
    }

    #[test]
    fn expedition_loot_is_added_to_state() {
        let mut state = endless_state();
        let mut found = false;

        for seed in 0..500u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "ravine", 60).expect("поход");
            for loot in report.loot {
                found = true;
                if loot.active {
                    assert!(state.active_amount(&loot.item_id) >= loot.quantity);
                } else if loot.item_id != "map_forest" && loot.item_id != "map_mines" {
                    assert!(state.item_count(&loot.item_id) >= loot.quantity);
                }
            }
            state.active_resources.clear();
            state.inventory.clear();
        }

        assert!(found);
    }

    #[test]
    fn ruins_drop_steam_blueprint() {
        let mut state = endless_state();
        state.add_item("candle", 10_000);
        state.unlock_location("ancient_ruins");
        let mut found = false;

        for seed in 0..500u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "ancient_ruins", 200).expect("поход");
            if report
                .loot
                .iter()
                .any(|loot| loot.item_id == "steam_blueprint")
            {
                found = true;
                break;
            }
            state.active_resources.clear();
            state.inventory.clear();
        }

        assert!(found, "чертёж паровой машины не выпал");
    }

    #[test]
    fn low_energy_closes_active_expedition() {
        use crate::game::state::ActiveExpedition;

        let mut state = GameState {
            energy: 3.0,
            ..GameState::default()
        };
        state.active_expedition = Some(ActiveExpedition::new("field"));

        assert!(tick_active_expedition(&mut state, 1.0));

        let active = state.active_expedition.as_ref().expect("поход на месте");
        assert!(active.done, "поход должен закрыться при малой энергии");
        assert_eq!(state.energy, 3.0, "энергия не должна тратиться");
        assert!(active.notice.is_some());
    }

    #[test]
    fn candle_burns_out_and_stops_the_expedition() {
        let mut state = GameState {
            energy: 1_000_000.0,
            ..GameState::default()
        };
        state.add_item("candle", 2);
        state.unlock_location("deep_mines");

        // 200 секунд = ровно одна свеча. Рюкзак может заполниться раньше,
        // поэтому оставляем в нём только свечи.
        for _ in 0..balance::CANDLE_SECONDS_PER_ITEM {
            let tick = step_expedition(&mut state, "deep_mines", 1.0);
            assert!(!tick.is_over());
            state.inventory.retain(|item_id, _| item_id == "candle");
        }
        assert_eq!(state.item_count("candle"), 1);

        // Вторая свеча догорает на следующих 200 секундах, третьей нет.
        for _ in 0..balance::CANDLE_SECONDS_PER_ITEM {
            let tick = step_expedition(&mut state, "deep_mines", 1.0);
            assert!(!tick.is_over());
            state.inventory.retain(|item_id, _| item_id == "candle");
        }
        assert_eq!(state.item_count("candle"), 0);

        // Свеча кончилась — поход не продолжается.
        let tick = step_expedition(&mut state, "deep_mines", 1.0);
        assert!(tick.out_of_candle, "без свечи поход должен прерваться");
        assert!(tick.is_over());
    }

    #[test]
    fn candle_is_not_spent_in_light_locations() {
        let mut state = endless_state();
        state.add_item("candle", 1);
        for _ in 0..500 {
            let tick = step_expedition(&mut state, "field", 1.0);
            assert!(!tick.is_over());
        }
        assert_eq!(state.item_count("candle"), 1);
    }

    #[test]
    fn finished_expedition_does_not_restart() {
        use crate::game::state::ActiveExpedition;

        let mut state = GameState {
            energy: 3.0,
            ..GameState::default()
        };
        state.active_expedition = Some(ActiveExpedition::new("field"));

        assert!(tick_active_expedition(&mut state, 1.0));
        let found_after_stop = state
            .active_expedition
            .as_ref()
            .map(|active| active.found.len())
            .unwrap_or(0);

        // Энергия восстановилась — поход не должен оживать.
        state.energy = 500.0;
        for _ in 0..30 {
            assert!(!tick_active_expedition(&mut state, 1.0));
        }

        assert_eq!(state.energy, 500.0, "законченный поход не тратит энергию");
        let active = state.active_expedition.as_ref().expect("поход на месте");
        assert!(active.done);
        assert_eq!(active.found.len(), found_after_stop);
        assert_eq!(active.elapsed, 0);
    }
}
