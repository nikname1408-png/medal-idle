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

/// Описание торговца в каталоге.
#[derive(Clone, Copy, Debug)]
pub struct TraderDef {
    pub id: &'static str,
    pub name: &'static str,
    pub goods: &'static [&'static str],
}

/// Все торговцы игры.
pub const TRADERS: [TraderDef; 3] = [
    TraderDef {
        id: "marta",
        name: "Марта",
        goods: &["iron_ingot", "strength_potion", "cloth"],
    },
    TraderDef {
        id: "wanderer",
        name: "Странник",
        goods: &["leather", "quartz", "backpack"],
    },
    TraderDef {
        id: "smith",
        name: "Кузнец",
        goods: &["iron_ore", "wood", "silver", "iron_sword"],
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
/// `>= +20` — дефицит, `<= -20` — избыток, между ними — баланс.
pub fn market_status(demand: u8, supply: u8) -> MarketStatus {
    match demand as i32 - supply as i32 {
        diff if diff >= 20 => MarketStatus::Deficit,
        diff if diff <= -20 => MarketStatus::Surplus,
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
    let factor = (SELL_RATE + (demand as f64 - supply as f64) / 500.0).clamp(0.30, 0.90);
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
    GoodState {
        item_id: item_id.to_string(),
        stock,
        demand,
        supply,
        buy_limit,
    }
}

/// Демо-ассортимент торговца по индексу.
pub fn default_market(index: usize) -> TraderMarket {
    let goods = match index {
        0 => vec![
            good("iron_ingot", 5, 64, 38, 10),
            good("strength_potion", 12, 72, 31, 10),
            good("cloth", 30, 20, 85, 20),
        ],
        1 => vec![
            good("leather", 18, 55, 45, 10),
            good("quartz", 6, 70, 40, 5),
            good("backpack", 2, 80, 30, 3),
        ],
        _ => vec![
            good("iron_ore", 40, 45, 60, 20),
            good("wood", 60, 40, 70, 20),
            good("silver", 4, 75, 35, 5),
            good("iron_sword", 1, 65, 40, 3),
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
        good.demand = good.demand.saturating_add(quantity.min(5) as u8).min(100);
        good.supply = good.supply.saturating_sub(quantity.min(5) as u8).max(5);
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
        good.supply = good.supply.saturating_add(quantity.min(5) as u8).min(100);
        good.demand = good.demand.saturating_sub(quantity.min(5) as u8).max(5);
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

fn market_step(state: &mut GameState) {
    let mut seed = state.market_seed;

    for market in state.traders.iter_mut() {
        for good in market.goods.iter_mut() {
            let demand_shift = noise(&mut seed, 3);
            let supply_shift = noise(&mut seed, 3);
            good.demand = (good.demand as i32 + demand_shift).clamp(5, 95) as u8;
            good.supply = (good.supply as i32 + supply_shift).clamp(5, 95) as u8;

            // Запас тянется к предложению: чем больше предложение, тем больше товара.
            let target = (good.supply as i32 / 3).clamp(0, 40);
            let drift = (target - good.stock as i32).signum();
            let stock_shift = noise(&mut seed, 1);
            good.stock = (good.stock as i32 + drift + stock_shift).clamp(0, 99) as u32;

            if good.stock == 0 && next_random(&mut seed).is_multiple_of(3) {
                good.stock = 2 + (next_random(&mut seed) % 4) as u32;
            }
            if good.buy_limit == 0 {
                good.buy_limit = 1 + (next_random(&mut seed) % 5) as u32;
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
        assert_eq!(market_status(70, 50), MarketStatus::Deficit);
        assert_eq!(market_status(69, 50), MarketStatus::Balance);
        assert_eq!(market_status(50, 50), MarketStatus::Balance);
        assert_eq!(market_status(31, 50), MarketStatus::Balance);
        assert_eq!(market_status(30, 50), MarketStatus::Surplus);
    }

    #[test]
    fn demo_goods_cover_all_market_statuses() {
        let market = default_market(0);
        assert_eq!(
            market.good("iron_ingot").map(GoodState::status),
            Some(MarketStatus::Deficit)
        );
        assert_eq!(
            market.good("strength_potion").map(GoodState::status),
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
            .get(0)
            .and_then(|market| market.good("iron_ingot"))
            .map(|good| good.stock)
            .unwrap_or(0);

        assert_eq!(buy(&mut state, 0, "iron_ingot", 2), Ok(()));
        assert_eq!(state.item_count("iron_ingot"), 2);
        assert_eq!(
            state
                .traders
                .get(0)
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
            buy(&mut state, 0, "iron_ingot", 0),
            Err(TradeError::InvalidQuantity)
        );
        assert_eq!(
            buy(&mut state, 0, "iron_ingot", 999),
            Err(TradeError::OutOfStock)
        );
        assert_eq!(
            buy(&mut state, 0, "iron_ingot", 1),
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

        assert_eq!(sell(&mut state, 0, "iron_ingot", 2), Ok(()));
        assert_eq!(state.item_count("iron_ingot"), 1);
        assert_eq!(
            state
                .traders
                .get(0)
                .and_then(|market| market.good("iron_ingot"))
                .map(|good| good.buy_limit),
            Some(8)
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
            sell(&mut state, 0, "iron_ingot", 0),
            Err(TradeError::InvalidQuantity)
        );
        assert_eq!(
            sell(&mut state, 0, "iron_ingot", 99),
            Err(TradeError::NothingToSell)
        );

        state.add_item("iron_ingot", 3);
        if let Some(good) = state
            .traders
            .get_mut(0)
            .and_then(|market| market.good_mut("iron_ingot"))
        {
            good.buy_limit = 1;
        }
        assert_eq!(
            sell(&mut state, 0, "iron_ingot", 2),
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
}
