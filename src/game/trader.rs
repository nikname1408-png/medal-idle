//! Несколько торговцев, живой рынок и сделки.
//!
//! Модуль не зависит от Dioxus: спрос, предложение, запасы и цены можно
//! проверять через `cargo test`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::balance;
use super::i18n;
use super::items::{self, Item};
use super::state::GameState;

/// Как часто рынок делает шаг симуляции.
pub const MARKET_TICK_SECONDS: f64 = 5.0;

/// Кошелёк торговца: сколько золота он готов отдать за товар за цикл.
/// Деньги ограничивают оборот торговца, поэтому оптом сбыть горы дерева
/// больше не выйдет — крупные партии упираются в кошелёк.
pub const TRADER_WALLET: f64 = 3_000.0;
/// Раз в сколько секунд кошелёк торговца пополняется.
pub const TRADER_WALLET_INTERVAL: f64 = 30.0 * 60.0;

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
    pub goods: &'static [&'static str],
}

impl TraderDef {
    /// Имя торговца на текущем языке.
    pub fn name(&self) -> &'static str {
        i18n::t_or(&format!("trader.{}", self.id), i18n::t("trader.unknown"))
    }
}

/// Общий ассортимент Марты и Кузнеца: весь крафт, кроме карт.
#[rustfmt::skip]
const CRAFT_GOODS: &[&str] = &[
    "jug", "candle", "cloth", "board", "brick", "stone_block",
    "whetstone", "clay", "leather", "fiber",
    "quartz", "silver", "gems", "gold_ore",
    "backpack", "wooden_bucket", "sickle", "scythe",
    "wooden_shovel", "iron_shovel", "hammer",
    "iron_bucket", "iron_ingot", "gold_ingot", "silver_ingot", "iron",
    "silver_ring", "trader_ring", "wanderer_pendant",
    "gold_ring", "gold_pendant", "miner_amulet", "iron_bracers",
    "robe", "leather_gloves", "winter_gloves", "jacket",
    "leather_hat", "wretched_hat", "war_hat", "iron_helmet",
    "coif", "armet", "wanderers_hood", "leather_coat",
    "beggars_cloak", "researchers_cloak", "travelers_cloak",
    "archers_jacket", "iron_armor", "chainmail",
    "wanderers_belt", "belt_with_bag",
    "wooden_shield", "norman_shield", "iron_shield",
    "beggars_gloves", "chainmail_gloves",
    "poor_boots", "leather_boots", "hiking_boots",
    "iron_boots", "winter_boots",
];

/// Все торговцы игры.
pub const TRADERS: [TraderDef; 4] = [
    TraderDef {
        id: "marta",
        goods: CRAFT_GOODS,
    },
    TraderDef {
        id: "wanderer",
        #[rustfmt::skip]
        goods: &[
            // Ремесло и походное.
            "leather", "fiber", "clay", "quartz", "silver", "gems",
            "backpack", "wooden_bucket", "sickle", "scythe", "candle", "stone_block",
            "hiking_boots", "winter_boots", "leather_boots",
            // Карты локаций.
            "map_forest", "map_mines", "map_quarry", "map_ruins",
        ],
    },
    TraderDef {
        id: "smith",
        goods: CRAFT_GOODS,
    },
    TraderDef {
        id: "miner",
        goods: &[
            "iron",
            "silver",
            "quartz",
            "gems",
            "gold_ore",
            "iron_ingot",
            "gold_ingot",
            "silver_ingot",
            "stone_block",
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
        .map(TraderDef::name)
        .unwrap_or_else(|| i18n::t("trader.unknown"))
}

/// Имя торговца по его строковому id (`marta`, `wanderer`, ...).
pub fn trader_name_by_id(id: &str) -> &'static str {
    TRADERS
        .iter()
        .find(|t| t.id == id)
        .map(TraderDef::name)
        .unwrap_or_else(|| i18n::t("trader.unknown"))
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
            Self::Deficit => i18n::t("market.deficit"),
            Self::Balance => i18n::t("market.balance"),
            Self::Surplus => i18n::t("market.surplus"),
        }
    }

    /// Короткий статус для таблицы. В русском это «Д↑», в остальных языках —
    /// латинские буквы, чтобы совпадать с подписью под таблицей.
    pub fn short(self) -> &'static str {
        if !i18n::is_ru() {
            return match self {
                Self::Deficit => "D↑",
                Self::Balance => "B→",
                Self::Surplus => "S↓",
            };
        }
        match self {
            Self::Deficit => i18n::t("market.short.deficit"),
            Self::Balance => i18n::t("market.short.balance"),
            Self::Surplus => i18n::t("market.short.surplus"),
        }
    }

    /// Подсказка под таблицей с расшифровкой коротких статусов.
    pub fn legend(self) -> &'static str {
        if i18n::is_ru() {
            i18n::t("trade.legend_ru")
        } else {
            i18n::t("trade.legend")
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
///
/// Каменный блок — ходовой стройматериал, поэтому торговец не берёт на нём
/// свою наценку: цена идёт только от рынка (2% оставлены как заслон, чтобы
/// покупка не стала дешевле продажи).
pub fn buy_price(item: &Item, demand: u8, supply: u8) -> f64 {
    let markup = if item.id == "stone_block" {
        0.0
    } else {
        BUY_RATE - 1.0
    };
    let factor = (market_factor(demand, supply) + markup).clamp(1.02, 1.75);
    (item.base_price as f64 * factor).round().max(1.0)
}

/// Цена покупки с учётом уровня кузнеца: неразблокированный товар стоит в 5 раз дороже.
pub fn buy_price_for(item: &Item, demand: u8, supply: u8, smith_level: u32) -> f64 {
    let base = buy_price(item, demand, supply);
    if smith_level < items::item_unlock_level(&item.id) {
        (base * balance::LOCKED_ITEM_PRICE_MULT).round().max(1.0)
    } else {
        base
    }
}

/// Цена продажи: всегда ниже базовой, растёт от бонуса торговли.
pub fn sell_price(item: &Item, demand: u8, supply: u8, trade_bonus: f32) -> f64 {
    let factor = (SELL_RATE + (demand as f64 - supply as f64) / 300.0).clamp(0.30, 0.90);
    let bonus = 1.0 + trade_bonus as f64;
    (item.base_price as f64 * factor * bonus).round().max(1.0)
}

/// Цена, по которой торговец продаёт товар с витрины.
/// Если у товара есть `avg_cost` (транзитный или переполненный), цена
/// = `avg_cost * 1.5` (наценка +50%).
pub fn sell_price_for_good(item: &Item, good: &GoodState, trade_bonus: f32) -> f64 {
    if let Some(avg) = good.avg_cost {
        (avg * 1.5).round().max(1.0)
    } else {
        sell_price(item, good.demand, good.supply, trade_bonus)
    }
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
    /// Средняя цена покупки у игрока (для наценки при перепродаже).
    /// `None` — товар из обычного ассортимента (не транзит).
    pub avg_cost: Option<f64>,
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
            avg_cost: None,
        }
    }
}

impl GoodState {
    pub fn diff(&self) -> i32 {
        self.demand as i32 - self.supply as i32
    }

    /// Резервная карточка: вещь, которую игрок продал, а торговец держит
    /// на витрине до перепродажи. Такие позиции не входят в обычный
    /// ассортимент и не восстанавливают лимит скупки.
    pub fn is_reserve(&self) -> bool {
        self.avg_cost.is_some() && self.buy_limit == 0
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
    /// Кошелёк на скупку товара за цикл; `0` — ещё не инициализирован.
    pub wallet: f64,
    /// Момент следующего пополнения кошелька в `total_play_seconds`.
    pub wallet_at: f64,
}

impl TraderMarket {
    pub fn good(&self, item_id: &str) -> Option<&GoodState> {
        self.goods.iter().find(|good| good.item_id == item_id)
    }

    pub fn good_mut(&mut self, item_id: &str) -> Option<&mut GoodState> {
        self.goods.iter_mut().find(|good| good.item_id == item_id)
    }
}

/// Кошелёк торговца с ленивой инициализацией и пополнением раз в цикл.
pub fn wallet_of(state: &mut GameState, trader_index: usize) -> f64 {
    let now = state.total_play_seconds;
    let Some(market) = state.traders.get_mut(trader_index) else {
        return 0.0;
    };
    if market.wallet_at <= 0.0 || now >= market.wallet_at {
        market.wallet = TRADER_WALLET;
        market.wallet_at = now + TRADER_WALLET_INTERVAL;
    }
    market.wallet
}

/// Кошелёк торговца без изменений состояния (для отрисовки).
pub fn wallet_preview(state: &GameState, trader_index: usize) -> f64 {
    let market = match state.traders.get(trader_index) {
        Some(market) => market,
        None => return 0.0,
    };
    if market.wallet_at <= 0.0 || state.total_play_seconds >= market.wallet_at {
        TRADER_WALLET
    } else {
        market.wallet
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
        avg_cost: None,
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
            Self::Common => i18n::t("rarity.common"),
            Self::Uncommon => i18n::t("rarity.uncommon"),
            Self::Rare => i18n::t("rarity.rare"),
            Self::Epic => i18n::t("rarity.epic"),
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

/// Тир товара: определяет, как охотно торговец его скупает и сколько держит.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Базовые материалы: 20 / 50.
    Bulk,
    /// Дешёвая одежда / XP-фураж: 15 / 5.
    CheapGear,
    /// Среднее снаряжение: 8 / 3.
    MidGear,
    /// Сложное снаряжение: 5 / 2.
    HeavyGear,
    /// Эпик: 2 / 1.
    Epic,
}

impl Tier {
    /// `(buy_limit, stock)` для тира.
    pub fn values(self) -> (u32, u32) {
        match self {
            Self::Bulk => (20, 50),
            Self::CheapGear => (15, 5),
            Self::MidGear => (8, 3),
            Self::HeavyGear => (5, 2),
            Self::Epic => (2, 1),
        }
    }
}

/// Явная классификация предметов по тирам. По id, а не по цене —
/// потому что «дешёвая» куртка крафтится дольше, чем «дорогой» слиток.
pub fn tier_of(item_id: &str) -> Tier {
    match item_id {
        // Tier 1: базовые материалы и деревянные инструменты.
        "board" | "cloth" | "whetstone" | "brick" | "stone_block" | "jug" | "candle"
        | "wooden_bucket" | "wooden_shovel" | "wooden_shield" | "clay" | "leather" | "fiber"
        | "quartz" | "silver" | "gold_ore" | "gems" | "iron" | "coal" => Tier::Bulk,

        // Tier 2: дешёвая одежда (XP-фураж), обувь, шапки, робы, плащи.
        "robe" | "leather_gloves" | "winter_gloves" | "beggars_gloves" | "jacket"
        | "archers_jacket" | "leather_coat" | "beggars_cloak" | "leather_hat" | "wretched_hat"
        | "wanderers_hood" | "poor_boots" | "leather_boots" | "hiking_boots" | "winter_boots"
        | "backpack" => Tier::CheapGear,

        // Tier 3: среднее снаряжение (лёгкий огонь, кольца, пояса).
        "iron_bucket" | "sickle" | "scythe" | "hammer" | "iron_shovel" | "iron_bracers"
        | "war_hat" | "silver_ring" | "trader_ring" | "miner_amulet" | "wanderer_pendant"
        | "wanderers_belt" | "belt_with_bag" | "smith_ring" | "smith_amulet" | "smith_belt"
        | "silver_ingot" | "iron_ingot" => Tier::MidGear,

        // Tier 4: сложное снаряжение (много огня / много слитков).
        "iron_helmet" | "coif" | "chainmail_gloves" | "norman_shield" | "iron_shield"
        | "iron_boots" | "researchers_cloak" | "travelers_cloak" | "chainmail" | "iron_armor" => {
            Tier::HeavyGear
        }

        // Tier 5: эпик (эндгейм-крафт).
        "armet" | "gold_ring" | "gold_pendant" | "gold_ingot" => Tier::Epic,

        // Всё неизвестное — Bulk (не должно случаться, но безопасный дефолт).
        _ => Tier::Bulk,
    }
}

/// Норма запаса товара у торговца.
fn base_stock(item_id: &str) -> u32 {
    // Карты — особый случай, у них фиксированный запас.
    if item_id.starts_with("map_") {
        return 10;
    }
    tier_of(item_id).values().1
}

/// Норма лимита скупки товара за цикл.
fn base_buy_limit(item_id: &str) -> u32 {
    if item_id.starts_with("map_") {
        return 10;
    }
    tier_of(item_id).values().0
}

/// Свежий товар, появившийся в лавке.
///
/// Настроение рынка у новинки сразу случайное, поэтому новый товар может
/// прийти и в дефиците, и в балансе, и в избытке — витрина не выглядит
/// одинаково ровной.
fn fresh_good(seed: &mut u64, item_id: &str) -> GoodState {
    // Карты приходят уже в дефиците и с малым запасом.
    if item_id.starts_with("map_") {
        return good(item_id, 10, 100, 5, 10);
    }
    let trend = noise(seed, TREND_LIMIT);
    let demand = (50 + trend + noise(seed, 10)).clamp(5, 95) as u8;
    let supply = (50 - trend + noise(seed, 10)).clamp(5, 95) as u8;
    good(
        item_id,
        base_stock(item_id),
        demand,
        supply,
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
            good("stone_block", 150, 45, 55, 100),
            good("leather_hat", 3, 60, 40, 4),
            good("beggars_cloak", 3, 55, 45, 4),
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
            good("wanderers_hood", 2, 60, 40, 4),
            good("hiking_boots", 1, 70, 35, 2),
            good("candle", 30, 35, 65, 20),
            good("stone_block", 150, 45, 55, 100),
        ],
        // Кузнец: металл, доспехи и инструменты.
        2 => vec![
            good("iron", 80, 50, 60, 80),
            good("iron_ingot", 12, 50, 55, 8),
            good("gold_ingot", 1, 85, 25, 2),
            good("board", 100, 30, 70, 80),
            good("iron_bucket", 4, 50, 55, 5),
            good("iron_helmet", 1, 70, 35, 2),
            good("iron_shield", 1, 75, 30, 2),
            good("hammer", 2, 60, 40, 3),
            good("iron_bracers", 1, 70, 35, 2),
            good("stone_block", 150, 45, 55, 100),
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
            good("stone_block", 150, 45, 55, 100),
            good("miner_amulet", 1, 82, 25, 2),
        ],
    };

    TraderMarket {
        goods,
        wallet: TRADER_WALLET,
        wallet_at: 0.0,
    }
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
    ItemNotSellable,
    NothingToOffer,
    NoTransitSpace,
    /// У торговца кончился кошелёк на этот цикл.
    TraderBroke,
}

impl TradeError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownTrader => i18n::t("err.unknown_trader"),
            Self::UnknownItem => i18n::t("err.unknown_good"),
            Self::InvalidQuantity => i18n::t("err.bad_quantity"),
            Self::OutOfStock => i18n::t("err.out_of_stock"),
            Self::NotEnoughGold => i18n::t("err.no_gold"),
            Self::GoldCapReached => i18n::t("err.gold_cap"),
            Self::NothingToSell => i18n::t("err.nothing_to_sell"),
            Self::LimitReached => i18n::t("err.trade_limit"),
            Self::NoSpace => i18n::t("err.bag_full"),
            Self::ItemNotSellable => i18n::t("err.item_not_sellable"),
            Self::NothingToOffer => i18n::t("err.nothing_to_offer"),
            Self::NoTransitSpace => i18n::t("err.no_transit_space"),
            Self::TraderBroke => i18n::t("err.trader_broke"),
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

    let price = buy_price_for(item, good.demand, good.supply, state.smith_level());
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
    // Крупные партии ограничены не штучным лимитом, а кошельком торговца.
    let bulk = tier_of(item_id) == Tier::Bulk;
    if !bulk && quantity > good.buy_limit {
        return Err(TradeError::LimitReached);
    }

    let cap = state.gold_cap();
    let price = sell_price(item, good.demand, good.supply, state.trade_bonus)
        * state.debt_sell_multiplier();
    let total = price * quantity as f64;
    if state.gold + total > cap + 1e-9 {
        return Err(TradeError::GoldCapReached);
    }
    let wallet = wallet_of(state, trader_index);
    if total > wallet + 1e-9 {
        return Err(TradeError::TraderBroke);
    }

    if !state.remove_item(item_id, quantity) {
        return Err(TradeError::NothingToSell);
    }

    let before = state.gold;
    state.gold = (state.gold + total).min(cap);
    state.stats.gold_earned += state.gold - before;
    if let Some(market) = state.traders.get_mut(trader_index) {
        market.wallet = (market.wallet - total).max(0.0);
    }

    if let Some(good) = state
        .traders
        .get_mut(trader_index)
        .and_then(|market| market.good_mut(item_id))
    {
        good.stock = good.stock.saturating_add(quantity);
        if !bulk {
            good.buy_limit = good.buy_limit.saturating_sub(quantity);
        }
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

/// Сколько единиц можно продать с учётом лимита, кошелька торговца и места
/// под золото. `bulk` — крупный товар, у которого штучного лимита нет.
pub fn max_sell(
    owned: u32,
    buy_limit: u32,
    gold: f64,
    gold_cap: f64,
    price: f64,
    wallet: f64,
    bulk: bool,
) -> u32 {
    if price <= 0.0 {
        return 0;
    }
    let room = (gold_cap - gold).max(0.0);
    let by_gold = (room / price).floor() as u32;
    let by_wallet = (wallet.max(0.0) / price).floor() as u32;
    let by_limit = if bulk { u32::MAX } else { buy_limit };
    owned.min(by_limit).min(by_gold).min(by_wallet)
}

/// Продвигает рынок на `dt` секунд.
pub fn tick_markets(state: &mut GameState, dt: f64) {
    state.market_timer += dt.max(0.0);
    while state.market_timer + 1e-9 >= MARKET_TICK_SECONDS {
        state.market_timer -= MARKET_TICK_SECONDS;
        market_step(state);
    }
    // Кошельки торговцев пополняются раз в полчаса, как и налог.
    let now = state.total_play_seconds;
    for index in 0..state.traders.len() {
        let refresh = state
            .traders
            .get(index)
            .map(|market| market.wallet_at <= 0.0 || now >= market.wallet_at)
            .unwrap_or(false);
        if refresh {
            if let Some(market) = state.traders.get_mut(index) {
                market.wallet = TRADER_WALLET;
                market.wallet_at = now + TRADER_WALLET_INTERVAL;
            }
        }
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
    owned: &HashSet<String>,
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

    // «Умная ротация»: если такой товар лежит у игрока в сумке и его можно
    // продать, он получает тройной вес внутри своей редкости. Редкость при
    // этом выпадает по-прежнему по весам `roll_rarity`, иначе легендарки
    // стали бы выпадать почти так же часто, как обычные товары.
    let weight_of = |item_id: &'static str| -> usize {
        if owned.contains(item_id) {
            3
        } else {
            1
        }
    };

    let mut rarity = Some(roll_rarity(seed));
    while let Some(current) = rarity {
        let mut candidates: Vec<&'static str> = Vec::new();
        for item_id in &available {
            if items::item(item_id).map(GoodRarity::of) == Some(current) {
                for _ in 0..weight_of(item_id) {
                    candidates.push(item_id);
                }
            }
        }
        if !candidates.is_empty() {
            let pick = random_below(seed, candidates.len() as u64) as usize;
            return Some(candidates[pick]);
        }
        rarity = current.lower();
    }

    // Все свободные товары оказались выше выпавшей редкости: берём любой, но
    // «свои» предметы всё равно с тройным перевесом.
    let mut weighted: Vec<&'static str> = Vec::new();
    for item_id in available {
        for _ in 0..weight_of(item_id) {
            weighted.push(item_id);
        }
    }
    let pick = random_below(seed, weighted.len() as u64) as usize;
    Some(weighted[pick])
}

/// Шанс «чуда» у Странника: раз в ~100 шагов рынка появляется редкая карта.
const WANDERER_MIRACLE_CHANCE: u64 = 1;

/// С шансом [`ROTATION_PERCENT`] заменяет один товар в лавке.
fn rotate_market(owned: &HashSet<String>, seed: &mut u64, index: usize, market: &mut TraderMarket) {
    let Some(trader) = TRADERS.get(index) else {
        return;
    };
    if trader.goods.is_empty() || market.goods.is_empty() {
        return;
    }

    // «Чудо» у Странника: с шансом 1% ставим редкую карту, если её нет
    // на витрине. Это единственный способ получить карту каменоломни или
    // руин у торговца — обычная ротация их почти не выбирает.
    if trader.id == "wanderer" && random_below(seed, 100) < WANDERER_MIRACLE_CHANCE {
        let miracle_cards = ["map_quarry", "map_ruins"];
        let candidates: Vec<&'static str> = miracle_cards
            .iter()
            .copied()
            .filter(|item_id| market.good(item_id).is_none())
            .collect();
        if !candidates.is_empty() {
            let pick = random_below(seed, candidates.len() as u64) as usize;
            // Резервные карточки не вытесняем: их место — на витрине.
            let slots: Vec<usize> = market
                .goods
                .iter()
                .enumerate()
                .filter(|(_, good)| !good.is_reserve())
                .map(|(index, _)| index)
                .collect();
            if slots.is_empty() {
                return;
            }
            let slot = slots[random_below(seed, slots.len() as u64) as usize];
            market.goods[slot] = fresh_good(seed, candidates[pick]);
            return;
        }
    }

    if random_below(seed, 100) >= ROTATION_PERCENT {
        return;
    }
    let Some(item_id) = pick_rotation_item(owned, seed, trader.goods, market) else {
        // Свободных товаров в пуле нет: витрина уже показывает весь
        // ассортимент (у Странника это четыре карты). Тогда убираем с
        // витрины товар, которого в пуле нет, — иначе старый ассортимент
        // висел бы в лавке навсегда и не давал ротации.
        if let Some(position) = market
            .goods
            .iter()
            .position(|good| !trader.goods.contains(&good.item_id.as_str()))
        {
            market.goods.remove(position);
        }
        return;
    };
    let slot = {
        let slots: Vec<usize> = market
            .goods
            .iter()
            .enumerate()
            .filter(|(_, good)| !good.is_reserve())
            .map(|(index, _)| index)
            .collect();
        if slots.is_empty() {
            return;
        }
        slots[random_below(seed, slots.len() as u64) as usize]
    };
    market.goods[slot] = fresh_good(seed, item_id);
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

    // «Умная ротация»: торговцы «видят» то, что игрок скрафтил и может
    // продать. Снимок делаем до изменяемого обхода `traders`: иначе `state`
    // был бы занят изменяемым заимствованием.
    let owned: HashSet<String> = state
        .inventory
        .iter()
        .filter(|(item_id, count)| **count > 0 && items::is_sellable_from_bag(state, item_id))
        .map(|(item_id, _)| item_id.clone())
        .collect();

    for (index, market) in state.traders.iter_mut().enumerate() {
        if market.goods.is_empty() {
            market.goods = default_market(index).goods;
        }
        rotate_market(&owned, &mut seed, index, market);

        for good in market.goods.iter_mut() {
            // Резервная карточка (вещь игрока): лимит скупки не растёт,
            // запас не восстанавливается. Иначе она навсегда застряла бы
            // в ассортименте. NPC изредка перекупают такую вещь.
            if good.is_reserve() {
                if good.stock > 0 && random_below(&mut seed, 100) < 10 {
                    good.stock -= 1;
                }
                continue;
            }

            // Карты всегда в дефиците, запас и лимит скупки — не больше 10.
            if good.item_id.starts_with("map_") {
                good.demand = 100;
                good.supply = 5;
                good.trend = 0;
                good.stock = good.stock.min(10);
                good.buy_limit = good.buy_limit.min(10);
                continue;
            }

            // Одежда и снаряжение: дефицит, NPC скупают остатки.
            // Bulk-материалы (Tier 1) остаются на обычной ротации.
            let tier = tier_of(&good.item_id);
            if !matches!(tier, Tier::Bulk) {
                good.demand = (good.demand as i32 + noise(&mut seed, 3)).clamp(80, 100) as u8;
                good.supply = (good.supply as i32 + noise(&mut seed, 2)).clamp(5, 15) as u8;
                good.trend = 25;

                // NPC скупают товар (шанс 30%).
                if good.stock > 0 && random_below(&mut seed, 100) < 30 {
                    good.stock -= 1;
                }
                // Медленное восстановление до 2 штук (шанс 15%).
                if good.stock < 2 && random_below(&mut seed, 100) < 15 {
                    good.stock += 1;
                }

                let norm = base_buy_limit(&good.item_id);
                if good.buy_limit < norm && random_below(&mut seed, 100) < 20 {
                    good.buy_limit += 1;
                }
                continue;
            }

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

    // Удаляем резервные карточки, которые полностью распроданы.
    for market in state.traders.iter_mut() {
        market.goods.retain(|g| !(g.stock == 0 && g.is_reserve()));
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
pub(crate) fn random_below(seed: &mut u64, bound: u64) -> u64 {
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

/// Разбивка предложения: сколько штук торговец возьмёт по нормальной цене,
/// а сколько — как неходовой товар. Нужна и для расчёта, и для подсказок в UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfferBreakdown {
    /// Штук по нормальной цене.
    pub normal_qty: u32,
    /// Штук по junk-цене.
    pub junk_qty: u32,
    /// Цена за штуку по нормальному тарифу.
    pub normal_unit: f64,
    /// Цена за штуку по junk-тарифу.
    pub junk_unit: f64,
    /// Рыночная цена за штуку (до скидок и наценок).
    pub market_unit: f64,
}

/// Считает разбивку предложения без ажиотажа и торга.
pub fn offer_breakdown(
    state: &GameState,
    trader_idx: usize,
    item_id: &str,
    quantity: u32,
) -> OfferBreakdown {
    let empty = OfferBreakdown {
        normal_qty: 0,
        junk_qty: quantity,
        normal_unit: 0.0,
        junk_unit: 0.0,
        market_unit: 0.0,
    };
    let Some(item) = items::item(item_id) else {
        return empty;
    };
    let Some(trader) = TRADERS.get(trader_idx) else {
        return empty;
    };

    let (demand, supply, current_stock) = state
        .traders
        .get(trader_idx)
        .and_then(|market| market.good(item_id))
        .map(|good| (good.demand, good.supply, good.stock))
        .unwrap_or((50, 50, 0));

    // Цена за штуку без учёта junk/нормы.
    let market_unit =
        sell_price(item, demand, supply, state.trade_bonus) * state.debt_sell_multiplier();

    // Профильный ли торговец этим товаром?
    let is_native = trader.goods.contains(&item_id);

    // Лимит транзита (для профильного — ×2, для непрофильного — 0).
    let transit_limit = if is_native {
        base_buy_limit(item_id) * 2
    } else {
        0
    };

    // Склад переполнен? Тогда все последующие продажи — по junk.
    let overflow = current_stock > base_stock(item_id);

    // Сколько штук по норме: остаток транзитного лимита.
    let normal_quota = if is_native && !overflow {
        transit_limit.saturating_sub(current_stock)
    } else {
        0
    };

    let normal_qty = quantity.min(normal_quota);
    OfferBreakdown {
        normal_qty,
        junk_qty: quantity - normal_qty,
        // Профильный: -10%, непрофильный (junk) или переполнение: -35%.
        normal_unit: (market_unit * 0.9).max(1.0),
        junk_unit: (market_unit * 0.65).max(1.0),
        market_unit,
    }
}

/// Оценка стартовой цены, которую торговец предложит за товар.
/// Считает цену за партию. Возвращает `OfferQuote` с разбивкой на части:
/// профильный товар идёт по нормальной цене до лимита, остаток — по junk.
pub fn evaluate_offer(
    state: &mut GameState,
    trader_idx: usize,
    item_id: &str,
    quantity: u32,
) -> Result<OfferQuote, TradeError> {
    if quantity == 0 {
        return Err(TradeError::InvalidQuantity);
    }
    let item = items::item(item_id).ok_or(TradeError::UnknownItem)?;
    if !items::is_sellable_from_bag(state, item_id) {
        return Err(TradeError::ItemNotSellable);
    }
    let trader = TRADERS.get(trader_idx).ok_or(TradeError::UnknownTrader)?;
    if state.item_count(item_id) == 0 {
        return Err(TradeError::NothingToOffer);
    }
    if state.item_count(item_id) < quantity {
        return Err(TradeError::NothingToSell);
    }

    let breakdown = offer_breakdown(state, trader_idx, item_id, quantity);
    let base_unit = breakdown.market_unit;
    let is_native = trader.goods.contains(&item_id);

    let mut parts = Vec::new();
    if breakdown.normal_qty > 0 {
        parts.push(OfferPart {
            quantity: breakdown.normal_qty,
            unit_price: breakdown.normal_unit,
            is_junk: false,
        });
    }
    if breakdown.junk_qty > 0 {
        parts.push(OfferPart {
            quantity: breakdown.junk_qty,
            unit_price: breakdown.junk_unit,
            is_junk: true,
        });
    }

    let mut quote = OfferQuote { total: 0.0, parts };
    quote.recalc();

    // Механика Ажиотажа: если другой торговец уже давал больше, перебиваем.
    // Для junk-товара (непрофильного) ажиотаж слабый (×1.02), независимо
    // от суммы: торговец не сражается за то, что ему не нужно.
    let total_base_value = item.base_price as f64 * quantity as f64;
    if let Some(best) = &state.best_offer {
        if best.item_id == item_id && best.trader_index != trader_idx {
            let best_unit = best.price / best.quantity.max(1) as f64;
            let mut multiplier = None;
            if !is_native {
                // Junk: только 10% шанс, что перебьёт, и то всего на ×1.02.
                let mut seed = state.market_seed;
                if random_below(&mut seed, 100) < 10 {
                    multiplier = Some(1.02);
                }
                state.market_seed = seed;
            } else if total_base_value > 2000.0 {
                multiplier = Some(1.10);
            } else if total_base_value > 500.0 {
                let mut seed = state.market_seed;
                if random_below(&mut seed, 100) < 50 {
                    multiplier = Some(1.05);
                }
                state.market_seed = seed;
            } else {
                let mut seed = state.market_seed;
                if random_below(&mut seed, 100) < 10 {
                    multiplier = Some(1.02);
                }
                state.market_seed = seed;
            }
            if let Some(multiplier) = multiplier {
                let counter = (best_unit * multiplier).round().max(1.0) * quantity as f64;
                if counter > quote.total {
                    let ratio = counter / quote.total.max(1.0);
                    for part in quote.parts.iter_mut() {
                        part.unit_price = (part.unit_price * ratio).round().max(1.0);
                    }
                    quote.recalc();
                }
            }
        }
    }

    // Потолок: не больше +50% от рыночной цены партии.
    let market_total = (base_unit * quantity as f64).round().max(1.0);
    let cap = (market_total * 1.5).round();
    if quote.total > cap {
        let ratio = cap / quote.total;
        for part in quote.parts.iter_mut() {
            part.unit_price = (part.unit_price * ratio).round().max(1.0);
        }
        quote.recalc();
    }

    Ok(quote)
}

/// Финальная продажа предмета торговцу через "Предложить товар".
pub fn execute_offer(
    state: &mut GameState,
    trader_idx: usize,
    item_id: &str,
    quantity: u32,
    final_price: f64,
) -> Result<(), TradeError> {
    if quantity == 0 {
        return Err(TradeError::InvalidQuantity);
    }
    let item = items::item(item_id).ok_or(TradeError::UnknownItem)?;
    if !items::is_sellable_from_bag(state, item_id) {
        return Err(TradeError::ItemNotSellable);
    }
    if state.item_count(item_id) < quantity {
        return Err(TradeError::NothingToSell);
    }
    if final_price <= 0.0 {
        return Err(TradeError::InvalidQuantity);
    }

    let trader = TRADERS.get(trader_idx).ok_or(TradeError::UnknownTrader)?;
    let is_native = trader.goods.contains(&item_id);

    // Транзитных карточек (вещей игрока) на витрине не больше двух. Проверяем
    // это до списания предмета: иначе отказ съедал бы товар, а золото уже
    // было бы начислено. Уже выставленный товар просто обновляем.
    let market_goods = &state
        .traders
        .get(trader_idx)
        .ok_or(TradeError::UnknownTrader)?
        .goods;
    let already_listed = market_goods.iter().any(|good| good.item_id == item_id);
    let reserve_count = market_goods.iter().filter(|good| good.is_reserve()).count();
    if !already_listed && reserve_count >= 2 {
        return Err(TradeError::NoTransitSpace);
    }

    let cap = state.gold_cap();
    if state.gold + final_price > cap + 1e-9 {
        return Err(TradeError::GoldCapReached);
    }
    let wallet = wallet_of(state, trader_idx);
    if final_price > wallet + 1e-9 {
        return Err(TradeError::TraderBroke);
    }

    if !state.remove_item(item_id, quantity) {
        return Err(TradeError::NothingToSell);
    }

    let before = state.gold;
    state.gold = (state.gold + final_price).min(cap);
    state.stats.gold_earned += state.gold - before;

    let unit_cost = final_price / quantity as f64;

    // Обновляем витрину.
    let market = state
        .traders
        .get_mut(trader_idx)
        .ok_or(TradeError::UnknownTrader)?;
    market.wallet = (market.wallet - final_price).max(0.0);

    if let Some(good) = market.good_mut(item_id) {
        // Товар уже есть на витрине (родной или транзитный) — обновляем его.
        let old_stock = good.stock as f64;
        let old_avg = good.avg_cost.unwrap_or(good_price(item, good));
        let new_avg =
            (old_avg * old_stock + unit_cost * quantity as f64) / (old_stock + quantity as f64);
        good.stock = good.stock.saturating_add(quantity);
        good.avg_cost = Some(new_avg);
        if !is_native {
            good.buy_limit = 0;
        }
        let impact = (quantity * 4).min(80) as u8;
        good.supply = good.supply.saturating_add(impact).min(100);
        good.demand = good.demand.saturating_sub(impact).max(5);
    } else {
        // Новая позиция — резервная карточка. Место на витрине уже проверено
        // выше; карточка висит, пока её не купят.
        let new_good = GoodState {
            item_id: item_id.to_string(),
            stock: quantity,
            demand: 70,
            supply: 30,
            trend: 0,
            // buy_limit = 0: торговец больше не скупает этот товар, пока
            // карточка не уйдёт с витрины.
            buy_limit: 0,
            avg_cost: Some(unit_cost),
        };
        market.goods.push(new_good);
    }

    // Запоминаем лучшее предложение.
    state.best_offer = Some(crate::game::state::BestOffer {
        item_id: item_id.to_string(),
        trader_index: trader_idx,
        price: final_price,
        quantity,
    });

    Ok(())
}

/// Крупный ли товар: у него нет штучного лимита скупки, ограничивает кошелёк.
pub fn is_bulk(item_id: &str) -> bool {
    tier_of(item_id) == Tier::Bulk
}

/// Базовая рыночная цена товара (без учёта avg_cost).
/// Используется только для расчёта средней при первом выставлении.
fn good_price(item: &Item, good: &GoodState) -> f64 {
    sell_price(item, good.demand, good.supply, 0.0)
}

/// Сколько штук этого товара можно предложить торговцу через
/// «Предложить товар». В отличие от старой вкладки «Продать», здесь
/// лимитов нет — игрок может предложить всю партию.
pub fn max_offer(state: &GameState, _trader_index: usize, item_id: &str) -> u32 {
    state.item_count(item_id)
}

/// Одна часть сделки: сколько штук по какой цене.
#[derive(Clone, Debug, PartialEq)]
pub struct OfferPart {
    pub quantity: u32,
    pub unit_price: f64,
    /// `true` — цена по junk-тарифу (переполнение или непрофильный товар).
    pub is_junk: bool,
}

/// Полная котировка сделки: сумма + разбивка по частям.
#[derive(Clone, Debug, PartialEq)]
pub struct OfferQuote {
    pub total: f64,
    pub parts: Vec<OfferPart>,
}

impl OfferQuote {
    /// Пересчитывает total по текущим частям.
    pub fn recalc(&mut self) {
        self.total = self
            .parts
            .iter()
            .map(|p| p.unit_price * p.quantity as f64)
            .sum();
    }
}

/// Диапазон цены для торга: (min, max) от текущего предложения торговца.
/// min — минимальная цена, ниже которой торговец не пойдёт.
/// max — потолок, выше которого он не поднимется.
pub fn haggle_range(current: f64) -> (f64, f64) {
    let min = (current * 0.75).round().max(1.0);
    let max = (current * 1.5).round().max(min + 1.0);
    (min, max)
}

/// Шанс (0..100), что торговец согласится на `target`.
///
/// Чем ближе `target` к `current` — тем проще. Чем ближе к `max` — тем
/// сложнее. Если игрок просит **меньше** текущей цены — торговец всегда
/// согласится (это его выгода).
pub fn haggle_chance(
    state: &GameState,
    current: f64,
    target: f64,
    min: f64,
    max: f64,
    is_junk: bool,
) -> u64 {
    // Базовый шанс.
    let mut chance: f64 = if target <= current {
        // Игрок сам снижает — торговцу это выгодно, согласится всегда.
        98.0
    } else {
        // Насколько «нагло» игрок просит: 0 при current, 1 при max.
        let span = (max - current).max(1.0);
        let ratio = ((target - current) / span).clamp(0.0, 1.0);
        // От 90% (только-только выше) до 5% (упёрся в потолок).
        90.0 - ratio * 85.0
    };

    // Бонусы экипировки.
    chance += (state.trade_bonus as f64 * 100.0).clamp(0.0, 40.0);
    chance += (state.haggle_bonus as f64 * 100.0).clamp(0.0, 25.0);

    // Junk-цена: сложнее торговаться за неинтересный товар.
    if is_junk {
        chance -= 15.0;
    }

    // Ограничения.
    chance = chance.clamp(3.0, 98.0);
    // min здесь не используется для формулы, но оставим параметр —
    // в будущем можно учитывать «насколько ниже min пытается уйти игрок».
    let _ = min;

    chance.round() as u64
}

/// Результат броска торга.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HaggleOutcome {
    /// Торговец согласился на `target`.
    Accepted(f64),
    /// Торговец отказался, но предлагает компромисс — новую цену.
    Rejected(f64),
}

/// Разрешает предложение игрока: бросок кубика, при неудаче — компромисс.
///
/// Возвращает новую цену и результат. Не мутирует `state` (кроме
/// `market_seed` для последующих бросков).
pub fn resolve_haggle(
    state: &mut GameState,
    current: f64,
    target: f64,
    min: f64,
    max: f64,
    is_junk: bool,
) -> HaggleOutcome {
    let chance = haggle_chance(state, current, target, min, max, is_junk);
    let mut seed = state.market_seed;
    let roll = random_below(&mut seed, 100);
    state.market_seed = seed;

    if roll < chance {
        HaggleOutcome::Accepted(target.clamp(min, max))
    } else {
        // Компромисс: торговец идёт на 40% пути от current к target.
        let compromise = current + (target - current) * 0.4;
        let new_price = compromise.clamp(min, max).round();
        HaggleOutcome::Rejected(new_price)
    }
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
            market.good("leather_hat").map(GoodState::status),
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
            bank_level: 10,
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
    fn wallet_limits_bulk_sales_and_refills_each_cycle() {
        let mut state = GameState {
            bank_level: 10,
            ..GameState::default()
        };
        state.add_item("stone_block", 160);

        // 120 блоков по 23 золота — 2760 из 3000, кошелёк почти пуст.
        assert_eq!(sell(&mut state, 0, "stone_block", 120), Ok(()));
        assert!(state.traders[0].wallet < TRADER_WALLET);
        // После обвала цены 30 блоков стоят 360 — кошелька не хватает.
        assert_eq!(
            sell(&mut state, 0, "stone_block", 30),
            Err(TradeError::TraderBroke)
        );

        state.total_play_seconds = TRADER_WALLET_INTERVAL + 1.0;
        assert_eq!(sell(&mut state, 0, "stone_block", 1), Ok(()));
        assert!(state.traders[0].wallet < TRADER_WALLET);
    }

    #[test]
    fn bulk_sale_is_limited_by_the_wallet_not_by_pieces() {
        let mut state = GameState {
            bank_level: 10,
            ..GameState::default()
        };
        state.add_item("board", 40);
        if let Some(good) = state
            .traders
            .get_mut(0)
            .and_then(|market| market.good_mut("board"))
        {
            good.buy_limit = 1;
        }

        assert_eq!(sell(&mut state, 0, "board", 20), Ok(()));
        assert_eq!(
            state
                .traders
                .get(0)
                .and_then(|market| market.good("board"))
                .map(|good| good.buy_limit),
            Some(1)
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
        for market in &state.traders {
            for good in &market.goods {
                if good.item_id.starts_with("map_") {
                    assert_eq!((good.demand, good.supply), (100, 5));
                    continue;
                }
                assert!((5..=95).contains(&good.demand));
                assert!((5..=95).contains(&good.supply));
            }
        }
    }

    #[test]
    fn max_helpers_respect_stock_and_limits() {
        assert_eq!(max_buy(100.0, 12, 45.0), 2);
        assert_eq!(max_buy(100.0, 1, 45.0), 1);
        assert_eq!(max_buy(10.0, 12, 45.0), 0);
        assert_eq!(max_sell(8, 3, 0.0, 1000.0, 45.0, 3000.0, false), 3);
        assert_eq!(max_sell(2, 3, 0.0, 1000.0, 45.0, 3000.0, false), 2);
        // Место под золото ограничивает продажу.
        assert_eq!(max_sell(8, 8, 970.0, 1000.0, 45.0, 3000.0, false), 0);
        assert_eq!(max_sell(8, 8, 920.0, 1000.0, 45.0, 3000.0, false), 1);
        // Кошелёк торговца ограничивает крупные партии.
        assert_eq!(max_sell(100, 0, 0.0, 1000.0, 10.0, 55.0, true), 5);
        // У bulk-товара штучный лимит не мешает.
        assert_eq!(max_sell(100, 0, 0.0, 1000.0, 10.0, 3000.0, true), 100);
    }

    #[test]
    fn locked_goods_cost_five_times_more_until_smith_level() {
        let robe = items::item("robe").expect("роба");
        let base = buy_price(robe, 50, 50);
        assert_eq!(buy_price_for(robe, 50, 50, 1), (base * 5.0).round());
        assert_eq!(buy_price_for(robe, 50, 50, 6), base);

        // Железный слиток открыт сразу после кузницы — наценки нет.
        let ingot = items::item("iron_ingot").expect("слиток");
        assert_eq!(buy_price_for(ingot, 50, 50, 1), buy_price(ingot, 50, 50));
    }

    #[test]
    fn player_sales_flood_the_market() {
        let mut state = GameState {
            gold: 0.0,
            ..GameState::default()
        };
        state.add_item("board", 80);

        // Bulk-лимит = 20, значит за одну сделку продаём 20.
        assert_eq!(sell(&mut state, 0, "board", 20), Ok(()));

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
    fn fresh_goods_come_with_different_market_moods() {
        let mut seed = 42;
        let mut deficits = 0;
        let mut surpluses = 0;
        let mut balances = 0;
        for _ in 0..300 {
            match fresh_good(&mut seed, "board").status() {
                MarketStatus::Deficit => deficits += 1,
                MarketStatus::Surplus => surpluses += 1,
                MarketStatus::Balance => balances += 1,
            }
        }
        assert!(
            deficits > 30,
            "новинки почти не бывают в дефиците: {deficits}"
        );
        assert!(
            surpluses > 30,
            "новинки почти не бывают в избытке: {surpluses}"
        );
        assert!(
            balances > 30,
            "новинки почти не бывают в балансе: {balances}"
        );
    }

    #[test]
    fn market_recovers_gradually_after_flood() {
        let mut state = GameState::default();
        // Все товары Марты уже на витрине — ротация не сможет ничего заменить.
        let mut market = TraderMarket {
            goods: Vec::new(),
            ..TraderMarket::default()
        };
        let mut seed = 7;
        for item_id in TRADERS[0].goods {
            market.goods.push(fresh_good(&mut seed, item_id));
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
        // Но если в пуле торговца меньше товаров, чем помещается на витрину,
        // ротация почти не срабатывает: смен меньше. Ослабляем порог.
        for (index, count) in changes.iter().enumerate() {
            let pool_size = TRADERS[index].goods.len();
            let min_changes = if pool_size <= 4 { 1 } else { 20 };
            assert!(
                *count >= min_changes,
                "рынок {index} почти не меняется: {count} смен за 200 шагов (пул {pool_size})"
            );
        }
    }

    #[test]
    fn stone_block_has_no_trader_markup() {
        let block = item("stone_block");
        let base = block.base_price as f64;
        // Без наценки 1.15: в балансе остаётся только 2% закупки.
        assert_eq!(buy_price(block, 50, 50), (base * 1.02).round());
        // Остальные товары наценку сохраняют.
        let ring = item("trader_ring");
        assert!(buy_price(ring, 50, 50) > ring.base_price as f64 * 1.1);
    }

    #[test]
    fn stone_block_is_sold_by_every_trader_in_bulk() {
        assert!(items::item("stone_block").is_some(), "нет каменного блока");
        for (index, trader) in TRADERS.iter().enumerate() {
            assert!(
                trader.goods.contains(&"stone_block"),
                "у торговца {index} нет каменного блока"
            );
            assert!(
                default_market(index).good("stone_block").is_some(),
                "у торговца {index} блок не на витрине"
            );
        }
        assert_eq!(base_stock("stone_block"), 50);
        assert_eq!(base_buy_limit("stone_block"), 20);
    }

    #[test]
    fn wanderer_shelf_drops_goods_outside_his_pool() {
        let mut state = GameState::default();
        for _ in 0..1_000 {
            market_step(&mut state);
        }

        let market = &state.traders[1];
        assert!(!market.goods.is_empty(), "лавка Странника опустела");
        for good in &market.goods {
            assert!(
                TRADERS[1].goods.contains(&good.item_id.as_str()),
                "в лавке Странника остался чужой товар: {}",
                good.item_id
            );
        }
    }

    #[test]
    fn rotation_picks_only_missing_goods() {
        let market = default_market(0);
        let owned = HashSet::new();
        let mut seed = 99;
        for _ in 0..1000 {
            let picked = pick_rotation_item(&owned, &mut seed, TRADERS[0].goods, &market)
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

    #[test]
    fn smart_rotation_weights_goods_the_player_can_sell() {
        let market = TraderMarket {
            goods: Vec::new(),
            ..TraderMarket::default()
        };
        let empty = HashSet::new();
        let mut owned = HashSet::new();
        owned.insert("jug".to_string());

        let count = |owned: &HashSet<String>| {
            let mut seed = 1234u64;
            (0..5000)
                .filter(|_| {
                    pick_rotation_item(owned, &mut seed, TRADERS[0].goods, &market) == Some("jug")
                })
                .count()
        };

        // «Свой» товар должен появляться на витрине заметно чаще.
        assert!(
            count(&owned) > count(&empty),
            "товар из сумки не получает тройной вес"
        );
    }

    #[test]
    fn offer_price_scales_with_quantity() {
        let mut state = GameState::default();
        state.add_item("jug", 5);

        let one = evaluate_offer(&mut state, 0, "jug", 1)
            .expect("оценка за одну штуку")
            .total;
        let three = evaluate_offer(&mut state, 0, "jug", 3)
            .expect("оценка за три штуки")
            .total;
        assert!(three > one, "за партию платят больше, чем за штуку");
        assert!(
            three >= one * 2.5,
            "цена партии не должна равняться цене одной штуки: {one} vs {three}"
        );
    }

    #[test]
    fn offer_rejects_unsellable_and_missing_items() {
        let mut state = GameState::default();
        state.add_item("board", 10);
        state.add_item("jug", 2);

        // Доски — обработанный материал, но торговцу их не предлагают.
        assert_eq!(
            evaluate_offer(&mut state, 0, "board", 1),
            Err(TradeError::ItemNotSellable)
        );
        // Сырьё тоже нельзя.
        assert_eq!(
            evaluate_offer(&mut state, 0, "iron", 1),
            Err(TradeError::ItemNotSellable)
        );
        // Больше, чем лежит в сумке, не оценивают.
        assert_eq!(
            evaluate_offer(&mut state, 0, "jug", 3),
            Err(TradeError::NothingToSell)
        );
        // Чего нет в сумке — предлагать нечего.
        assert_eq!(
            evaluate_offer(&mut state, 0, "candle", 1),
            Err(TradeError::NothingToOffer)
        );
    }

    #[test]
    fn haggle_chance_drops_as_the_ask_grows() {
        let mut state = GameState::default();
        state.add_item("jug", 10);
        let current = evaluate_offer(&mut state, 0, "jug", 2)
            .expect("оценка")
            .total;
        let (min, max) = haggle_range(current);

        // Просить меньше текущей цены — почти гарантированный успех.
        let low = haggle_chance(&mut state, current, current * 0.9, min, max, false);
        // Упереться в потолок — почти невозможно.
        let high = haggle_chance(&mut state, current, max, min, max, false);
        assert_eq!(low, 98, "снижение цены должно проходить всегда");
        assert!(high <= 10, "потолок должен быть рискованным: {high}");
        assert!(low > high);
    }

    #[test]
    fn haggle_range_brackets_the_current_price() {
        let (min, max) = haggle_range(1000.0);
        assert_eq!(min, 750.0);
        assert_eq!(max, 1500.0);
        // Совсем маленькая цена не схлопывает диапазон в точку.
        let (min, max) = haggle_range(1.0);
        assert!(min >= 1.0);
        assert!(max > min);
    }

    #[test]
    fn resolved_haggle_stays_inside_the_range() {
        let mut state = GameState::default();
        state.add_item("jug", 10);
        let current = evaluate_offer(&mut state, 0, "jug", 2)
            .expect("оценка")
            .total;
        let (min, max) = haggle_range(current);

        for _ in 0..200 {
            let price = match resolve_haggle(&mut state, current, max, min, max, false) {
                HaggleOutcome::Accepted(price) | HaggleOutcome::Rejected(price) => price,
            };
            assert!(
                price >= min - 1e-9 && price <= max + 1e-9,
                "торг вышел за диапазон: {price} не в {min}..{max}"
            );
        }
    }

    #[test]
    fn executing_an_offer_pays_and_removes_the_goods() {
        let mut state = GameState::default();
        state.gold = 100.0;
        state.add_item("jug", 3);

        let price = evaluate_offer(&mut state, 0, "jug", 2)
            .expect("оценка")
            .total;
        assert_eq!(execute_offer(&mut state, 0, "jug", 2, price), Ok(()));
        assert_eq!(state.item_count("jug"), 1);
        assert!((state.gold - (100.0 + price)).abs() < 1e-9);

        let best = state
            .best_offer
            .as_ref()
            .expect("лучшее предложение сохранено");
        assert_eq!(best.trader_index, 0);
        assert_eq!(best.item_id, "jug");
        assert_eq!(best.quantity, 2);

        // Остаток меньше запрошенного — сделка не проходит.
        assert_eq!(
            execute_offer(&mut state, 0, "jug", 2, price),
            Err(TradeError::NothingToSell)
        );
    }

    #[test]
    fn sold_goods_stay_as_two_reserve_cards() {
        // Марта держит на витрине не больше двух вещей игрока. Третий товар
        // она не берёт — иначе витрина разрасталась бы с каждой сделкой.
        let mut state = GameState::default();
        let before = state.traders[0].goods.len();
        let items = ["archers_jacket", "leather_coat", "winter_gloves"];
        for item_id in items {
            state.add_item(item_id, 1);
        }

        for (sold, item_id) in items.iter().enumerate() {
            // Убираем родную карточку, чтобы продажа создавала новую.
            state.traders[0]
                .goods
                .retain(|good| good.item_id != *item_id);
            let quote = evaluate_offer(&mut state, 0, item_id, 1).expect("оценка");
            let result = execute_offer(&mut state, 0, item_id, 1, quote.total);
            if sold < 2 {
                assert_eq!(result, Ok(()), "первые две вещи должны продаться");
            } else {
                assert_eq!(result, Err(TradeError::NoTransitSpace));
                assert_eq!(
                    state.item_count(item_id),
                    1,
                    "отказ не должен съедать товар"
                );
            }
        }

        let market = &state.traders[0];
        assert!(
            market.goods.len() <= before + 2,
            "витрина выросла до {} позиций",
            market.goods.len()
        );
        assert_eq!(
            market.goods.iter().filter(|good| good.is_reserve()).count(),
            2,
            "резервных карточек должно быть ровно две"
        );
    }

    #[test]
    fn haggling_has_a_ceiling() {
        let mut state = GameState::default();
        state.add_item("jug", 10);
        let current = evaluate_offer(&mut state, 0, "jug", 2)
            .expect("оценка")
            .total;
        let (min, max) = haggle_range(current);
        // Спамим торгом на потолок: цена не должна выйти за max.
        let mut price = current;
        for _ in 0..300 {
            price = match resolve_haggle(&mut state, price, max, min, max, false) {
                HaggleOutcome::Accepted(price) | HaggleOutcome::Rejected(price) => price,
            };
        }
        assert!(price <= max + 1e-9, "торг разогнал цену: {price} > {max}");
    }

    #[test]
    fn rival_offer_is_compared_per_unit_and_capped() {
        use crate::game::state::BestOffer;

        let mut state = GameState::default();
        state.add_item("gold_pendant", 1);
        let base = evaluate_offer(&mut state, 2, "gold_pendant", 1)
            .expect("оценка")
            .total;

        // Соперник якобы дал огромную сумму за партию из десяти штук.
        state.best_offer = Some(BestOffer {
            item_id: "gold_pendant".to_string(),
            trader_index: 0,
            price: base * 10.0,
            quantity: 10,
        });

        let mut counter = 0.0_f64;
        for _ in 0..50 {
            counter = counter.max(
                evaluate_offer(&mut state, 2, "gold_pendant", 1)
                    .expect("контроценка")
                    .total,
            );
        }
        // Сравнение идёт за штуку: перебиваем на 5%, а не в десять раз.
        assert!(counter > base, "соперника не перебили");
        assert!(
            counter <= (base * 1.9).round(),
            "ажиотаж разогнал цену: {counter} при базе {base}"
        );
    }

    #[test]
    fn transient_goods_cap_at_two() {
        let mut state = GameState::default();
        // У Рудокопа нет курток, перчаток, шапок в профиле — все будут транзитными.
        let items = ["jacket", "leather_gloves", "wretched_hat"];
        for item_id in items {
            state.add_item(item_id, 5);
        }
        for (sold, item_id) in items.iter().enumerate() {
            let quote = evaluate_offer(&mut state, 3, item_id, 5).expect("оценка");
            let result = execute_offer(&mut state, 3, item_id, 5, quote.total);
            if sold < 2 {
                assert!(result.is_ok(), "первые две вещи должны продаться");
            } else {
                assert_eq!(result, Err(TradeError::NoTransitSpace));
                assert_eq!(
                    state.item_count(item_id),
                    5,
                    "отказ не должен съедать товар"
                );
            }
        }
        let market = &state.traders[3];
        let transient: Vec<_> = market
            .goods
            .iter()
            .filter(|g| g.buy_limit == 0 && g.avg_cost.is_some())
            .collect();
        assert_eq!(
            transient.len(),
            2,
            "транзитных позиций должно быть ровно две"
        );
    }
}
