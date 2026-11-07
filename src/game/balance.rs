//! Числа баланса. Всё, что можно «покрутить», живёт здесь.
//!
//! Часть констант заготовлена под следующие версии, поэтому модуль временно
//! разрешает неиспользуемые элементы.
#![allow(dead_code)]

/// Версия формата сохранения. Меняется при несовместимых правках.
pub const SAVE_VERSION: u32 = 2;

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

/// Стоимость постройки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub wood: f64,
    pub stone: f64,
}

/// Стоимость следующего уровня дома.
pub fn house_cost(level: u32) -> Cost {
    let factor = HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (HOUSE_BASE_COST_STONE * factor).ceil(),
    }
}

/// Стоимость следующего уровня склада.
pub fn storage_cost(level: u32) -> Cost {
    let factor = STORAGE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (STORAGE_BASE_COST_WOOD * factor).ceil(),
        stone: (STORAGE_BASE_COST_STONE * factor).ceil(),
    }
}

/// Стоимость следующего уровня дома для работников.
pub fn worker_house_cost(level: u32) -> Cost {
    let factor = WORKER_HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (WORKER_HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (WORKER_HOUSE_BASE_COST_STONE * factor).ceil(),
    }
}

/// Стоимость следующего уровня дома для изучения.
pub fn research_house_cost(level: u32) -> Cost {
    let factor = RESEARCH_HOUSE_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (RESEARCH_HOUSE_BASE_COST_WOOD * factor).ceil(),
        stone: (RESEARCH_HOUSE_BASE_COST_STONE * factor).ceil(),
    }
}

/// Стоимость следующего уровня лесопилки.
pub fn sawmill_cost(level: u32) -> Cost {
    let factor = SAWMILL_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (SAWMILL_BASE_COST_WOOD * factor).ceil(),
        stone: (SAWMILL_BASE_COST_STONE * factor).ceil(),
    }
}

/// Стоимость следующего уровня угольной ямы.
pub fn coal_pit_cost(level: u32) -> Cost {
    let factor = COAL_PIT_COST_GROWTH.powi(level as i32);
    Cost {
        wood: (COAL_PIT_BASE_COST_WOOD * factor).ceil(),
        stone: (COAL_PIT_BASE_COST_STONE * factor).ceil(),
    }
}

/// Сколько работников даёт один уровень дома для работников.
pub const WORKERS_PER_HOUSE_LEVEL: u32 = 1;
/// Сколько исследователей даёт один уровень дома для изучения.
pub const RESEARCHERS_PER_HOUSE_LEVEL: u32 = 1;

/// Сколько дерева в секунду приносит один работник лесопилки.
pub const SAWMILL_WOOD_PER_WORKER_PER_SEC: f64 = 0.4;

/// Сколько опыта нужно, чтобы изучить угольную яму.
pub const COAL_PIT_RESEARCH_COST: f64 = 45.0;
/// Сколько опыта в секунду даёт один исследователь.
pub const RESEARCH_PER_RESEARCHER_PER_SEC: f64 = 0.75;

/// Сколько дерева тратит одна партия угольной ямы.
pub const COAL_PIT_WOOD_PER_BATCH: f64 = 1.0;
/// Сколько угля даёт одна партия угольной ямы.
pub const COAL_PIT_COAL_PER_BATCH: f64 = 1.0;
/// За сколько секунд один работник делает одну партию.
pub const COAL_PIT_BATCH_SECONDS: f64 = 5.0;

const HOUSE_BASE_COST_WOOD: f64 = 15.0;
const HOUSE_BASE_COST_STONE: f64 = 8.0;
const HOUSE_COST_GROWTH: f64 = 1.9;

const STORAGE_BASE_COST_WOOD: f64 = 30.0;
const STORAGE_BASE_COST_STONE: f64 = 15.0;
const STORAGE_COST_GROWTH: f64 = 1.9;

const WORKER_HOUSE_BASE_COST_WOOD: f64 = 18.0;
const WORKER_HOUSE_BASE_COST_STONE: f64 = 12.0;
const WORKER_HOUSE_COST_GROWTH: f64 = 1.9;

const RESEARCH_HOUSE_BASE_COST_WOOD: f64 = 24.0;
const RESEARCH_HOUSE_BASE_COST_STONE: f64 = 15.0;
const RESEARCH_HOUSE_COST_GROWTH: f64 = 1.9;

const SAWMILL_BASE_COST_WOOD: f64 = 24.0;
const SAWMILL_BASE_COST_STONE: f64 = 12.0;
const SAWMILL_COST_GROWTH: f64 = 1.9;

const COAL_PIT_BASE_COST_WOOD: f64 = 30.0;
const COAL_PIT_BASE_COST_STONE: f64 = 18.0;
const COAL_PIT_COST_GROWTH: f64 = 1.9;
