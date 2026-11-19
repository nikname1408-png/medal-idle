//! Числа баланса. Всё, что можно «покрутить», живёт здесь.
//!
//! Часть констант заготовлена под следующие версии, поэтому модуль временно
//! разрешает неиспользуемые элементы.
#![allow(dead_code)]

/// Версия формата сохранения. Меняется при несовместимых правках.
pub const SAVE_VERSION: u32 = 16;

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
pub const STORAGE_CAP_PER_LEVEL: f64 = 500.0;
/// Прирост лимита ресурсов за уровень амбара.
pub const BARN_CAP_PER_LEVEL: f64 = 1_000.0;
/// Уровень склада, на котором открывается амбар.
pub const BARN_UNLOCK_STORAGE_LEVEL: u32 = 8;

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
    /// Энергия, которую тратит постройка.
    pub energy: f64,
    /// Доски (предмет `board`).
    pub boards: u32,
    /// Железные слитки (предмет `iron_ingot`).
    pub iron_ingots: u32,
}

/// Стоимость из дерева, камня и золота без предметов и энергии.
fn cost(wood: f64, stone: f64, gold: f64) -> Cost {
    Cost {
        wood,
        stone,
        gold,
        energy: 0.0,
        boards: 0,
        iron_ingots: 0,
    }
}

/// Стоимость следующего уровня дома.
pub fn house_cost(level: u32) -> Cost {
    let factor = HOUSE_COST_GROWTH.powi(level as i32);
    cost(
        (HOUSE_BASE_COST_WOOD * factor).ceil(),
        (HOUSE_BASE_COST_STONE * factor).ceil(),
        (HOUSE_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Стоимость следующего уровня склада.
pub fn storage_cost(level: u32) -> Cost {
    let factor = STORAGE_COST_GROWTH.powi(level as i32);
    cost(
        (STORAGE_BASE_COST_WOOD * factor).ceil(),
        (STORAGE_BASE_COST_STONE * factor).ceil(),
        (STORAGE_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Стоимость следующего уровня амбара. Дерево и доски дорожают медленнее
/// золота и не упираются в лимиты хранения.
pub fn barn_cost(level: u32) -> Cost {
    let factor = BARN_COST_GROWTH.powi(level as i32);
    let wood_factor = BARN_COST_GROWTH_WOOD.powi(level as i32);
    Cost {
        boards: ((BARN_BASE_COST_BOARDS as f64 * wood_factor).ceil() as u32)
            .max(BARN_BASE_COST_BOARDS),
        ..cost(
            (BARN_BASE_COST_WOOD * wood_factor).ceil(),
            0.0,
            (BARN_BASE_COST_GOLD * factor).ceil(),
        )
    }
}

/// Стоимость следующего уровня «Уюта».
pub fn comfort_cost(level: u32) -> Cost {
    let factor = COMFORT_COST_GROWTH.powi(level as i32);
    cost(
        (COMFORT_BASE_COST_WOOD * factor).ceil(),
        (COMFORT_BASE_COST_STONE * factor).ceil(),
        (COMFORT_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Стоимость следующего уровня дома для работников.
pub fn worker_house_cost(level: u32) -> Cost {
    let factor = WORKER_HOUSE_COST_GROWTH.powi(level as i32);
    cost(
        (WORKER_HOUSE_BASE_COST_WOOD * factor).ceil(),
        (WORKER_HOUSE_BASE_COST_STONE * factor).ceil(),
        (WORKER_HOUSE_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Стоимость следующего уровня дома для изучения (легаси, UI удалён).
pub fn research_house_cost(level: u32) -> Cost {
    let factor = RESEARCH_HOUSE_COST_GROWTH.powi(level as i32);
    cost(
        (RESEARCH_HOUSE_BASE_COST_WOOD * factor).ceil(),
        (RESEARCH_HOUSE_BASE_COST_STONE * factor).ceil(),
        (RESEARCH_HOUSE_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Стоимость следующего уровня лесопилки.
pub fn sawmill_cost(level: u32) -> Cost {
    let factor = SAWMILL_COST_GROWTH.powi(level as i32);
    cost(
        (SAWMILL_BASE_COST_WOOD * factor).ceil(),
        (SAWMILL_BASE_COST_STONE * factor).ceil(),
        (SAWMILL_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Цена постройки: нулевой уровень — разовая цена открытия (золото), дальше —
/// цена улучшения, которая растёт от базовой.
fn unlock_then_upgrade(wood: f64, stone: f64, growth: f64, level: u32) -> Cost {
    if level == 0 {
        return cost(wood, stone, 0.0);
    }
    let factor = growth.powi(level as i32 - 1);
    cost((wood * factor).ceil(), (stone * factor).ceil(), 0.0)
}

/// Открытие постройки за золото, затем улучшения за дерево и камень.
///
/// Открытие — это разблокировка кнопки постройки: пока золото не уплачено,
/// кнопки нет, а после оплаты постройка появляется без уровней и её можно
/// улучшать за дерево и камень.
fn unlock_with_gold(gold: f64, wood: f64, stone: f64, growth: f64, level: u32) -> Cost {
    if level == 0 {
        return cost(0.0, 0.0, gold);
    }
    let factor = growth.powi(level as i32 - 1);
    cost((wood * factor).ceil(), (stone * factor).ceil(), 0.0)
}

/// Угольная яма.
///
/// - `0` — открытие (письмо): 300 золота.
/// - `1` — постройка 0→1 («Поле»): 180 дерева, 100 камня.
/// - `2+` — улучшения: старые цены (60×1.8^(n−2) дерева, 40×1.8^(n−2) камня).
pub fn coal_pit_cost(level: u32) -> Cost {
    match level {
        0 => Cost {
            wood: 0.0,
            stone: 0.0,
            gold: COAL_PIT_UNLOCK_GOLD,
            energy: 0.0,
            boards: 0,
            iron_ingots: 0,
        },
        1 => Cost {
            wood: COAL_PIT_FIRST_WOOD,
            stone: COAL_PIT_FIRST_STONE,
            gold: 0.0,
            energy: 0.0,
            boards: 0,
            iron_ingots: 0,
        },
        _ => {
            let factor = COAL_PIT_COST_GROWTH.powi(level as i32 - 2);
            cost(
                (COAL_PIT_UPGRADE_WOOD * factor).ceil(),
                (COAL_PIT_UPGRADE_STONE * factor).ceil(),
                0.0,
            )
        }
    }
}

/// Кузница.
///
/// - `0` — открытие (письмо): 200 дерева, 100 камня, 300 энергии, 500 золота.
/// - `1` — постройка 0→1 («Поле»): 300 дерева, 250 камня.
/// - `2+` — улучшения: старые цены (150×1.8^(n−2) дерева, 100×1.8^(n−2) камня).
pub fn forge_cost(level: u32) -> Cost {
    match level {
        0 => Cost {
            wood: FORGE_UNLOCK_WOOD,
            stone: FORGE_UNLOCK_STONE,
            gold: FORGE_UNLOCK_GOLD,
            energy: FORGE_UNLOCK_ENERGY,
            boards: 0,
            iron_ingots: 0,
        },
        1 => Cost {
            wood: FORGE_FIRST_WOOD,
            stone: FORGE_FIRST_STONE,
            gold: 0.0,
            energy: 0.0,
            boards: 0,
            iron_ingots: 0,
        },
        _ => {
            let factor = FORGE_COST_GROWTH.powi(level as i32 - 2);
            cost(
                (FORGE_UPGRADE_WOOD * factor).ceil(),
                (FORGE_UPGRADE_STONE * factor).ceil(),
                0.0,
            )
        }
    }
}

/// Стоимость следующего уровня шахты.
pub fn mine_cost(level: u32) -> Cost {
    let factor = MINE_COST_GROWTH.powi(level as i32);
    cost(
        (MINE_BASE_COST_WOOD * factor).ceil(),
        (MINE_BASE_COST_STONE * factor).ceil(),
        (MINE_BASE_COST_GOLD * factor).ceil(),
    )
}

/// Банк.
///
/// - `0` — открытие (письмо): 25 железных слитков, 500 золота.
/// - `1` — постройка 0→1 («Поле»): 300 дерева, 200 камня, 300 золота.
/// - `2+` — улучшения: старые цены (200×1.8^(n−2) дерева, 150×1.8^(n−2) камня).
pub fn bank_cost(level: u32) -> Cost {
    match level {
        0 => Cost {
            wood: 0.0,
            stone: 0.0,
            gold: BANK_UNLOCK_GOLD,
            energy: 0.0,
            boards: 0,
            iron_ingots: BANK_UNLOCK_IRON_INGOTS,
        },
        1 => Cost {
            wood: BANK_FIRST_WOOD,
            stone: BANK_FIRST_STONE,
            gold: BANK_FIRST_GOLD,
            energy: 0.0,
            boards: 0,
            iron_ingots: 0,
        },
        _ => {
            let factor = BANK_COST_GROWTH.powi(level as i32 - 2);
            cost(
                (BANK_UPGRADE_WOOD * factor).ceil(),
                (BANK_UPGRADE_STONE * factor).ceil(),
                0.0,
            )
        }
    }
}

// ---------- События ----------
/// Первое фоновое событие в новой игре (5 минут).
pub const EVENTS_BASELINE_FIRST_SECS: f64 = 5.0 * 60.0;
/// Минимальный интервал между фоновыми событиями.
pub const EVENTS_BASELINE_MIN_SECS: f64 = 15.0 * 60.0;
/// Максимальный интервал между фоновыми событиями.
pub const EVENTS_BASELINE_MAX_SECS: f64 = 20.0 * 60.0;
/// Шанс, что фоновое событие вообще сработает (в процентах).
pub const EVENTS_BASELINE_CHANCE: u64 = 70;

/// Минимальная сумма сделки оптовика, чтобы предложение имело смысл.
pub const EVENTS_BULK_MIN_GOLD: u32 = 50;

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
pub const FORGE_FIRE_MAX_SECONDS: u32 = 67;
/// Сколько секунд огня даёт один уголь.
pub const FIRE_SECONDS_PER_COAL: u32 = 20;
/// Прибавка к огню с одного угля за каждый уровень кузницы выше первого.
pub const FIRE_PER_COAL_PER_LEVEL: u32 = 10;
/// Прибавка к запасу огня за каждый уровень кузницы выше первого.
pub const FORGE_FIRE_PER_LEVEL: u32 = 67;
/// Как часто кнопка «Авто» подбрасывает один уголь.
pub const FORGE_AUTO_FEED_SECONDS: f64 = 2.0;

/// Сколько камня в секунду даёт один работник шахты.
pub const MINE_STONE_PER_WORKER_PER_SEC: f64 = 0.5;

/// Ниже этой энергии поход автоматически закрывается.
pub const EXPEDITION_MIN_ENERGY: f64 = 5.0;

/// Порог золота, на котором появляется предложение построить банк.
pub const BANK_OFFER_GOLD: f64 = 800.0;

/// Базовый лимит золота без банка.
pub const BASE_GOLD_CAP: f64 = 1_000.0;
/// Прирост лимита золота за уровень банка.
pub const GOLD_CAP_PER_LEVEL: f64 = 1_000.0;

/// Базовый порог перерождения (R = 0) в единицах дерева и камня за цикл.
pub const CYCLE_THRESHOLD_BASE: f64 = 6_000.0;
/// Номер цикла, на котором стоит стена «постройка до уровня».
pub const CYCLE_WALL_LEVEL: u32 = 5;
/// Уровень шахты, который требуется на стене R = 5.
pub const CYCLE_WALL_MINE_LEVEL: u32 = 3;
/// Номер цикла, на котором стоит стена «добыть камень сверх порога».
pub const CYCLE_WALL_STONE_LEVEL: u32 = 10;
/// Сколько камня нужно добыть за цикл сверх порога на стене R = 10.
pub const CYCLE_WALL_EXTRA_STONE: f64 = 5_000.0;

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

/// Отдельная проверка чертежа паровой машины в Древних руинах.
///
/// Один бросок из двух тысяч за секунду пути: в среднем одна находка
/// примерно за 2000 секунд похода.
pub const EXPEDITION_STEAM_BLUEPRINT_ROLL: u32 = 2_000;

/// Огонь кузницы, который сжигает сборка прототипа паровой машины.
pub const STEAM_PROTOTYPE_FIRE: u32 = 650;

/// Прибавка к множителю дохода после вступления в Эпоху Пара.
pub const STEAM_ERA_INCOME_BONUS: f64 = 0.5;

/// Сколько секунд похода в тёмной локации даёт одна свеча.
pub const CANDLE_SECONDS_PER_ITEM: u32 = 200;

/// Какая доля редких находок в текущей локации превращается в карту, %.
///
/// Карта не должна сыпаться пачками, но и ждать её слишком долго нельзя:
/// при 15% лута, 8% редкого и 6% здесь выходит одна карта примерно раз
/// в 1400 секунд пути.
pub const EXPEDITION_MAP_CHANCE: u32 = 6;

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
const HOUSE_COST_GROWTH: f64 = 1.7;

const STORAGE_BASE_COST_WOOD: f64 = 30.0;
const STORAGE_BASE_COST_GOLD: f64 = 0.0;
const STORAGE_BASE_COST_STONE: f64 = 15.0;
/// Рост цены склада мягче остальных построек: цена уровня должна
/// оставаться ниже лимита хранения, иначе улучшение невозможно купить.
const STORAGE_COST_GROWTH: f64 = 1.45;

pub const BARN_BASE_COST_WOOD: f64 = 300.0;
pub const BARN_BASE_COST_BOARDS: u32 = 50;
const BARN_BASE_COST_GOLD: f64 = 200.0;
const BARN_COST_GROWTH: f64 = 1.35;
/// Дерево и доски амбара дорожают медленнее золота.
pub const BARN_COST_GROWTH_WOOD: f64 = 1.15;

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
const SAWMILL_BASE_COST_GOLD: f64 = 60.0;
const SAWMILL_BASE_COST_STONE: f64 = 12.0;
const SAWMILL_COST_GROWTH: f64 = 2.2;

const COAL_PIT_UNLOCK_GOLD: f64 = 300.0;

const COAL_PIT_FIRST_WOOD: f64 = 180.0;
const COAL_PIT_FIRST_STONE: f64 = 100.0;

const COAL_PIT_UPGRADE_WOOD: f64 = 60.0;
const COAL_PIT_UPGRADE_STONE: f64 = 40.0;
const COAL_PIT_COST_GROWTH: f64 = 1.8;

const FORGE_UNLOCK_WOOD: f64 = 200.0;
const FORGE_UNLOCK_STONE: f64 = 100.0;
const FORGE_UNLOCK_ENERGY: f64 = 300.0;
const FORGE_UNLOCK_GOLD: f64 = 500.0;

const FORGE_FIRST_WOOD: f64 = 300.0;
const FORGE_FIRST_STONE: f64 = 250.0;

const FORGE_UPGRADE_WOOD: f64 = 150.0;
const FORGE_UPGRADE_STONE: f64 = 100.0;
const FORGE_COST_GROWTH: f64 = 1.55;

const MINE_BASE_COST_WOOD: f64 = 45.0;
const MINE_BASE_COST_GOLD: f64 = 80.0;
const MINE_BASE_COST_STONE: f64 = 30.0;
const MINE_COST_GROWTH: f64 = 2.2;

const BANK_UNLOCK_GOLD: f64 = 500.0;
const BANK_UNLOCK_IRON_INGOTS: u32 = 25;

const BANK_FIRST_WOOD: f64 = 300.0;
const BANK_FIRST_STONE: f64 = 200.0;
const BANK_FIRST_GOLD: f64 = 300.0;

const BANK_UPGRADE_WOOD: f64 = 200.0;
const BANK_UPGRADE_STONE: f64 = 150.0;
const BANK_COST_GROWTH: f64 = 1.8;

// ---------- Кризисы ----------
/// «Пожар»: [Залить камнем] тратит столько камня.
pub const CRISIS_FIRE_STONE: u32 = 100;
/// «Пожар»: [Потушить руками] тратит столько энергии.
pub const CRISIS_FIRE_ENERGY: u32 = 50;
/// «Пожар»: длительность дебаффа к регенерации после тушения руками.
pub const CRISIS_FIRE_DEBUFF_MIN_SECS: f64 = 180.0;
pub const CRISIS_FIRE_DEBUFF_MAX_SECS: f64 = 240.0;

/// «Воры»: [Проверить] теряет столько процентов дерева.
pub const CRISIS_THIEVES_WOOD_PERCENT: u32 = 20;
/// «Воры»: [Проверить] даёт столько железа.
pub const CRISIS_THIEVES_IRON: u32 = 10;
/// «Воры»: [Стража] стоит столько золота.
pub const CRISIS_THIEVES_GOLD: u32 = 500;

/// «Сломался инструмент»: [Починить] тратит столько руды.
pub const CRISIS_TOOL_IRON: u32 = 50;
/// «Сломался инструмент»: длительность дебаффа к шахте после «Сколотить».
pub const CRISIS_TOOL_DEBUFF_MIN_SECS: f64 = 120.0;
pub const CRISIS_TOOL_DEBUFF_MAX_SECS: f64 = 180.0;

/// Базовый опыт за один крафт (до прибавки за цену).
pub const SMITH_XP_PER_CRAFT_BASE: f64 = 1.5;
/// Делитель цены предмета для расчета бонусного опыта.
pub const SMITH_XP_PRICE_DIVISOR: f64 = 40.0;
/// Потолок опыта за один предмет (чтобы золотые кольца не давали по 50 опыта).
pub const SMITH_XP_CAP_PER_CRAFT: f64 = 10.0;
/// Множитель цены у торговца для неразблокированных предметов (в 5 раз дороже).
pub const LOCKED_ITEM_PRICE_MULT: f64 = 5.0;
/// Стартовый опыт кузнеца за каждый пройденный цикл после перерождения.
pub const SMITH_XP_PER_CYCLE: f64 = 5.0;
/// Опыт за собранный прототип паровой машины.
pub const SMITH_XP_STEAM_PROTOTYPE: f64 = 50.0;
/// Опыт за крафт досок (крафтятся пачками).
pub const SMITH_XP_BOARD: f64 = 0.5;
/// Опыт за крафт каменных блоков (камня нужно больше).
pub const SMITH_XP_STONE_BLOCK: f64 = 0.8;

/// Максимальный уровень кузнеца. Оставлен для тестов и старых сохранений:
/// прокачка на деле больше не ограничена этим потолком.
pub const SMITH_MAX_LEVEL: u32 = 20;

pub fn smith_xp_for_level(level: u32) -> f64 {
    let lvl = level.max(1) as f64;
    100.0 * lvl.powf(1.6)
}

/// Считает, сколько опыта игрок получит за крафт рецепта.
pub fn smith_xp_per_craft(recipe_id: &str, base_price: u32, quantity: u32) -> f64 {
    if recipe_id == "board" {
        return SMITH_XP_BOARD * quantity.max(1) as f64;
    }
    if recipe_id == "stone_block" {
        return SMITH_XP_STONE_BLOCK * quantity.max(1) as f64;
    }

    let bonus = (base_price as f64 / SMITH_XP_PRICE_DIVISOR)
        .min(SMITH_XP_CAP_PER_CRAFT - SMITH_XP_PER_CRAFT_BASE);

    (SMITH_XP_PER_CRAFT_BASE + bonus) * quantity.max(1) as f64
}

/// Доля ресурсов, которая возвращается при разборе предмета.
/// 0.5 — вернётся половина ингредиентов (кроме огня).
pub const DISMANTLE_REFUND: f64 = 0.5;

// ---------- Город ----------

/// Налог берётся раз в 30 минут игрового времени.
pub const CITY_TAX_INTERVAL: f64 = 30.0 * 60.0;
/// Доля выручки за период, которая уходит на налог.
pub const CITY_TAX_PERCENT: f64 = 0.15;
/// Стоимость добычи единицы ресурса: для налога с производства и пола цен.
/// Нули означают «не добывается линиями» — такие ресурсы считаются по рынку.
pub fn production_cost(item_id: &str) -> f64 {
    match item_id {
        "wood" | "stone" => 0.5,
        "coal" | "clay" => 1.0,
        _ => 0.0,
    }
}

/// Налог с производства: доля стоимости потока линий за цикл.
pub const CITY_PRODUCTION_TAX_PERCENT: f64 = 0.05;

/// Пол цены рецепта по тирам: к себестоимости ингредиентов и огня.
/// Материалы, простая одежда, среднее снаряжение, тяжёлое, эпик.
pub const PRICE_FLOOR_MATERIALS: f64 = 1.4;
pub const PRICE_FLOOR_CHEAP: f64 = 1.6;
pub const PRICE_FLOOR_MID: f64 = 1.9;
pub const PRICE_FLOOR_HEAVY: f64 = 2.3;
pub const PRICE_FLOOR_EPIC: f64 = 2.0;
/// Цена единицы огня в себестоимости крафта.
pub const FIRE_PRICE: f64 = 0.15;
/// Сколько минут до налога игрок получает письмо-предупреждение.
pub const CITY_TAX_WARNING_SECS: f64 = 5.0 * 60.0;
/// Штраф за пропущенный налог: рейтинг лавки.
pub const CITY_TAX_RATING_PENALTY: f32 = 1.0;
/// Штраф за переоткрытие закрытой лавки.
pub const CITY_SHOP_REOPEN_GOLD: f64 = 5_000.0;
/// Рейтинг после переоткрытия лавки.
pub const CITY_SHOP_REOPEN_RATING: f32 = 2.0;

pub const SHOP_STARTING_SLOTS: u32 = 3;
pub const SHOP_MAX_SLOTS: u32 = 10;
pub const SHOP_STARTING_CASH_CAP: f64 = 5_000.0;
pub const SHOP_MAX_CASH_CAP: f64 = 100_000.0;
pub const SHOP_STARTING_RATING: f32 = 1.0;
pub const SHOP_MAX_RATING: f32 = 5.0;
pub const SHOP_PRICE_MIN: f32 = 0.5;
pub const SHOP_PRICE_MAX: f32 = 2.0;
pub const SHOP_BUYER_BASE_INTERVAL: f64 = 25.0;
/// Рейтинг растёт не за каждую продажу, а раз в N продаж.
pub const SHOP_RATING_SALES_PER_GAIN: u32 = 5;
pub const SHOP_RATING_PER_GAIN: f32 = 0.25;
pub const SHOP_RATING_PER_LEAVE: f32 = 0.02;
pub const SHOP_EMPTY_RATING_PENALTY: f32 = 0.01;
/// Порог тренда спроса на рынке города: столько
/// пунктов нужно, чтобы стрелка показала рост или падение.
pub const MARKET_TREND_THRESHOLD: u32 = 16;

/// С какого момента наценка начинает отпугивать покупателя и при какой
/// он уходит всегда. Между ними шанс растёт плавно, квадратично.
pub const SHOP_PRICE_LEAVE_FROM: f32 = 1.3;
pub const SHOP_PRICE_LEAVE_FULL: f32 = 2.0;
/// Наценка VIP-покупателя.
pub const SHOP_VIP_PRICE_BONUS: f64 = 0.2;
/// Обычный покупатель берёт не больше этого числа штук за визит.
pub const SHOP_BUYER_MAX: u32 = 3;
/// VIP-покупатель берёт не больше этого числа штук за визит.
pub const SHOP_VIP_MAX: u32 = 5;
/// Товар дороже этой цены на полке берут только VIP.
pub const SHOP_VIP_ONLY_PRICE: f64 = 500.0;

// ---------- Спрос города ----------
//
// Город живёт по той же схеме, что и рынок торговцев (`game::market`),
// только масштаб крупнее: ёмкость — тир товара у торговца × `DEMAND_CITY_SCALE`.
// У торговца запас конечен и пополняется, у города спрос плавает цеховыми
// циклами: у каждого товара свой спрос, своё
// предложение и своё настроение с памятью, поэтому дефицит и избыток
// держатся часами, а не один бросок.

/// Шаг настроения города: как часто спрос и предложение делают шаг.
pub const DEMAND_STEP_SECS: f64 = 20.0;
/// Равновесие города: к этим значениям тянутся спрос и предложение.
pub const DEMAND_EQUILIBRIUM: u8 = 50;
/// Сколько пунктов спрос/предложение подтягиваются к равновесию за шаг.
pub const DEMAND_RETURN_STEP: u8 = 1;
/// Во сколько раз город закупает больше одной лавки торговца.
///
/// Тир товара тот же (`Tier::values().1`): сырьё — 50 × 30 = 1500 штук,
/// эпик — 1 × 30 = 30. Отдельной «городской цены» у спроса нет.
pub const DEMAND_CITY_SCALE: f64 = 30.0;
/// Сколько штук город спрашивает дополнительно за каждую точку дефицита
/// сверх равновесия (и теряет при избытке), в долях от ёмкости на пункт.
pub const DEMAND_TREND_UNITS: f64 = 0.02;
/// Сколько штук покупатель берёт за визит в городе — вдвое больше, чем
/// у торговца: город берёт партиями.
pub const SHOP_VISIT_MULT: f64 = 2.0;
/// Полная ёмкость спроса наливается за 30 минут.
pub const DEMAND_FILL_SECS: f64 = 30.0 * 60.0;
/// Вес одной проданной штуки в спросе: 16 штук снимают один пункт.
pub const DEMAND_SALE_DRAIN: f64 = 1.0;
/// Максимальный шаг симуляции спроса за один тик (секунды).
pub const DEMAND_MAX_STEP: f64 = 60.0;
/// Шанс уйти из-за насыщенного спроса (при пустой полке).
pub const DEMAND_LEAVE_MAX: f64 = 0.6;
/// Во сколько раз быстрее восстанавливается спрос на товар из слуха.
pub const DEMAND_RUMOR_FILL_MULT: f64 = 2.0;
/// Наценка лавки на товар из слуха.
pub const SHOP_RUMOR_PRICE_BONUS: f64 = 0.5;

// ---------- Наёмные линии ----------

/// Во сколько раз наёмный работник добывает больше общинного.
pub const HIRE_YIELD_MULT: f64 = 3.0;
/// Зарплата одного наёмного работника за цикл (30 минут).
pub const HIRE_SALARY: f64 = 150.0;
/// Жалованная грамота на наём: разовая цена в золоте.
pub const CHARTER_COST: f64 = 25_000.0;

// ---------- Путешественники в таверне ----------

/// Сколько путешественников живёт в таверне.
pub const TRAVELER_COUNT: usize = 3;
/// Один кружок дружбы в очках.
pub const TRAVELER_FRIENDSHIP_POINT: u32 = 10;
/// Потолок дружбы: пять кружков.
pub const TRAVELER_FRIENDSHIP_MAX: u32 = 50;
/// Цена разговора с путешественником.
pub const TRAVELER_TALK_PRICE: f64 = 100.0;
/// Прибавка дружбы за успешный квест и разговор.
pub const TRAVELER_QUEST_GAIN: u32 = 10;
/// Штраф дружбы за провальный квест.
pub const TRAVELER_QUEST_LOSS: u32 = 10;
/// Доплата за лут при возвращении.
pub const TRAVELER_PAYOUT_PERCENT: f64 = 0.15;
/// Доля лута, которая становится доплатой.
pub const TRAVELER_PAYOUT_LOOT_SHARE: f64 = 0.15;
/// Минимальная доплата, если в снаряжении есть торговый бонус.
pub const TRAVELER_PAYOUT_MIN_WITH_TRADE: f64 = 0.05;
/// Сколько лута везёт путешественник за поход (строк таблицы).
pub const TRAVELER_LOOT_ROLLS: u32 = 5;
/// Сколько лута добавляет каждая точка бонуса снаряжения (в %).
pub const TRAVELER_GEAR_LOOT_STEP: f64 = 1.0;

/// Сколько путешественников живёт в доме после его постройки.
pub const TRAVELER_TARGET_COUNT: usize = 5;

/// Одна строка лута локации путешественника.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TravelerLootDef {
    pub item_id: &'static str,
    pub min_quantity: u32,
    pub max_quantity: u32,
    /// Вес в таблице дропа (проценты за штуку).
    pub weight: u32,
}

/// Куда можно отправить путешественника.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TravelerLocationDef {
    pub id: &'static str,
    /// Сколько длится путь в секундах.
    pub minutes: f64,
    /// Аванс в золоте.
    pub advance: f64,
    /// Шанс вернуться ни с чем.
    pub empty_chance: f64,
    /// Открывается только с Домом странников.
    pub needs_house: bool,
    pub loot: &'static [TravelerLootDef],
}

/// Шесть направлений путешественников.
/// До постройки дома доступны только три ближние.
pub const TRAVELER_LOCATIONS: [TravelerLocationDef; 6] = [
    TravelerLocationDef {
        id: "river_meadows",
        minutes: 10.0,
        advance: 500.0,
        empty_chance: 0.10,
        needs_house: false,
        loot: &[
            TravelerLootDef {
                item_id: "wood",
                min_quantity: 20,
                max_quantity: 70,
                weight: 40,
            },
            TravelerLootDef {
                item_id: "stone",
                min_quantity: 20,
                max_quantity: 50,
                weight: 30,
            },
            TravelerLootDef {
                item_id: "clay",
                min_quantity: 5,
                max_quantity: 20,
                weight: 15,
            },
            TravelerLootDef {
                item_id: "fiber",
                min_quantity: 10,
                max_quantity: 20,
                weight: 10,
            },
            TravelerLootDef {
                item_id: "leather",
                min_quantity: 1,
                max_quantity: 8,
                weight: 5,
            },
        ],
    },
    TravelerLocationDef {
        id: "stony_hill",
        minutes: 15.0,
        advance: 800.0,
        empty_chance: 0.15,
        needs_house: false,
        loot: &[
            TravelerLootDef {
                item_id: "stone",
                min_quantity: 30,
                max_quantity: 80,
                weight: 49,
            },
            TravelerLootDef {
                item_id: "iron",
                min_quantity: 10,
                max_quantity: 40,
                weight: 15,
            },
            TravelerLootDef {
                item_id: "coal",
                min_quantity: 5,
                max_quantity: 20,
                weight: 24,
            },
            TravelerLootDef {
                item_id: "quartz",
                min_quantity: 4,
                max_quantity: 10,
                weight: 12,
            },
        ],
    },
    TravelerLocationDef {
        id: "thick_forest",
        minutes: 20.0,
        advance: 600.0,
        empty_chance: 0.12,
        needs_house: false,
        loot: &[
            TravelerLootDef {
                item_id: "wood",
                min_quantity: 80,
                max_quantity: 150,
                weight: 55,
            },
            TravelerLootDef {
                item_id: "leather",
                min_quantity: 10,
                max_quantity: 20,
                weight: 20,
            },
            TravelerLootDef {
                item_id: "fiber",
                min_quantity: 15,
                max_quantity: 30,
                weight: 25,
            },
        ],
    },
    TravelerLocationDef {
        id: "highlands",
        minutes: 25.0,
        advance: 1_500.0,
        empty_chance: 0.20,
        needs_house: true,
        loot: &[
            TravelerLootDef {
                item_id: "stone",
                min_quantity: 40,
                max_quantity: 90,
                weight: 35,
            },
            TravelerLootDef {
                item_id: "iron",
                min_quantity: 15,
                max_quantity: 50,
                weight: 20,
            },
            TravelerLootDef {
                item_id: "silver",
                min_quantity: 5,
                max_quantity: 10,
                weight: 15,
            },
            TravelerLootDef {
                item_id: "quartz",
                min_quantity: 5,
                max_quantity: 20,
                weight: 30,
            },
        ],
    },
    TravelerLocationDef {
        id: "old_mines",
        minutes: 30.0,
        advance: 2_000.0,
        empty_chance: 0.25,
        needs_house: true,
        loot: &[
            TravelerLootDef {
                item_id: "iron",
                min_quantity: 20,
                max_quantity: 60,
                weight: 40,
            },
            TravelerLootDef {
                item_id: "coal",
                min_quantity: 10,
                max_quantity: 30,
                weight: 35,
            },
            TravelerLootDef {
                item_id: "gold_ore",
                min_quantity: 1,
                max_quantity: 10,
                weight: 10,
            },
            TravelerLootDef {
                item_id: "gems",
                min_quantity: 1,
                max_quantity: 10,
                weight: 15,
            },
        ],
    },
    TravelerLocationDef {
        id: "overseas_coast",
        minutes: 30.0,
        advance: 1_800.0,
        empty_chance: 0.15,
        needs_house: true,
        loot: &[
            TravelerLootDef {
                item_id: "wood",
                min_quantity: 50,
                max_quantity: 100,
                weight: 35,
            },
            TravelerLootDef {
                item_id: "leather",
                min_quantity: 10,
                max_quantity: 40,
                weight: 25,
            },
            TravelerLootDef {
                item_id: "fiber",
                min_quantity: 10,
                max_quantity: 30,
                weight: 20,
            },
            TravelerLootDef {
                item_id: "quartz",
                min_quantity: 5,
                max_quantity: 15,
                weight: 15,
            },
            TravelerLootDef {
                item_id: "cloth",
                min_quantity: 1,
                max_quantity: 8,
                weight: 5,
            },
        ],
    },
];

// ---------- Дом странников ----------

/// Аренда земли под дом.
pub const TRAVELER_HOUSE_RENT: f64 = 8_000.0;
/// Сколько этапов у стройки.
pub const TRAVELER_HOUSE_STAGES: u32 = 3;
/// Сколько длится один этап стройки.
pub const TRAVELER_HOUSE_STAGE_SECS: f64 = 20.0 * 60.0;

/// Материалы и золото одного этапа стройки. Первый этап — фундамент,
/// второй — стены, третий — крыша.
pub fn traveler_house_cost(stage: u32) -> Cost {
    match stage {
        0 => cost(600.0, 400.0, 3_000.0),
        1 => cost(900.0, 700.0, 6_000.0),
        _ => cost(1_200.0, 1_000.0, 10_000.0),
    }
}

// ---------- Кузнецы в «Труде» ----------

/// Сколько кузнецов можно построить.
pub const SMITH_COUNT: usize = 2;
/// Стартовое число ячеек рецептов у кузнеца.
pub const SMITH_START_SLOTS: usize = 5;
/// Максимум прокачек скорости.
pub const SMITH_SPEED_MAX_LEVEL: u32 = 5;
/// Максимум прокачек горна.
pub const SMITH_FORGE_MAX_LEVEL: u32 = 5;
/// Максимум прокачек ячеек рецептов.
pub const SMITH_SLOTS_MAX_LEVEL: u32 = 5;
/// Доля цены поделки, которая уходит в жалованье кузнецу.
pub const SMITH_WAGE_SHARE: f64 = 0.10;
/// Минимальное жалованье за одну поделку.
pub const SMITH_WAGE_MIN: f64 = 2.0;
/// Разрешение ратуши на первую кузницу.
pub const SMITH_FIRST_PERMIT_GOLD: f64 = 10_000.0;
/// Разрешение ратуши на вторую кузницу.
pub const SMITH_SECOND_PERMIT_GOLD: f64 = 20_000.0;

/// Пауза между ковками по уровню скорости: 60 → 18 секунд.
pub fn smith_interval(speed_level: u32) -> f64 {
    const SECS: [f64; 6] = [60.0, 48.0, 38.0, 30.0, 24.0, 18.0];
    SECS[(speed_level as usize).min(SECS.len() - 1)]
}

/// Ёмкость горна кузнеца по уровню: 300 → 1800 огня.
pub fn smith_fire_capacity(forge_level: u32) -> u32 {
    const FIRE: [u32; 6] = [300, 500, 750, 1_050, 1_400, 1_800];
    FIRE[(forge_level as usize).min(FIRE.len() - 1)]
}

/// Сколько огня даёт один уголь в горне кузнеца: 20 → 50.
pub fn smith_fire_per_coal(forge_level: u32) -> u32 {
    const FIRE: [u32; 6] = [20, 25, 30, 35, 40, 50];
    FIRE[(forge_level as usize).min(FIRE.len() - 1)]
}

/// Сколько рецептов помещается в очередь кузнеца.
pub fn smith_slots(slots_level: u32) -> usize {
    SMITH_START_SLOTS + (slots_level as usize).min(SMITH_SLOTS_MAX_LEVEL as usize)
}

/// Жалованье за одну поделку: 10 % цены, но не меньше двух золотых.
pub fn smith_wage(item_price: u32, quantity: u32) -> f64 {
    (item_price as f64 * quantity.max(1) as f64 * SMITH_WAGE_SHARE).max(SMITH_WAGE_MIN)
}

/// Стоимость постройки кузнеца: первый дешевле, второй — с разрешением.
pub fn smith_build_cost(index: usize) -> Cost {
    if index == 0 {
        cost(1_200.0, 800.0, 5_000.0)
    } else {
        cost(2_500.0, 1_600.0, 8_000.0)
    }
}

/// Улучшение скорости: золото, дерево и камень.
pub fn smith_speed_cost(level: u32) -> Cost {
    const GOLD: [f64; 5] = [2_000.0, 3_500.0, 6_000.0, 10_000.0, 16_000.0];
    const WOOD: [f64; 5] = [600.0, 900.0, 1_400.0, 2_100.0, 3_200.0];
    const STONE: [f64; 5] = [400.0, 600.0, 900.0, 1_400.0, 2_100.0];
    let index = (level as usize).min(GOLD.len() - 1);
    cost(WOOD[index], STONE[index], GOLD[index])
}

/// Улучшение горна: золото, камень и железные слитки.
pub fn smith_forge_cost(level: u32) -> Cost {
    const GOLD: [f64; 5] = [2_500.0, 4_500.0, 7_500.0, 12_000.0, 19_000.0];
    const STONE: [f64; 5] = [500.0, 800.0, 1_200.0, 1_800.0, 2_700.0];
    const INGOTS: [u32; 5] = [20, 35, 60, 100, 160];
    let index = (level as usize).min(GOLD.len() - 1);
    Cost {
        wood: 0.0,
        stone: STONE[index],
        gold: GOLD[index],
        energy: 0.0,
        boards: 0,
        iron_ingots: INGOTS[index],
    }
}

/// Улучшение ячеек рецептов: только золото.
pub fn smith_slots_cost(level: u32) -> Cost {
    const GOLD: [f64; 5] = [3_000.0, 5_500.0, 9_000.0, 15_000.0, 24_000.0];
    let index = (level as usize).min(GOLD.len() - 1);
    cost(0.0, 0.0, GOLD[index])
}

// ---------- Приказчик в лавке ----------

/// Разовая цена найма приказчика.
pub const CLERK_PRICE: f64 = 3_000.0;
/// Зарплата приказчика за цикл (30 минут).
pub const CLERK_SALARY: f64 = 100.0;
/// Как часто приказчик выставляет товар.
pub const CLERK_INTERVAL: f64 = 60.0;
/// Наценка приказчика по умолчанию.
pub const CLERK_DEFAULT_MULT: f32 = 1.2;
/// Границы наценки приказчика.
pub const CLERK_MULT_MIN: f32 = 1.0;
pub const CLERK_MULT_MAX: f32 = 2.0;
/// Стартовый размер склада приказчика.
pub const CLERK_START_CELLS: u32 = 5;
/// Сколько ячеек склада добавляет одно улучшение гильдии.
pub const CLERK_CELLS_PER_UPGRADE: u32 = 5;
/// Сколько штук приказчик выставляет на полку за один шаг.
pub const CLERK_BATCH: u32 = 10;

pub const GUILD_SALES_REQUIRED: u32 = 15;
pub const GUILD_ENTRY_GOLD: f64 = 4_000.0;
pub const GUILD_RANK_SALES: u32 = 20;
pub const GUILD_RANK_MAX: u32 = 10;

pub const TOWN_HALL_SALES_REQUIRED: u32 = 60;
pub const TOWN_HALL_MIN_RATING: f32 = 3.0;

pub const BANK_CELLS_MAX: u32 = 100;
pub const BANK_CELL_BONUS: f64 = 3_000.0;
pub const BANK_CELL_BASE_PRICE: f64 = 5_000.0;
pub const BANK_CELL_PRICE_GROWTH: f64 = 1.02;

/// Слух в таверне: цена и перезарядка.
pub const TAVERN_RUMOR_PRICE: f64 = 100.0;
pub const TAVERN_RUMOR_COOLDOWN: f64 = 30.0 * 60.0;
pub const TAVERN_RUMOR_DURATION: f64 = 5.0 * 60.0;
/// Кости: границы ставки.
pub const TAVERN_DICE_MIN_BET: f64 = 100.0;
pub const TAVERN_DICE_MAX_BET: f64 = 1_000.0;

/// Пожертвования в храме.
pub const CHURCH_DONATIONS: [f64; 3] = [100.0, 1_000.0, 10_000.0];
/// Длительность благословения: sqrt(сумма) × столько секунд.
/// 100 золота → 10 минут, 10 000 золота → 100 минут.
pub const CHURCH_BLESSING_SECS_PER_SQRT_GOLD: f64 = 60.0;
pub const CHURCH_CONFESSION_PRICE: f64 = 500.0;
pub const CHURCH_CONFESSION_COOLDOWN: f64 = 24.0 * 60.0 * 60.0;
pub const CHURCH_BLESSING_VIP_BONUS: f64 = 0.10;
pub const CHURCH_BLESSING_PRICE_BONUS: f64 = 0.05;

/// Городские события: шанс запуска и разброс интервала.
pub const CITY_EVENT_CHANCE: u64 = 35;
pub const CITY_EVENT_JITTER: f64 = 10.0 * 60.0;
pub const CITY_EVENT_COMPETITOR_SECS: f64 = 30.0 * 60.0;
pub const CITY_EVENT_EPIDEMIC_SECS: f64 = 30.0 * 60.0;
pub const CITY_EVENT_FESTIVAL_SECS: f64 = 30.0 * 60.0;
pub const CITY_EVENT_FAIR_SECS: f64 = 30.0 * 60.0;
pub const CITY_EVENT_FIRE_REPAIR_GOLD: f64 = 500.0;
pub const CITY_EVENT_BRIBE_GOLD: f64 = 6_000.0;

/// Название товара для слуха/событий — не чаще одной проверки в секунду.
pub const CITY_TICK_MIN_STEP: f64 = 0.0;
