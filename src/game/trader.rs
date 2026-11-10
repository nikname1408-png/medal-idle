//! Несколько торговцев, живой рынок и сделки.
//!
//! Модуль не зависит от Dioxus: спрос, предложение, запасы и цены можно
//! проверять через `cargo test`.

use serde::{Deserialize, Serialize};

use super::items::{self, Item};
use super::state::GameState;

/// Как часто рынок делает шаг симуляции.
pub const MARKET_TICK_SECONDS: f64 = 5.0;

/// Доля базовой цены, которую торговец платит игроку в равновесии.
pub const SELL_RATE: f64 = 0.60;

/// Наценка торговца к базовой цене в равновесии.
pub const BUY_RATE: f64 = 1.15;

/// Шанс смены одного товара за шаг рынка (в процентах).
pub const ROTATION_PERCENT: u64 = 30;

/// Предел «настроения рынка»: насколько спрос и предложение расходятся
/// вокруг 50 из-за медленного тренда.
const TREND_LIMIT: i32 = 20;

/// Описание торговца в каталоге.
#[derive(Clone, Copy, Debug)]
pub struct TraderDef {
    pub id: &'static str,
    pub name: &'static str,
    pub goods: &'static [&'static str],
}

/// Все торговцы игры.
pub const TRADERS: [TraderDef; 4] = [
    TraderDef {
        id: "marta",
        name: "Марта",
        goods: &[
            "jug",
            "candle",
            "cloth",
            "board",
            "brick",
            "horseshoe",
            "iron_dagger",
            "silver_ring",
            "trader_ring",
            "wanderer_pendant",
            "whetstone",
            "clay",
        ],
    },
    TraderDef {
        id: "wanderer",
        name: "Странник",
        goods: &[
            "leather",
            "fiber",
            "clay",
            "quartz",
            "silver",
            "gems",
            "backpack",
            "wooden_bucket",
            "sickle",
            "scythe",
            "candle",
            "map_forest",
            "map_mines",
        ],
    },
    TraderDef {
        id: "smith",
        name: "Кузнец",
        goods: &[
            "iron",
            "iron_ingot",
            "gold_ingot",
            "nails",
            "board",
            "iron_bucket",
            "hoe",
            "pickaxe",
            "axe",
            "iron_sword",
            "chainmail",
            "iron_bracers",
        ],
    },
    TraderDef {
        id: "miner",
        name: "Рудокоп",
        goods: &[
            "iron",
            "silver",
            "quartz",
            "gems",
            "gold_ore",
            "iron_ingot",
            "gold_ingot",
            "whetstone",
            "miner_amulet",
            "iron_bracers",
        ],
    },
];

pub fn trader_def(index: usize) -> Option<&'static TraderDef> {
    TRADERS.get(index)
}

pub fn trader_name(index: usize) -> &'static str {
    trader_def(index)
        .map(|trader| trader.name)
        .unwrap_or("Торговец")
}

/// Состояние рынка по разнице спроса и предложения.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketStatus {
    Deficit,
    Balance,
    Surplus,
}

impl MarketStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deficit => "ДЕФИЦИТ",
            Self::Balance => "БАЛАНС",
            Self::Surplus => "ИЗБЫТОК",
        }
    }

    /// Короткий статус для таблицы.
    pub fn short(self) -> &'static str {
        match self {
            Self::Deficit => "Д↑",
            Self::Balance => "Б→",
            Self::Surplus => "И↓",
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            Self::Deficit => "↑",
            Self::Balance => "→",
            Self::Surplus => "↓",
        }
    }
}

/// Рынок: спрос − предложение.
///
/// `>= +16` — дефицит, `<= -16` — избыток, между ними — баланс.
pub fn market_status(demand: u8, supply: u8) -> MarketStatus {
    match demand as i32 - supply as i32 {
        diff if diff >= 16 => MarketStatus::Deficit,
        diff if diff <= -16 => MarketStatus::Surplus,
        _ => MarketStatus::Balance,
    }
}

/// Модификатор рынка: дефицит двигает цену вверх, избыток — вниз.
pub fn market_factor(demand: u8, supply: u8) -> f64 {
    let diff = demand as f64 - supply as f64;
    1.0 + diff / 200.0
}

/// Цена покупки: всегда выше базовой, растёт при дефиците.
pub fn buy_price(item: &Item, demand: u8, supply: u8) -> f64 {
    let factor = (market_factor(demand, supply) + BUY_RATE - 1.0).clamp(1.02, 1.75);
    (item.base_price as f64 * factor).round().max(1.0)
}

/// Цена продажи: всегда ниже базовой, растёт от бонуса торговли.
pub fn sell_price(item: &Item, demand: u8, supply: u8, trade_bonus: f32) -> f64 {
    let factor = (SELL_RATE + (demand as f64 - supply as f64) / 300.0).clamp(0.30, 0.90);
    let bonus = 1.0 + trade_bonus as f64;
    (item.base_price as f64 * factor * bonus).round().max(1.0)
}

/// Наценка торговца: разница между покупкой и продажей.
pub fn margin(item: &Item, demand: u8, supply: u8, trade_bonus: f32) -> f64 {
    (buy_price(item, demand, supply) - sell_price(item, demand, supply, trade_bonus)).max(0.0)
}

/// Один товар на рынке.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoodState {
    pub item_id: String,
    /// Запас у торговца.
    pub stock: u32,
    /// Спрос 0–100.
    pub demand: u8,
    /// Предложение 0–100.
    pub supply: u8,
    /// Медленный тренд рынка: спрос и предложение тянутся к `50 ± trend`.
    /// Благодаря ему на рынке бывают устойчивые дефицит и избыток, а не
    /// вечное равновесие 50/50.
    pub trend: i8,
    /// Сколько ещё торговец готов скупить за цикл.
    pub buy_limit: u32,
}

impl Default for GoodState {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            stock: 0,
            demand: 50,
            supply: 50,
            trend: 0,
            buy_limit: 0,
        }
    }
}

impl GoodState {
    pub fn diff(&self) -> i32 {
        self.demand as i32 - self.supply as i32
    }

    pub fn status(&self) -> MarketStatus {
        market_status(self.demand, self.supply)
    }
}

/// Ассортимент одного торговца.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraderMarket {
    pub goods: Vec<GoodState>,
}

impl TraderMarket {
    pub fn good(&self, item_id: &str) -> Option<&GoodState> {
        self.goods.iter().find(|good| good.item_id == item_id)
    }

    pub fn good_mut(&mut self, item_id: &str) -> Option<&mut GoodState> {
        self.goods.iter_mut().find(|good| good.item_id == item_id)
    }
}

fn good(item_id: &str, stock: u32, demand: u8, supply: u8, buy_limit: u32) -> GoodState {
    // Стартовое настроение выводим из демо-спроса и предложения, чтобы
    // витрина сразу начиналась с разными статусами рынка.
    let trend = ((demand as i32 - supply as i32) / 2).clamp(-TREND_LIMIT, TREND_LIMIT) as i8;
    GoodState {
        item_id: item_id.to_string(),
        stock,
        demand,
        supply,
        trend,
        buy_limit,
    }
}

/// Редкость товара на рынке: чем реже, тем меньше шанс появления.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoodRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl GoodRarity {
    /// Редкость по базовой цене предмета.
    pub fn of(item: &Item) -> Self {
        match item.base_price {
            0..=5 => Self::Common,
            6..=20 => Self::Uncommon,
            21..=60 => Self::Rare,
            _ => Self::Epic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Common => "обычный",
            Self::Uncommon => "необычный",
            Self::Rare => "редкий",
            Self::Epic => "эпический",
        }
    }

    /// Вес выпадения редкости.
    fn weight(self) -> u64 {
        match self {
            Self::Common => 55,
            Self::Uncommon => 28,
            Self::Rare => 13,
            Self::Epic => 4,
        }
    }

    fn lower(self) -> Option<Self> {
        match self {
            Self::Common => None,
            Self::Uncommon => Some(Self::Common),
            Self::Rare => Some(Self::Uncommon),
            Self::Epic => Some(Self::Rare),
        }
    }
}

/// Товары, у которых увеличены запас и лимит скупки.
fn bulk_volume(item_id: &str) -> Option<(u32, u32)> {
    match item_id {
        "clay" => Some((150, 100)),
        "brick" => Some((80, 60)),
        "jug" => Some((60, 50)),
        "board" => Some((100, 80)),
        "nails" => Some((100, 80)),
        "iron" => Some((120, 100)),
        _ => None,
    }
}

/// Норма запаса товара у торговца.
fn base_stock(item_id: &str) -> u32 {
    if let Some((stock, _)) = bulk_volume(item_id) {
        return stock;
    }
    let Some(item) = items::item(item_id) else {
        return 1;
    };
    match GoodRarity::of(item) {
        GoodRarity::Common => 40,
        GoodRarity::Uncommon => 24,
        GoodRarity::Rare => 8,
        GoodRarity::Epic => 2,
    }
}

/// Норма лимита скупки товара за цикл.
fn base_buy_limit(item_id: &str) -> u32 {
    if let Some((_, limit)) = bulk_volume(item_id) {
        return limit;
    }
    let Some(item) = items::item(item_id) else {
        return 1;
    };
    match GoodRarity::of(item) {
        GoodRarity::Common => 60,
        GoodRarity::Uncommon => 40,
        GoodRarity::Rare => 20,
        GoodRarity::Epic => 6,
    }
}

/// Свежий товар, появившийся в лавке.
fn fresh_good(item_id: &str) -> GoodState {
    // Новый товар приходит на витрину нейтральным: настроение рынка дальше
    // гуляет само, без перекоса в дефицит.
    good(
        item_id,
        base_stock(item_id),
        50,
        50,
        base_buy_limit(item_id),
    )
}

/// Демо-ассортимент торговца по индексу.
///
/// В лавке показывается только часть ассортимента: остальные товары могут
/// появиться при ротации рынка.
pub fn default_market(index: usize) -> TraderMarket {
    let goods = match index {
        // Марта: товары на продажу и бижутерия.
        0 => vec![
            good("jug", 60, 55, 40, 50),
            good("candle", 40, 30, 70, 30),
            good("cloth", 30, 20, 85, 20),
            good("board", 100, 35, 60, 80),
            good("brick", 80, 45, 55, 60),
            good("horseshoe", 8, 60, 40, 8),
            good("silver_ring", 2, 75, 30, 3),
            good("whetstone", 6, 65, 40, 6),
        ],
        // Странник: ремесло, ферма и редкие находки.
        1 => vec![
            good("leather", 18, 55, 45, 10),
            good("fiber", 40, 25, 70, 30),
            good("clay", 150, 40, 70, 100),
            good("quartz", 6, 70, 40, 5),
            good("silver", 5, 78, 32, 5),
            good("gems", 2, 85, 25, 2),
            good("backpack", 2, 80, 30, 3),
            good("wooden_bucket", 4, 50, 55, 5),
            good("candle", 30, 35, 65, 20),
        ],
        // Кузнец: металл, инструменты и оружие.
        2 => vec![
            good("iron", 80, 50, 60, 80),
            good("iron_ingot", 12, 50, 55, 8),
            good("gold_ingot", 1, 85, 25, 2),
            good("nails", 100, 45, 60, 80),
            good("board", 100, 30, 70, 80),
            good("iron_bucket", 4, 50, 55, 5),
            good("pickaxe", 2, 65, 40, 3),
            good("axe", 2, 60, 40, 3),
            good("iron_bracers", 1, 70, 35, 2),
        ],
        // Рудокоп: руды, металлы и шахтёрское снаряжение.
        _ => vec![
            good("iron", 120, 55, 50, 100),
            good("silver", 8, 78, 32, 8),
            good("quartz", 10, 70, 40, 8),
            good("gems", 2, 85, 25, 2),
            good("gold_ore", 3, 88, 22, 3),
            good("iron_ingot", 10, 50, 55, 8),
            good("whetstone", 5, 60, 45, 5),
            good("miner_amulet", 1, 82, 25, 2),
        ],
    };

    TraderMarket { goods }
}

/// Все рынки по умолчанию.
pub fn default_markets() -> Vec<TraderMarket> {
    (0..TRADERS.len()).map(default_market).collect()
}

/// Результат сделки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeError {
    UnknownTrader,
    UnknownItem,
    InvalidQuantity,
    OutOfStock,
    NotEnoughGold,
    GoldCapReached,
    NothingToSell,
    LimitReached,
    NoSpace,
}

impl TradeError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownTrader => "Такого торговца нет.",
            Self::UnknownItem => "Такого товара нет.",
            Self::InvalidQuantity => "Количество должно быть больше нуля.",
            Self::OutOfStock => "У торговца не хватает запаса.",
            Self::NotEnoughGold => "Недостаточно золота.",
            Self::GoldCapReached => "Лимит золота достигнут. Улучши банк.",
            Self::NothingToSell => "У вас нет столько этого товара.",
            Self::LimitReached => "Торговец больше не скупает этот товар за цикл.",
            Self::NoSpace => "В сумке нет места.",
        }
    }
}

/// Покупка товара у торговца.
pub fn buy(
    state: &mut GameState,
    trader_index: usize,
    item_id: &str,
    quantity: u32,
) -> Result<(), TradeError> {
    if quantity == 0 {
        return Err(TradeError::InvalidQuantity);
    }
    let item = items::item(item_id).ok_or(TradeError::UnknownItem)?;
    let market = state
        .traders
        .get(trader_index)
        .ok_or(TradeError::UnknownTrader)?;
    let good = market.good(item_id).ok_or(TradeError::UnknownItem)?;

    if quantity > good.stock {
        return Err(TradeError::OutOfStock);
    }

    let price = buy_price(item, good.demand, good.supply);
    let total = price * quantity as f64;
    if state.gold + 1e-9 < total {
        return Err(TradeError::NotEnoughGold);
    }
    if !state.can_add_item(item_id, quantity) {
        return Err(TradeError::NoSpace);
    }

    state.gold = (state.gold - total).max(0.0);
    state.add_item(item_id, quantity);

    if let Some(good) = state
        .traders
        .get_mut(trader_index)
        .and_then(|market| market.good_mut(item_id))
    {
        good.stock -= quantity;
        good.demand = good.demand.saturating_add(quantity.min(20) as u8).min(100);
        good.supply = good.supply.saturating_sub(quantity.min(20) as u8).max(5);
    }

    Ok(())
}

/// Продажа товара торговцу.
pub fn sell(
    state: &mut GameState,
    trader_index: usize,
    item_id: &str,
    quantity: u32,
) -> Result<(), TradeError> {
    if quantity == 0 {
        return Err(TradeError::InvalidQuantity);
    }
    let item = items::item(item_id).ok_or(TradeError::UnknownItem)?;
    let market = state
        .traders
        .get(trader_index)
        .ok_or(TradeError::UnknownTrader)?;
    let good = market.good(item_id).ok_or(TradeError::UnknownItem)?;

    if state.item_count(item_id) < quantity {
        return Err(TradeError::NothingToSell);
    }
    if quantity > good.buy_limit {
        return Err(TradeError::LimitReached);
    }

    let cap = state.gold_cap();
    let price = sell_price(item, good.demand, good.supply, state.trade_bonus);
    let total = price * quantity as f64;
    if state.gold + total > cap + 1e-9 {
        return Err(TradeError::GoldCapReached);
    }

    if !state.remove_item(item_id, quantity) {
        return Err(TradeError::NothingToSell);
    }

    let before = state.gold;
    state.gold = (state.gold + total).min(cap);
    state.stats.gold_earned += state.gold - before;

    if let Some(good) = state
        .traders
        .get_mut(trader_index)
        .and_then(|market| market.good_mut(item_id))
    {
        good.stock = good.stock.saturating_add(quantity);
        good.buy_limit = good.buy_limit.saturating_sub(quantity);
        // Продажа игрока сильно роняет цену: предложение растёт, спрос падает.
        let impact = (quantity * 4).min(80) as u8;
        good.supply = good.supply.saturating_add(impact).min(100);
        good.demand = good.demand.saturating_sub(impact).max(5);
    }

    Ok(())
}

/// Сколько единиц можно купить при данном золоте и запасе.
pub fn max_buy(gold: f64, stock: u32, price: f64) -> u32 {
    if price <= 0.0 {
        return 0;
    }
    let by_gold = (gold / price).floor().max(0.0) as u32;
    stock.min(by_gold)
}

/// Сколько единиц можно продать с учётом остатка лимита скупки и места под золото.
pub fn max_sell(owned: u32, buy_limit: u32, gold: f64, gold_cap: f64, price: f64) -> u32 {
    if price <= 0.0 {
        return 0;
    }
    let room = (gold_cap - gold).max(0.0);
    let by_gold = (room / price).floor() as u32;
    owned.min(buy_limit).min(by_gold)
}

/// Продвигает рынок на `dt` секунд.
pub fn tick_markets(state: &mut GameState, dt: f64) {
    state.market_timer += dt.max(0.0);
    while state.market_timer + 1e-9 >= MARKET_TICK_SECONDS {
        state.market_timer -= MARKET_TICK_SECONDS;
        market_step(state);
    }
}

/// Бросает редкость товара по весам.
fn roll_rarity(seed: &mut u64) -> GoodRarity {
    let rarities = [
        GoodRarity::Common,
        GoodRarity::Uncommon,
        GoodRarity::Rare,
        GoodRarity::Epic,
    ];
    let total: u64 = rarities.iter().map(|rarity| rarity.weight()).sum();
    let mut roll = random_below(seed, total);
    for rarity in rarities {
        let weight = rarity.weight();
        if roll < weight {
            return rarity;
        }
        roll -= weight;
    }
    GoodRarity::Common
}

/// Выбирает новый товар нужной редкости, которого ещё нет в лавке.
///
/// Сначала пробуем выпавшую редкость и понижаем её, пока не найдём свободный
/// товар. Если все свободные товары оказались выше выпавшей редкости (так
/// бывает, когда на витрине остались только дешёвые вещи), берём любой
/// свободный товар: иначе рынок мог «зависнуть» навсегда на одной редкости.
/// Один и тот же товар повторно не ставим.
fn pick_rotation_item(
    seed: &mut u64,
    roster: &'static [&'static str],
    market: &TraderMarket,
) -> Option<&'static str> {
    let available: Vec<&'static str> = roster
        .iter()
        .copied()
        .filter(|item_id| market.good(item_id).is_none())
        .collect();
    if available.is_empty() {
        return None;
    }

    let mut rarity = Some(roll_rarity(seed));
    while let Some(current) = rarity {
        let candidates: Vec<&'static str> = available
            .iter()
            .copied()
            .filter(|item_id| items::item(item_id).map(GoodRarity::of) == Some(current))
            .collect();
        if !candidates.is_empty() {
            let pick = random_below(seed, candidates.len() as u64) as usize;
            return Some(candidates[pick]);
        }
        rarity = current.lower();
    }

    let pick = random_below(seed, available.len() as u64) as usize;
    Some(available[pick])
}

/// С шансом [`ROTATION_PERCENT`] заменяет один товар в лавке.
fn rotate_market(seed: &mut u64, index: usize, market: &mut TraderMarket) {
    let Some(trader) = TRADERS.get(index) else {
        return;
    };
    if trader.goods.is_empty() || market.goods.is_empty() {
        return;
    }
    if random_below(seed, 100) >= ROTATION_PERCENT {
        return;
    }
    let Some(item_id) = pick_rotation_item(seed, trader.goods, market) else {
        return;
    };
    let slot = random_below(seed, market.goods.len() as u64) as usize;
    market.goods[slot] = fresh_good(item_id);
}

/// Плавно двигает значение к цели.
fn approach(value: u8, target: u8, step: u8) -> u8 {
    if value < target {
        (value + step).min(target)
    } else {
        value.saturating_sub(step).max(target)
    }
}

fn market_step(state: &mut GameState) {
    let mut seed = state.market_seed;

    for (index, market) in state.traders.iter_mut().enumerate() {
        if market.goods.is_empty() {
            market.goods = default_market(index).goods;
        }
        rotate_market(&mut seed, index, market);

        for good in market.goods.iter_mut() {
            // Настроение рынка медленно гуляет: спрос и предложение тянутся
            // к `50 ± trend`, поэтому периодически возникают устойчивые
            // дефицит и избыток, а не вечный баланс 50/50.
            let trend = (good.trend as i32 + noise(&mut seed, 4)).clamp(-TREND_LIMIT, TREND_LIMIT);
            good.trend = trend as i8;
            let target_demand = (50 + trend) as u8;
            let target_supply = (50 - trend) as u8;

            // Небольшой шум: значения шевелятся на каждом шаге рынка.
            good.demand = (good.demand as i32 + noise(&mut seed, 2)).clamp(5, 95) as u8;
            good.supply = (good.supply as i32 + noise(&mut seed, 2)).clamp(5, 95) as u8;

            // Возврат к настроению постепенный: 1 пункт за шаг рынка, поэтому
            // обвал цены после продаж восстанавливается минутами, а не за 5 с.
            good.demand = approach(good.demand, target_demand, 1);
            good.supply = approach(good.supply, target_supply, 1);

            // Запас медленно восстанавливается до нормы.
            let stock_norm = base_stock(&good.item_id);
            if good.stock < stock_norm {
                let step = 1 + random_below(&mut seed, 2) as u32;
                good.stock = (good.stock + step).min(stock_norm);
            }

            // Лимит скупки тоже восстанавливается понемногу.
            let limit_norm = base_buy_limit(&good.item_id);
            if good.buy_limit < limit_norm {
                let step = 1 + random_below(&mut seed, 2) as u32;
                good.buy_limit = (good.buy_limit + step).min(limit_norm);
            }
        }
    }

    state.market_seed = seed;
}

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *seed
}

/// Случайное число в диапазоне `0..bound` по старшим битам ГПСЧ.
///
/// Младшие биты ЛКГ сильно коррелируют: из-за них шанс смены товара и
/// распределение редкостей получались заметно ниже задуманных.
fn random_below(seed: &mut u64, bound: u64) -> u64 {
    if bound == 0 {
        return 0;
    }
    (next_random(seed) >> 33) % bound
}

/// Детерминированный шум в диапазоне `-range..=range`.
fn noise(seed: &mut u64, range: i32) -> i32 {
    let value = next_random(seed);
    ((value >> 33) as i32) % (range * 2 + 1) - range
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> &'static Item {
        items::item(id).expect("предмет есть в каталоге")
    }

    #[test]
    fn market_status_thresholds() {
        assert_eq!(market_status(72, 31), MarketStatus::Deficit);
        assert_eq!(market_status(66, 50), MarketStatus::Deficit);
        assert_eq!(market_status(65, 50), MarketStatus::Balance);
        assert_eq!(market_status(50, 50), MarketStatus::Balance);
        assert_eq!(market_status(35, 50), MarketStatus::Balance);
        assert_eq!(market_status(34, 50), MarketStatus::Surplus);
    }

    #[test]
    fn demo_goods_cover_all_market_statuses() {
        let market = default_market(0);
        assert_eq!(
            market.good("jug").map(GoodState::status),
            Some(MarketStatus::Balance)
        );
        assert_eq!(
            market.good("horseshoe").map(GoodState::status),
            Some(MarketStatus::Deficit)
        );
        assert_eq!(
            market.good("silver_ring").map(GoodState::status),
            Some(MarketStatus::Deficit)
        );
        assert_eq!(
            market.good("cloth").map(GoodState::status),
            Some(MarketStatus::Surplus)
        );
    }

    #[test]
    fn buy_is_above_base_and_sell_is_below_base() {
        let item = item("iron_ingot");
        let buy = buy_price(item, 50, 50);
        let sell = sell_price(item, 50, 50, 0.0);
        assert!(buy > item.base_price as f64);
        assert!(sell < item.base_price as f64);
    }

    #[test]
    fn deficit_raises_prices_and_surplus_lowers_them() {
        let item = item("iron_ingot");
        let deficit = buy_price(item, 90, 20);
        let balance = buy_price(item, 50, 50);
        let surplus = buy_price(item, 20, 90);
        assert!(deficit > balance);
        assert!(balance > surplus);
    }

    #[test]
    fn trade_bonus_raises_sell_price() {
        let item = item("iron_ingot");
        let without = sell_price(item, 50, 50, 0.0);
        let with = sell_price(item, 50, 50, 0.10);
        assert!(with > without);
    }

    #[test]
    fn buy_spends_gold_and_moves_stock_to_inventory() {
        let mut state = GameState {
            gold: 1000.0,
            ..GameState::default()
        };
        let before = state
            .traders
            .get(2)
            .and_then(|market| market.good("iron_ingot"))
            .map(|good| good.stock)
            .unwrap_or(0);

        assert_eq!(buy(&mut state, 2, "iron_ingot", 2), Ok(()));
        assert_eq!(state.item_count("iron_ingot"), 2);
        assert_eq!(
            state
                .traders
                .get(2)
                .and_then(|market| market.good("iron_ingot"))
                .map(|good| good.stock),
            Some(before - 2)
        );
        assert!(state.gold < 1000.0);
    }

    #[test]
    fn buy_rejects_bad_requests() {
        let mut state = GameState {
            gold: 10.0,
            ..GameState::default()
        };
        assert_eq!(
            buy(&mut state, 2, "iron_ingot", 0),
            Err(TradeError::InvalidQuantity)
        );
        assert_eq!(
            buy(&mut state, 2, "iron_ingot", 999),
            Err(TradeError::OutOfStock)
        );
        assert_eq!(
            buy(&mut state, 2, "iron_ingot", 1),
            Err(TradeError::NotEnoughGold)
        );
    }

    #[test]
    fn sell_moves_inventory_to_trader_and_respects_limit() {
        let mut state = GameState {
            gold: 0.0,
            ..GameState::default()
        };
        state.add_item("iron_ingot", 3);

        assert_eq!(sell(&mut state, 2, "iron_ingot", 2), Ok(()));
        assert_eq!(state.item_count("iron_ingot"), 1);
        assert_eq!(
            state
                .traders
                .get(2)
                .and_then(|market| market.good("iron_ingot"))
                .map(|good| good.buy_limit),
            Some(6)
        );
        assert!(state.gold > 0.0);
    }

    #[test]
    fn sell_rejects_bad_requests() {
        let mut state = GameState {
            gold: 0.0,
            ..GameState::default()
        };
        assert_eq!(
            sell(&mut state, 2, "iron_ingot", 0),
            Err(TradeError::InvalidQuantity)
        );
        assert_eq!(
            sell(&mut state, 2, "iron_ingot", 99),
            Err(TradeError::NothingToSell)
        );

        state.add_item("iron_ingot", 3);
        if let Some(good) = state
            .traders
            .get_mut(2)
            .and_then(|market| market.good_mut("iron_ingot"))
        {
            good.buy_limit = 1;
        }
        assert_eq!(
            sell(&mut state, 2, "iron_ingot", 2),
            Err(TradeError::LimitReached)
        );
    }

    #[test]
    fn market_tick_keeps_values_in_bounds_and_changes_them() {
        let mut state = GameState::default();
        let before: Vec<(u8, u8)> = state
            .traders
            .iter()
            .flat_map(|market| market.goods.iter().map(|good| (good.demand, good.supply)))
            .collect();

        tick_markets(&mut state, MARKET_TICK_SECONDS);

        let after: Vec<(u8, u8)> = state
            .traders
            .iter()
            .flat_map(|market| market.goods.iter().map(|good| (good.demand, good.supply)))
            .collect();

        assert_ne!(before, after);
        for (demand, supply) in after {
            assert!((5..=95).contains(&demand));
            assert!((5..=95).contains(&supply));
        }
    }

    #[test]
    fn max_helpers_respect_stock_and_limits() {
        assert_eq!(max_buy(100.0, 12, 45.0), 2);
        assert_eq!(max_buy(100.0, 1, 45.0), 1);
        assert_eq!(max_buy(10.0, 12, 45.0), 0);
        assert_eq!(max_sell(8, 3, 0.0, 1000.0, 45.0), 3);
        assert_eq!(max_sell(2, 3, 0.0, 1000.0, 45.0), 2);
        // Место под золото ограничивает продажу.
        assert_eq!(max_sell(8, 8, 970.0, 1000.0, 45.0), 0);
        assert_eq!(max_sell(8, 8, 920.0, 1000.0, 45.0), 1);
    }

    #[test]
    fn player_sales_flood_the_market() {
        let mut state = GameState {
            gold: 0.0,
            ..GameState::default()
        };
        state.add_item("board", 80);

        assert_eq!(sell(&mut state, 0, "board", 80), Ok(()));

        let good = state
            .traders
            .get(0)
            .and_then(|market| market.good("board"))
            .cloned()
            .expect("доска осталась в лавке");
        assert!(good.supply >= 90, "предложение не выросло: {}", good.supply);
        assert!(good.demand <= 10, "спрос не упал: {}", good.demand);
    }

    #[test]
    fn flooded_price_is_well_below_balanced() {
        let ring = item("trader_ring");
        let balanced = sell_price(ring, 50, 50, 0.0);
        let flooded = sell_price(ring, 5, 95, 0.0);

        assert!(flooded <= balanced * 0.55);
    }

    #[test]
    fn market_recovers_gradually_after_flood() {
        let mut state = GameState::default();
        // Все товары Марты уже на витрине — ротация не сможет ничего заменить.
        let mut market = TraderMarket { goods: Vec::new() };
        for item_id in TRADERS[0].goods {
            market.goods.push(fresh_good(item_id));
        }
        if let Some(board) = market.good_mut("board") {
            board.supply = 95;
            board.demand = 5;
            // Нейтральное настроение: проверяем возврат именно к 50.
            board.trend = 0;
        }
        state.traders = vec![market];
        state.market_seed = 7;

        market_step(&mut state);
        let after_one = state
            .traders
            .get(0)
            .and_then(|market| market.good("board"))
            .cloned()
            .expect("доска осталась в лавке");
        // За один шаг рынка цена почти не восстанавливается: шум не больше
        // четырёх пунктов, а к равновесию значение идёт на один пункт.
        assert!(
            after_one.supply >= 90,
            "предложение восстановилось слишком быстро: {}",
            after_one.supply
        );
        assert!(
            after_one.demand <= 10,
            "спрос восстановился слишком быстро: {}",
            after_one.demand
        );

        for _ in 0..60 {
            market_step(&mut state);
        }
        let after_minute = state
            .traders
            .get(0)
            .and_then(|market| market.good("board"))
            .cloned()
            .expect("доска осталась в лавке");
        // За пять минут рынок в основном восстанавливается, но не мгновенно.
        assert!(
            after_minute.supply <= 70,
            "за пять минут предложение почти не упало: {}",
            after_minute.supply
        );
        assert!(
            after_minute.demand >= 30,
            "за пять минут спрос почти не вырос: {}",
            after_minute.demand
        );
    }

    #[test]
    fn rotation_never_duplicates_goods() {
        let mut state = GameState::default();
        for _ in 0..200 {
            tick_markets(&mut state, MARKET_TICK_SECONDS);
        }

        for market in &state.traders {
            assert!(!market.goods.is_empty());
            let mut seen = std::collections::HashSet::new();
            for good in &market.goods {
                assert!(seen.insert(good.item_id.clone()), "товар повторяется");
            }
        }
    }

    #[test]
    fn market_rotates_assortment() {
        let mut state = GameState::default();
        let mut changes = vec![0u32; state.traders.len()];
        let mut prev: Vec<Vec<String>> = state
            .traders
            .iter()
            .map(|market| {
                market
                    .goods
                    .iter()
                    .map(|good| good.item_id.clone())
                    .collect()
            })
            .collect();

        for _ in 0..200 {
            market_step(&mut state);
            for (index, market) in state.traders.iter().enumerate() {
                let now: Vec<String> = market
                    .goods
                    .iter()
                    .map(|good| good.item_id.clone())
                    .collect();
                if now != prev[index] {
                    changes[index] += 1;
                }
                prev[index] = now;
            }
        }

        // Шанс смены — 30% за шаг рынка, значит за 200 шагов ожидается ~60.
        for (index, count) in changes.iter().enumerate() {
            assert!(
                *count >= 20,
                "рынок {index} почти не меняется: {count} смен за 200 шагов"
            );
        }
    }

    #[test]
    fn rotation_picks_only_missing_goods() {
        let market = default_market(0);
        let mut seed = 99;
        for _ in 0..1000 {
            let picked = pick_rotation_item(&mut seed, TRADERS[0].goods, &market)
                .expect("на витрине не весь ассортимент");
            assert!(
                market.good(picked).is_none(),
                "{picked} уже есть в лавке — будет дубль"
            );
        }
    }

    #[test]
    fn market_has_deficits_and_surpluses() {
        let mut state = GameState::default();
        let (mut deficits, mut surpluses) = (0u32, 0u32);
        for _ in 0..200 {
            market_step(&mut state);
            for market in &state.traders {
                for good in &market.goods {
                    match good.status() {
                        MarketStatus::Deficit => deficits += 1,
                        MarketStatus::Surplus => surpluses += 1,
                        MarketStatus::Balance => {}
                    }
                }
            }
        }
        assert!(deficits > 100, "дефицит почти не появляется: {deficits}");
        assert!(surpluses > 100, "избыток почти не появляется: {surpluses}");
    }

    #[test]
    fn market_values_keep_moving() {
        let mut state = GameState::default();
        let mut min = u8::MAX;
        let mut max = 0u8;
        for _ in 0..100 {
            market_step(&mut state);
            for good in &state.traders[0].goods {
                min = min.min(good.demand);
                max = max.max(good.demand);
            }
        }
        assert!(max - min >= 10, "спрос почти не меняется: {min}..{max}");
    }
}
