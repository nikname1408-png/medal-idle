//! Числа баланса. Всё, что можно «покрутить», живёт здесь.
//!
//! Часть констант заготовлена под следующие версии, поэтому модуль временно
//! разрешает неиспользуемые элементы.
#![allow(dead_code)]

/// Версия формата сохранения. Меняется при несовместимых правках.
pub const SAVE_VERSION: u32 = 8;

/// Стартовый капитал новой игры. Ноль: всё добывается и зарабатывается.
pub const START_GOLD: f64 = 0.0;

/// Энергия в начале игры. Капать сама она не будет — только отдых и дом.
pub const START_ENERGY: f64 = 0.0;
/// Максимальный запас энергии в начале.
pub const START_ENERGY_MAX: f64 = 100.0;
/// Длительность одного игрового тика в миллисекундах.
pub const TICK_MS: u64 = 1000;

/// Сколько энергии даёт одно нажатие «Отдых» без дома.
pub const REST_ENERGY: f64 = 1.0;
/// Насколько «Отдых» становится сильнее за каждый уровень дома.
pub const REST_ENERGY_PER_HOUSE_LEVEL: f64 = 0.5;
/// Сколько раз нужно отдохнуть, чтобы открыть добычу ресурсов.
pub const REST_CLICKS_TO_UNLOCK_GATHER: u32 = 10;

/// Энергия, которую тратит один клик добычи.
pub const CLICK_ENERGY_COST: f64 = 1.0;
/// Сколько ресурса даёт один клик.
pub const CLICK_YIELD: f64 = 1.0;

/// После первого дерева в баре появляется значок дерева.
pub const WOOD_VISIBLE_FROM: f64 = 1.0;
/// После двух деревьев открываются камень, дом и склад.
pub const STONE_UNLOCK_WOOD: f64 = 2.0;

/// Базовый лимит ресурсов без склада.
pub const BASE_STORAGE_CAP: f64 = 100.0;
/// Прирост лимита ресурсов за уровень склада.
pub const STORAGE_CAP_PER_LEVEL: f64 = 150.0;

/// Прирост регенерации энергии за уровень дома.
pub const HOUSE_ENERGY_PER_LEVEL: f64 = 0.5;

/// Прирост максимума энергии за уровень «Уюта».
pub const ENERGY_CAP_PER_COMFORT: f64 = 100.0;

/// Стоимость постройки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub wood: f64,
    pub stone: f64,
    /// Доплата золотом (часть построек требует монет).
    pub gold: f64,
}

/// Стоимость следующего уровня дома.
pub fn house_cost(level: u32) -> Cost {
    let factor = HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (HOUSE_BASE_COST_STONE * factor).ceil(),
        gold: (HOUSE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня склада.
pub fn storage_cost(level: u32) -> Cost {
    let factor = STORAGE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (STORAGE_BASE_COST_WOOD * factor).ceil(),
        stone: (STORAGE_BASE_COST_STONE * factor).ceil(),
        gold: (STORAGE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня «Уюта».
pub fn comfort_cost(level: u32) -> Cost {
    let factor = COMFORT_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (COMFORT_BASE_COST_WOOD * factor).ceil(),
        stone: (COMFORT_BASE_COST_STONE * factor).ceil(),
        gold: (COMFORT_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня дома для работников.
pub fn worker_house_cost(level: u32) -> Cost {
    let factor = WORKER_HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (WORKER_HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (WORKER_HOUSE_BASE_COST_STONE * factor).ceil(),
        gold: (WORKER_HOUSE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня дома для изучения.
pub fn research_house_cost(level: u32) -> Cost {
    let factor = RESEARCH_HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (RESEARCH_HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (RESEARCH_HOUSE_BASE_COST_STONE * factor).ceil(),
        gold: (RESEARCH_HOUSE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня лесопилки.
pub fn sawmill_cost(level: u32) -> Cost {
    let factor = SAWMILL_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (SAWMILL_BASE_COST_WOOD * factor).ceil(),
        stone: (SAWMILL_BASE_COST_STONE * factor).ceil(),
        gold: (SAWMILL_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня угольной ямы.
pub fn coal_pit_cost(level: u32) -> Cost {
    let factor = COAL_PIT_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (COAL_PIT_BASE_COST_WOOD * factor).ceil(),
        stone: (COAL_PIT_BASE_COST_STONE * factor).ceil(),
        gold: (COAL_PIT_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня кузницы.
pub fn forge_cost(level: u32) -> Cost {
    let factor = FORGE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (FORGE_BASE_COST_WOOD * factor).ceil(),
        stone: (FORGE_BASE_COST_STONE * factor).ceil(),
        gold: (FORGE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня шахты.
pub fn mine_cost(level: u32) -> Cost {
    let factor = MINE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (MINE_BASE_COST_WOOD * factor).ceil(),
        stone: (MINE_BASE_COST_STONE * factor).ceil(),
        gold: (MINE_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Стоимость следующего уровня банка.
pub fn bank_cost(level: u32) -> Cost {
    let factor = BANK_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (BANK_BASE_COST_WOOD * factor).ceil(),
        stone: (BANK_BASE_COST_STONE * factor).ceil(),
        gold: (BANK_BASE_COST_GOLD * factor).ceil(),
    }
}

/// Сколько работников даёт один уровень дома для работников.
pub const WORKERS_PER_HOUSE_LEVEL: u32 = 1;
/// Сколько исследователей даёт один уровень дома для изучения.
pub const RESEARCHERS_PER_HOUSE_LEVEL: u32 = 1;

/// Сколько камня в секунду приносит один работник лесопилки.
pub const SAWMILL_WOOD_PER_WORKER_PER_SEC: f64 = 0.5;

/// Сколько опыта нужно, чтобы изучить угольную яму.
pub const COAL_PIT_RESEARCH_COST: f64 = 60.0;
/// Сколько опыта в секунду даёт один исследователь.
pub const RESEARCH_PER_RESEARCHER_PER_SEC: f64 = 0.75;

/// Сколько дерева тратит одна партия угольной ямы.
pub const COAL_PIT_WOOD_PER_BATCH: f64 = 1.0;
/// Сколько угля даёт одна партия угольной ямы.
pub const COAL_PIT_COAL_PER_BATCH: f64 = 1.0;
/// За сколько секунд один работник делает одну партию.
pub const COAL_PIT_BATCH_SECONDS: f64 = 5.0;

/// Сколько опыта нужно, чтобы изучить кузницу.
pub const FORGE_RESEARCH_COST: f64 = 120.0;
/// Сколько опыта нужно, чтобы изучить банк (открывает банк после кузницы).
pub const BANK_RESEARCH_COST: f64 = 200.0;

/// Максимальный запас огня кузницы без амулета.
pub const FORGE_FIRE_MAX_SECONDS: u32 = 60;
/// Сколько секунд огня даёт один уголь.
pub const FIRE_SECONDS_PER_COAL: u32 = 20;
/// Прибавка к огню с одного угля за каждый уровень кузницы выше первого.
pub const FIRE_PER_COAL_PER_LEVEL: u32 = 10;
/// Прибавка к запасу огня за каждый уровень кузницы выше первого.
pub const FORGE_FIRE_PER_LEVEL: u32 = 50;

/// Сколько камня в секунду даёт один работник шахты.
pub const MINE_STONE_PER_WORKER_PER_SEC: f64 = 0.5;

/// Базовый лимит золота без банка.
pub const BASE_GOLD_CAP: f64 = 1_000.0;
/// Прирост лимита золота за уровень банка.
pub const GOLD_CAP_PER_LEVEL: f64 = 1_000.0;

/// Постоянный бонус к производству за каждый пройденный цикл.
pub const CYCLE_BONUS_PER_CYCLE: f64 = 0.10;
/// Сколько железа нужно, чтобы начать новый цикл.
pub const CYCLE_IRON_REQUIREMENT: f64 = 25.0;

/// Сколько секунд оффлайна дают одну единицу времени.
pub const OFFLINE_SECONDS_PER_TIME: f64 = 60.0;
/// Максимум оффлайн-времени, которое учитывается.
pub const OFFLINE_MAX_SECONDS: f64 = 8.0 * 3600.0;
/// Потолок ресурса «время».
pub const TIME_CAP: f64 = 480.0;
/// Сколько секунд игры даёт один тайм-варп.
pub const TIME_WARP_SECONDS: f64 = 120.0;
/// Сколько опыта изучения даёт один тайм-варп в изучениях.
pub const RESEARCH_TIME_WARP: f64 = 15.0;

/// Сколько энергии тратит один поход.
pub const EXPEDITION_ENERGY_COST: f64 = 10.0;

/// Шанс найти лут за один тик пути (1 с), %.
pub const EXPEDITION_LOOT_CHANCE: u32 = 15;
/// Шансы редкости лута при сработавшем броске, в промилле (0..1000).
/// Обычный 70%, необычный 20%, редкий 8%, эпический 1.9%, легендарный 0.1%.
pub const EXPEDITION_COMMON_WEIGHT: u32 = 700;
pub const EXPEDITION_UNCOMMON_WEIGHT: u32 = 200;
pub const EXPEDITION_RARE_WEIGHT: u32 = 80;
pub const EXPEDITION_EPIC_WEIGHT: u32 = 19;
pub const EXPEDITION_LEGENDARY_WEIGHT: u32 = 1;

/// Отдельная проверка чертежа паровой машины в Древних руинах: 1% в секунду.
pub const EXPEDITION_STEAM_BLUEPRINT_CHANCE: u32 = 1;

/// Какая доля редких находок в текущей локации превращается в карту, %.
///
/// Карта не должна сыпаться пачками: при 15% лута, 8% редкого и 4% здесь
/// выходит одна карта примерно раз в 2000 секунд пути.
pub const EXPEDITION_MAP_CHANCE: u32 = 4;

/// Стоимость предметов в кузнице.
pub const ITEM_SWORD_IRON: f64 = 5.0;
pub const ITEM_SWORD_COAL: f64 = 0.0;
pub const ITEM_ARMOR_IRON: f64 = 8.0;
pub const ITEM_ARMOR_COAL: f64 = 0.0;
pub const ITEM_AMULET_IRON: f64 = 3.0;
pub const ITEM_AMULET_COAL: f64 = 5.0;
/// Сколько секунд огня тратит создание предмета.
pub const ITEM_SWORD_FIRE: f64 = 10.0;
pub const ITEM_ARMOR_FIRE: f64 = 15.0;
pub const ITEM_AMULET_FIRE: f64 = 5.0;

/// Бонусы экипировки.
pub const ARMOR_ENERGY_BONUS: f64 = 25.0;
pub const SWORD_GATHER_BONUS: f64 = 1.0;
pub const AMULET_FIRE_BONUS: f64 = 15.0;

/// Курсы торговца.
pub const TRADE_WOOD_AMOUNT: f64 = 20.0;
pub const TRADE_STONE_AMOUNT: f64 = 10.0;
pub const TRADE_COAL_AMOUNT: f64 = 2.0;
pub const TRADE_GOLD_PER_SALE: f64 = 1.0;
pub const TRADE_BUY_COAL_GOLD: f64 = 2.0;
pub const TRADE_BUY_COAL_AMOUNT: f64 = 5.0;
pub const TRADE_BUY_IRON_GOLD: f64 = 3.0;
pub const TRADE_BUY_IRON_AMOUNT: f64 = 2.0;

const HOUSE_BASE_COST_WOOD: f64 = 15.0;
const HOUSE_BASE_COST_GOLD: f64 = 0.0;
const HOUSE_BASE_COST_STONE: f64 = 8.0;
const HOUSE_COST_GROWTH: f64 = 2.2;

const STORAGE_BASE_COST_WOOD: f64 = 30.0;
const STORAGE_BASE_COST_GOLD: f64 = 0.0;
const STORAGE_BASE_COST_STONE: f64 = 15.0;
const STORAGE_COST_GROWTH: f64 = 2.2;

const COMFORT_BASE_COST_WOOD: f64 = 40.0;
const COMFORT_BASE_COST_GOLD: f64 = 0.0;
const COMFORT_BASE_COST_STONE: f64 = 25.0;
const COMFORT_COST_GROWTH: f64 = 2.2;

const WORKER_HOUSE_BASE_COST_WOOD: f64 = 18.0;
const WORKER_HOUSE_BASE_COST_GOLD: f64 = 0.0;
const WORKER_HOUSE_BASE_COST_STONE: f64 = 12.0;
const WORKER_HOUSE_COST_GROWTH: f64 = 2.2;

const RESEARCH_HOUSE_BASE_COST_WOOD: f64 = 24.0;
const RESEARCH_HOUSE_BASE_COST_GOLD: f64 = 0.0;
const RESEARCH_HOUSE_BASE_COST_STONE: f64 = 15.0;
const RESEARCH_HOUSE_COST_GROWTH: f64 = 2.2;

const SAWMILL_BASE_COST_WOOD: f64 = 24.0;
const SAWMILL_BASE_COST_GOLD: f64 = 0.0;
const SAWMILL_BASE_COST_STONE: f64 = 12.0;
const SAWMILL_COST_GROWTH: f64 = 2.2;

const COAL_PIT_BASE_COST_WOOD: f64 = 30.0;
const COAL_PIT_BASE_COST_GOLD: f64 = 60.0;
const COAL_PIT_BASE_COST_STONE: f64 = 18.0;
const COAL_PIT_COST_GROWTH: f64 = 2.2;

const FORGE_BASE_COST_WOOD: f64 = 60.0;
const FORGE_BASE_COST_GOLD: f64 = 120.0;
const FORGE_BASE_COST_STONE: f64 = 40.0;
const FORGE_COST_GROWTH: f64 = 2.2;

const MINE_BASE_COST_WOOD: f64 = 45.0;
const MINE_BASE_COST_GOLD: f64 = 0.0;
const MINE_BASE_COST_STONE: f64 = 30.0;
const MINE_COST_GROWTH: f64 = 2.2;

const BANK_BASE_COST_WOOD: f64 = 80.0;
const BANK_BASE_COST_GOLD: f64 = 200.0;
const BANK_BASE_COST_STONE: f64 = 60.0;
const BANK_COST_GROWTH: f64 = 2.2;
