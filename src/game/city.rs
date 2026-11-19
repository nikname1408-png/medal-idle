//! Город: сюжетная цепочка Мартина, лавка, банк, гильдия, ратуша,
//! таверна, храм, налоги и городские события.
//!
//! Вся логика живёт здесь и не зависит от Dioxus. Таймеры считаются в
//! `GameState::total_play_seconds`, поэтому переживают перезапуск и не
//! ускоряются тайм-варпом.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::actions;
use super::balance;
use super::expedition;
use super::i18n;
use super::items;
use super::quests::{Quest, QuestAuthor, QuestReward, QuestStatus};
use super::state::{self, GameState};

/// Шаблон сюжетного объявления Мартина. Обычные квесты используют 0–7.
pub const INTRO_TEMPLATE_ID: u8 = 8;
/// Стабильный ID сюжетного квеста.
pub const INTRO_QUEST_ID: &str = "city_intro";
/// Награда за вступление в город.
pub const INTRO_REWARD_GOLD: f64 = 500.0;

/// Этап знакомства с Мартином.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MartinStage {
    /// Ещё не разговаривали.
    #[default]
    NotMet,
    /// Мартин спрашивает, мастер ли игрок.
    AskCraft,
    /// Ждёт компас.
    WaitCompass,
    /// Город открыт.
    Complete,
}

/// Городское состояние. Все поля с `serde(default)`: старые сохранения
/// получают город закрытым, без лавки и событий.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CityState {
    pub unlocked: bool,
    pub martin: MartinStage,
    pub shop: Option<Shop>,
    /// Купленные ячейки банка: каждая добавляет золотой лимит.
    pub bank_cells: u32,
    /// Сколько квестов банкира уже выполнено (0..=3).
    pub bank_quests_done: u32,
    pub guild_unlocked: bool,
    pub town_hall_unlocked: bool,
    /// Купленные улучшения гильдии.
    pub upgrades: HashSet<String>,
    pub taxes_paid: u32,
    pub taxes_missed: u32,
    /// Момент следующего налога в `total_play_seconds`.
    pub next_tax_at: f64,
    /// Неоплаченный остаток налога, который можно погасить из золота.
    pub tax_due: f64,
    /// Письмо-предупреждение о налоге уже отправлено.
    pub tax_warning_sent: bool,
    /// Момент следующей попытки городского события.
    pub next_event_at: f64,
    pub active_event: Option<CityEvent>,
    /// Пожар ждёт решения игрока: починить или проигнорировать.
    pub fire_pending: bool,
    /// Благословение храма действует до этого момента.
    pub blessing_until: f64,
    /// Исповедь доступна с этого момента.
    pub next_confession_at: f64,
    /// Следующий слух в таверне.
    pub next_rumor_at: f64,
    /// Непрерывный спрос города по каждому товару.
    pub demand: HashMap<String, DemandEntry>,
    /// Момент последнего пересчёта спроса в `total_play_seconds`.
    pub demand_at: f64,
    /// Стоимость потока производственных линий с прошлого налога.
    pub production_value: f64,
    /// Жалованная грамота на наём получена: платный найм открыт.
    pub hire_charter: bool,
    /// Наёмные работники по линиям: лесопилка, шахта, угольная яма.
    pub hired_sawmill: u32,
    pub hired_mine: u32,
    pub hired_coal: u32,
    /// Момент следующей выплаты зарплат в `total_play_seconds`.
    pub next_salary_at: f64,
    /// Кузнецы в «Труде»: два независимых мастера.
    pub smiths: [SmithState; balance::SMITH_COUNT],
    /// Разрешение ратуши на постройку второй кузницы.
    pub smith_second_permit: bool,
    /// Старое сохранение до v15: уровень дома кузнеца в гильдии.
    #[serde(default, skip_serializing, rename = "smith_house_level")]
    pub legacy_smith_level: u32,
    /// Старое сохранение до v15: рецепты дома кузнеца.
    #[serde(default, skip_serializing, rename = "smith_house_recipes")]
    pub legacy_smith_recipes: Vec<String>,
    /// Старое сохранение до v15: дом кузнеца стоял без зарплаты.
    #[serde(default, skip_serializing, rename = "smith_house_paused")]
    pub legacy_smith_paused: bool,
    /// Старое сохранение до v15: момент следующей ковки дома кузнеца.
    #[serde(default, skip_serializing, rename = "next_smith_craft_at")]
    pub legacy_next_smith_craft_at: f64,
    /// Приказчик в лавке нанят.
    pub clerk_hired: bool,
    /// Наценка, с которой приказчик выставляет товар.
    pub clerk_mult: f32,
    /// Момент следующей работы приказчика.
    pub next_clerk_at: f64,
    /// Склад приказчика: личный запас лавки, откуда он выставляет товар.
    pub stock: Vec<StockItem>,
    /// Сколько ячеек склада куплено отдельными улучшениями гильдии.
    pub stock_cells_bought: u32,
    /// Путешественники, живущие в таверне (после постройки — в Доме странников).
    pub travelers: Vec<Traveler>,
    /// Путешественник предложил построить Дом странников.
    pub traveler_house_offered: bool,
    /// Игрок уже ответил на предложение постройки.
    pub traveler_house_answer: Option<bool>,
    /// Игрок согласился строить дом — с этого момента Ратуша пускает к земле.
    pub traveler_house_land: bool,
    /// Земля под Дом странников арендована.
    pub traveler_house_rented: bool,
    /// Сколько этапов стройки уже сделано (0–3).
    pub traveler_house_stage: u32,
    /// Когда закончится текущий этап (0 — ничего не строится).
    pub traveler_house_ends_at: f64,
}

impl Default for CityState {
    fn default() -> Self {
        Self {
            unlocked: false,
            martin: MartinStage::NotMet,
            shop: None,
            bank_cells: 0,
            bank_quests_done: 0,
            guild_unlocked: false,
            town_hall_unlocked: false,
            upgrades: HashSet::new(),
            taxes_paid: 0,
            taxes_missed: 0,
            next_tax_at: 0.0,
            tax_due: 0.0,
            tax_warning_sent: false,
            next_event_at: 0.0,
            active_event: None,
            fire_pending: false,
            blessing_until: 0.0,
            next_confession_at: 0.0,
            next_rumor_at: 0.0,
            demand: HashMap::new(),
            demand_at: 0.0,
            production_value: 0.0,
            hire_charter: false,
            hired_sawmill: 0,
            hired_mine: 0,
            hired_coal: 0,
            next_salary_at: 0.0,
            smiths: std::array::from_fn(|_| SmithState::default()),
            smith_second_permit: false,
            legacy_smith_level: 0,
            legacy_smith_recipes: Vec::new(),
            legacy_smith_paused: false,
            legacy_next_smith_craft_at: 0.0,
            clerk_hired: false,
            clerk_mult: balance::CLERK_DEFAULT_MULT,
            next_clerk_at: 0.0,
            stock: Vec::new(),
            stock_cells_bought: 0,
            travelers: Vec::new(),
            traveler_house_offered: false,
            traveler_house_answer: None,
            traveler_house_land: false,
            traveler_house_rented: false,
            traveler_house_stage: 0,
            traveler_house_ends_at: 0.0,
        }
    }
}

/// Один городской кузнец из «Труда».
///
/// У каждого мастера своя скорость, горн, очередь рецептов и жалованье.
/// Огонь в горне не утекает со временем: он тратится только на ковку и
/// пополняется углём из склада игрока по потребности.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmithState {
    /// Кузнец построен.
    pub built: bool,
    /// Уровень скорости (0–5): пауза между ковками.
    pub speed_level: u32,
    /// Уровень горна (0–5): ёмкость и огонь с угля.
    pub forge_level: u32,
    /// Уровень ячеек (0–5): 5 → 10 рецептов в очереди.
    pub slots_level: u32,
    /// Очередь рецептов по приоритету.
    pub recipes: Vec<String>,
    /// Запас огня в горне.
    pub fire: u32,
    /// Момент следующей ковки в `total_play_seconds`.
    pub next_craft_at: f64,
    /// Накопленное жалованье, которое спишется в общий цикл выплат.
    pub wage_due: f64,
    /// Кузнец не получил жалованье и стоит до следующего цикла.
    pub paused: bool,
}

impl Default for SmithState {
    fn default() -> Self {
        Self {
            built: false,
            speed_level: 0,
            forge_level: 0,
            slots_level: 0,
            recipes: Vec::new(),
            fire: 0,
            next_craft_at: 0.0,
            wage_due: 0.0,
            paused: false,
        }
    }
}

/// Путешественник: постоянный житель таверны (потом Дома странников).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Traveler {
    /// Номер в `TRAVELER_PROFILES`.
    pub profile_id: usize,
    /// Очки дружбы (0–50, кружок = 10).
    pub friendship: u32,
    /// Квесты, которые путешественник задал.
    pub quests_asked: Vec<String>,
    /// Проваленные квесты: в пул этого персонажа они уже не вернутся.
    pub quests_failed: Vec<String>,
    /// Квест, который сейчас ждёт решения игрока.
    pub active_quest: Option<String>,
    /// Квест пройден и ждёт подтверждения обычной репликой.
    pub quest_result: bool,
    /// 0 — свободен; иначе момент возвращения в `total_play_seconds`.
    pub returns_at: f64,
    /// Куда ушёл путешественник (id локации путешественников).
    pub trip: String,
    /// Аванс, уплаченный перед походом.
    pub advance: f64,
    /// Лут, который путешественник принёс и ждёт, когда его заберут.
    pub carried: Vec<(String, u32)>,
    /// Доплата за последний поход, которую ещё не забрали.
    pub payout: f64,
    /// Если поход пустой — аванс сгорел (для письма и карточки).
    pub last_empty: bool,
    /// Снаряжение из Дома странников: Оружие / Броня / Аксессуар.
    pub gear: [Option<String>; 3],
}

impl Default for Traveler {
    fn default() -> Self {
        Self {
            profile_id: 0,
            friendship: 0,
            quests_asked: Vec::new(),
            quests_failed: Vec::new(),
            active_quest: None,
            quest_result: false,
            returns_at: 0.0,
            trip: String::new(),
            advance: 0.0,
            carried: Vec::new(),
            payout: 0.0,
            last_empty: false,
            gear: [None, None, None],
        }
    }
}

/// Спрос города на один товар.
///
/// Устроен как товар у торговца: спрос и предложение (0..100), тренд
/// настроения и шаг симуляции. Дефицит — спрос обгоняет предложение,
/// избыток — наоборот; цифры сами гуляют, поэтому статус живёт часами.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DemandEntry {
    /// Сколько спроса налито сейчас (в штуках): покупатели возьмут не больше.
    pub available: f64,
    /// Полная ёмкость спроса при текущем предложении: сколько штук город
    /// готов взять, пока не насытится.
    pub capacity: f64,
    /// Спрос 0..100: желание города.
    pub demand: u8,
    /// Предложение 0..100: сколько похожего товара уже навезли.
    pub supply: u8,
    /// Настроение с памятью, `-TREND_LIMIT..=TREND_LIMIT`.
    pub trend: i32,
    /// Номер шага симуляции, который уже учтён в этой записи.
    pub step: u64,
}

impl Default for DemandEntry {
    fn default() -> Self {
        Self {
            available: 0.0,
            capacity: 0.0,
            demand: balance::DEMAND_EQUILIBRIUM,
            supply: balance::DEMAND_EQUILIBRIUM,
            trend: 0,
            step: 0,
        }
    }
}

/// Одна позиция на полке лавки.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopSlot {
    pub item_id: String,
    pub quantity: u32,
    /// Наценка к базовой цене: 0.5–2.0.
    pub price_mult: f32,
}

impl Default for ShopSlot {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            quantity: 0,
            price_mult: 1.0,
        }
    }
}

/// Позиция на складе приказчика: его личный запас лавки.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StockItem {
    pub item_id: String,
    pub quantity: u32,
}

/// Запись о продаже для ратуши и налогов.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaleRecord {
    pub item_id: String,
    pub quantity: u32,
    pub unit_price: f64,
    pub total: f64,
    pub timestamp: f64,
    pub vip: bool,
}

impl Default for SaleRecord {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            quantity: 0,
            unit_price: 0.0,
            total: 0.0,
            timestamp: 0.0,
            vip: false,
        }
    }
}

/// Лавка игрока.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shop {
    pub rating: f32,
    pub slots: Vec<ShopSlot>,
    pub max_slots: u32,
    pub cash: f64,
    pub cash_cap: f64,
    /// Последние 100 продаж.
    pub sales: Vec<SaleRecord>,
    pub total_sales: u32,
    pub total_profit: f64,
    pub next_buyer_at: f64,
}

impl Default for Shop {
    fn default() -> Self {
        Self {
            rating: balance::SHOP_STARTING_RATING,
            slots: Vec::new(),
            max_slots: balance::SHOP_STARTING_SLOTS,
            cash: 0.0,
            cash_cap: balance::SHOP_STARTING_CASH_CAP,
            sales: Vec::new(),
            total_sales: 0,
            total_profit: 0.0,
            next_buyer_at: 0.0,
        }
    }
}

/// Активное городское событие.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CityEvent {
    /// Конкурент: поток −30%, цены −20%, налив спроса перекрыт.
    Competitor {
        ends_at: f64,
        /// Товары, которым конкурент перекрыл налив спроса.
        #[serde(default)]
        items: Vec<String>,
    },
    /// Слух-пара: один товар объявлен подделкой, другой — «тем самым».
    Rumor {
        /// Горячий товар: +50% к цене и двойной налив.
        item_id: String,
        /// Мёртвый товар: спрос обнулён, налив перекрыт.
        #[serde(default)]
        dead_item: String,
        ends_at: f64,
    },
    /// Эпидемия: покупатели не ходят.
    Epidemic { ends_at: f64 },
    /// Праздник: шанс VIP ×3.
    Festival { ends_at: f64 },
    /// Ярмарка: поток ×2, штраф за цену отключён.
    Fair { ends_at: f64 },
}

impl CityEvent {
    pub fn ends_at(&self) -> f64 {
        match self {
            Self::Competitor { ends_at, .. }
            | Self::Rumor { ends_at, .. }
            | Self::Epidemic { ends_at }
            | Self::Festival { ends_at }
            | Self::Fair { ends_at } => *ends_at,
        }
    }

    pub fn is_active(&self, now: f64) -> bool {
        self.ends_at() > now
    }

    /// Ключ названия события.
    pub fn name_key(&self) -> &'static str {
        match self {
            Self::Competitor { .. } => "event.competitor.name",
            Self::Rumor { .. } => "event.rumor.name",
            Self::Epidemic { .. } => "event.epidemic.name",
            Self::Festival { .. } => "event.festival.name",
            Self::Fair { .. } => "event.fair.name",
        }
    }

    pub fn name(&self) -> &'static str {
        i18n::t(self.name_key())
    }
}

/// Результат попытки покупателя.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyerDecision {
    Buy {
        vip: bool,
    },
    /// Покупатель ушёл из-за слишком высокой цены.
    Leave,
    /// Полка пустая или её нет.
    Skip,
}

/// Итог налога.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaxOutcome {
    Paid,
    /// Налог выставлен счётом, но касса и золото покрыли его лишь частично.
    Short,
    Missed,
    Closed,
    NoShop,
    /// Платить нечего: счёта нет.
    NothingDue,
}

/// Результат игры в кости.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiceResult {
    Win,
    Draw,
    Lose,
}

/// Ошибка лавки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopError {
    NoShop,
    SlotLimit,
    MissingItem,
    Unsellable,
    BadSlot,
    EmptySlot,
    NoSpace,
    NoGold,
    /// На полках нет места для нового товара.
    ShelfFull,
    /// В кассу больше не влезает ни одной штуки.
    CashFull,
}

impl ShopError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NoShop => i18n::t("err.shop_closed"),
            Self::SlotLimit => i18n::t("err.shop_slot_limit"),
            Self::MissingItem => i18n::t("err.shop_missing_item"),
            Self::Unsellable => i18n::t("err.shop_unsellable"),
            Self::BadSlot => i18n::t("err.shop_bad_slot"),
            Self::EmptySlot => i18n::t("err.shop_empty_slot"),
            Self::NoSpace => i18n::t("err.shop_no_space"),
            Self::ShelfFull => i18n::t("err.shop_shelf_full"),
            Self::NoGold => i18n::t("err.no_gold"),
            Self::CashFull => i18n::t("err.shop_cash_full"),
        }
    }
}

/// Ошибка банка.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankError {
    MaxCells,
    NoGold,
}

impl BankError {
    pub fn text(self) -> &'static str {
        match self {
            Self::MaxCells => i18n::t("err.bank_max_cells"),
            Self::NoGold => i18n::t("err.no_gold"),
        }
    }
}

/// Ошибка гильдии.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuildError {
    NotInGuild,
    AlreadyMember,
    NotEnoughSales,
    NotEnoughGold,
    UnknownUpgrade,
    AlreadyBought,
    PrerequisiteMissing,
}

impl GuildError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotInGuild => i18n::t("err.guild_not_member"),
            Self::AlreadyMember => i18n::t("err.guild_member"),
            Self::NotEnoughSales => i18n::t("err.guild_sales"),
            Self::NotEnoughGold => i18n::t("err.guild_gold"),
            Self::UnknownUpgrade => i18n::t("err.guild_upgrade_unknown"),
            Self::AlreadyBought => i18n::t("err.guild_upgrade_bought"),
            Self::PrerequisiteMissing => i18n::t("err.guild_upgrade_prerequisite"),
        }
    }
}

/// Ошибка таверны.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TavernError {
    NotEnoughGold,
    Cooldown,
    BadBet,
}

impl TavernError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotEnoughGold => i18n::t("err.no_gold"),
            Self::Cooldown => i18n::t("err.tavern_cooldown"),
            Self::BadBet => i18n::t("err.tavern_bet"),
        }
    }
}

/// Ошибка храма.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChurchError {
    NotEnoughGold,
    Cooldown,
    BadAmount,
}

impl ChurchError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotEnoughGold => i18n::t("err.no_gold"),
            Self::Cooldown => i18n::t("err.church_cooldown"),
            Self::BadAmount => i18n::t("err.church_amount"),
        }
    }
}

/// Ошибка найма.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HireError {
    /// Нет жалованной грамоты: платный найм закрыт.
    NoCharter,
    /// Грамота уже получена.
    AlreadyHave,
    /// Ратуша ещё закрыта.
    NoTownHall,
    /// Линия не построена.
    NoLine,
    NotEnoughGold,
}

impl HireError {
    pub fn text(self) -> &'static str {
        match self {
            Self::NoCharter => i18n::t("err.hire_no_charter"),
            Self::AlreadyHave => i18n::t("err.charter_have"),
            Self::NoTownHall => i18n::t("err.town_hall_locked"),
            Self::NoLine => i18n::t("err.hire_no_line"),
            Self::NotEnoughGold => i18n::t("err.no_gold"),
        }
    }
}

/// Ошибка путешественника в таверне.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelerError {
    /// Нет такого путешественника.
    BadTraveler,
    /// Путешественник в пути.
    Busy,
    /// Не хватает золота.
    NotEnoughGold,
    /// Сейчас нет квеста.
    NoQuest,
    /// Квест ждёт другого решения.
    QuestDecided,
    /// Нет компаса в сумке.
    NoCompass,
    /// Земля под Дом странников ещё не арендована.
    NoLand,
    /// Путешественники ещё не позвали игрока строить дом.
    HouseNotOffered,
    /// Дом уже построен.
    HouseBuilt,
    /// Этап уже идёт.
    Building,
    /// Ну постройку не хватает материалов.
    NoMaterials,
    /// Путешественник ещё не забрал предыдущий лут.
    LootWaiting,
    /// Забирать нечего.
    NothingToClaim,
    /// Это снаряжение уже надето на другого путешественника.
    GearSlotTaken,
    /// Нужного предмета нет в сумке.
    MissingGear,
    /// В сумке нет места под снятое снаряжение.
    NoSpace,
}

impl TravelerError {
    pub fn text(self) -> &'static str {
        match self {
            Self::BadTraveler => i18n::t("err.traveler_bad"),
            Self::Busy => i18n::t("err.traveler_busy"),
            Self::NotEnoughGold => i18n::t("err.no_gold"),
            Self::NoQuest => i18n::t("err.traveler_no_quest"),
            Self::QuestDecided => i18n::t("err.traveler_quest_decided"),
            Self::NoCompass => i18n::t("err.traveler_no_compass"),
            Self::NoLand => i18n::t("err.traveler_house_no_land"),
            Self::HouseNotOffered => i18n::t("err.traveler_house_not_offered"),
            Self::HouseBuilt => i18n::t("err.traveler_house_built"),
            Self::Building => i18n::t("err.traveler_house_building"),
            Self::NoMaterials => i18n::t("err.traveler_house_materials"),
            Self::LootWaiting => i18n::t("err.traveler_loot_waiting"),
            Self::NothingToClaim => i18n::t("err.traveler_nothing_to_claim"),
            Self::GearSlotTaken => i18n::t("err.traveler_gear_taken"),
            Self::MissingGear => i18n::t("err.traveler_gear_missing"),
            Self::NoSpace => i18n::t("err.no_space"),
        }
    }
}

/// Ошибка кузнеца из «Труда».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmithError {
    /// Город ещё не открыт.
    Locked,
    /// Ратуша ещё закрыта.
    NoTownHall,
    /// Вторая кузница без разрешения ратуши.
    MissingPermit,
    /// Разрешение уже куплено.
    PermitOwned,
    /// Кузнец уже построен.
    Built,
    /// Кузнец ещё не построен.
    NotBuilt,
    /// Уровень уже максимальный.
    MaxLevel,
    /// Не хватает материалов или золота.
    NotEnough,
    /// Такого рецепта нет.
    UnknownRecipe,
    /// Рецепт ещё не открыт по уровню кузнеца.
    RecipeLocked,
    /// Очередь рецептов заполнена.
    Full,
    /// Этот рецепт уже в очереди.
    AlreadyAssigned,
    /// Такого рецепта нет в очереди.
    NotInQueue,
}

impl SmithError {
    pub fn text(self) -> &'static str {
        match self {
            Self::Locked => i18n::t("err.smith_locked"),
            Self::NoTownHall => i18n::t("err.town_hall_locked"),
            Self::MissingPermit => i18n::t("err.smith_no_permit"),
            Self::PermitOwned => i18n::t("err.smith_permit_owned"),
            Self::Built => i18n::t("err.smith_built"),
            Self::NotBuilt => i18n::t("err.smith_not_built"),
            Self::MaxLevel => i18n::t("err.smith_max"),
            Self::NotEnough => i18n::t("err.not_enough"),
            Self::UnknownRecipe => i18n::t("err.unknown_recipe"),
            Self::RecipeLocked => i18n::t("err.recipe_locked"),
            Self::Full => i18n::t("err.smith_full"),
            Self::AlreadyAssigned => i18n::t("err.smith_assigned"),
            Self::NotInQueue => i18n::t("err.smith_not_assigned"),
        }
    }
}

/// Итог выплаты зарплат.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SalaryOutcome {
    /// Некого оплачивать.
    Nothing,
    /// Все получили жалованье.
    Paid,
    /// Золота не хватило: наёмные ушли, кузня встала.
    Unpaid,
}

/// Улучшение гильдии.
/// `requires` — id предыдущего улучшения в цепочке (`None` — доступно сразу).
/// Показывается только следующее недокупленное улучшение каждой цепочки.
pub struct UpgradeDef {
    pub id: &'static str,
    pub price: f64,
    pub requires: Option<&'static str>,
}

/// Все улучшения гильдии, сгруппированные цепочками.
/// Порядок в массиве — порядок отображения в UI.
pub const GUILD_UPGRADES: [UpgradeDef; 13] = [
    // Цепочка слотов полки.
    UpgradeDef {
        id: "slots_1",
        price: 8_000.0,
        requires: None,
    },
    UpgradeDef {
        id: "slots_2",
        price: 20_000.0,
        requires: Some("slots_1"),
    },
    // Цепочка сейфа (лимит кассы).
    UpgradeDef {
        id: "cash_1",
        price: 6_000.0,
        requires: None,
    },
    UpgradeDef {
        id: "cash_2",
        price: 15_000.0,
        requires: Some("cash_1"),
    },
    UpgradeDef {
        id: "cash_3",
        price: 30_000.0,
        requires: Some("cash_2"),
    },
    UpgradeDef {
        id: "cash_4",
        price: 80_000.0,
        requires: Some("cash_3"),
    },
    UpgradeDef {
        id: "cash_5",
        price: 200_000.0,
        requires: Some("cash_4"),
    },
    // Отдельные улучшения (без цепочки).
    UpgradeDef {
        id: "advert",
        price: 10_000.0,
        requires: None,
    },
    UpgradeDef {
        id: "price_license",
        price: 25_000.0,
        requires: None,
    },
    UpgradeDef {
        id: "vip_license",
        price: 30_000.0,
        requires: None,
    },
    // Цепочка склада приказчика (+5 ячеек за улучшение, до 20).
    UpgradeDef {
        id: "stock_1",
        price: 12_000.0,
        requires: None,
    },
    UpgradeDef {
        id: "stock_2",
        price: 35_000.0,
        requires: Some("stock_1"),
    },
    UpgradeDef {
        id: "stock_3",
        price: 90_000.0,
        requires: Some("stock_2"),
    },
];

/// Товары, которые может назвать слух.
const RUMOR_ITEMS: &[&str] = &[
    "cloth",
    "whetstone",
    "brick",
    "jug",
    "candle",
    "iron_ingot",
    "silver_ingot",
    "gold_ingot",
    "backpack",
    "robe",
    "jacket",
    "sickle",
    "scythe",
    "iron_bucket",
    "wooden_bucket",
    "trader_ring",
    "miner_amulet",
    "compass",
];

/// Сырьё, которое нельзя выставить в лавке.
const SHOP_RAW: &[&str] = &[
    "wood", "stone", "iron", "coal", "clay", "leather", "fiber", "quartz", "silver", "gold_ore",
    "gems", "board",
];

fn random_unit(seed: &mut u64) -> f64 {
    super::trader::random_below(seed, 1_000_000) as f64 / 1_000_000.0
}

fn now(state: &GameState) -> f64 {
    state.total_play_seconds
}

/// Купленное улучшение гильдии.
pub fn has_upgrade(state: &GameState, id: &str) -> bool {
    state.city.upgrades.contains(id)
}

/// Улучшения, доступные для покупки прямо сейчас: некупленные, чьё
/// предшествующее улучшение в цепочке уже куплено (или у которого его нет).
pub fn available_upgrades(state: &GameState) -> Vec<&'static UpgradeDef> {
    GUILD_UPGRADES
        .iter()
        .filter(|def| {
            !has_upgrade(state, def.id) && def.requires.is_none_or(|req| has_upgrade(state, req))
        })
        .collect()
}

/// Действует ли благословение храма.
pub fn blessing_active(state: &GameState) -> bool {
    state.city.blessing_until > now(state)
}

/// Активное событие, если оно ещё не истекло.
pub fn active_event(state: &GameState) -> Option<&CityEvent> {
    state
        .city
        .active_event
        .as_ref()
        .filter(|event| event.is_active(now(state)))
}

pub fn competitor_active(state: &GameState) -> bool {
    matches!(active_event(state), Some(CityEvent::Competitor { .. }))
}

pub fn epidemic_active(state: &GameState) -> bool {
    matches!(active_event(state), Some(CityEvent::Epidemic { .. }))
}

pub fn festival_active(state: &GameState) -> bool {
    matches!(active_event(state), Some(CityEvent::Festival { .. }))
}

pub fn fair_active(state: &GameState) -> bool {
    matches!(active_event(state), Some(CityEvent::Fair { .. }))
}

/// Товар, о дефиците которого говорит слух.
pub fn rumored_item(state: &GameState) -> Option<&str> {
    match active_event(state) {
        Some(CityEvent::Rumor { item_id, .. }) => Some(item_id.as_str()),
        _ => None,
    }
}

/// Товар, который слух объявил подделкой (спрос обнулён).
pub fn dead_item(state: &GameState) -> Option<&str> {
    match active_event(state) {
        Some(CityEvent::Rumor { dead_item, .. }) if !dead_item.is_empty() => {
            Some(dead_item.as_str())
        }
        _ => None,
    }
}

/// Товары, которым конкурент перекрыл налив спроса.
pub fn competitor_items(state: &GameState) -> &[String] {
    match active_event(state) {
        Some(CityEvent::Competitor { items, .. }) => items.as_slice(),
        _ => &[],
    }
}

/// Ранг титула: 0 — Лавочник, 1 — Купец, 2 — Магнат.
pub fn title_rank(total_sales: u32) -> u32 {
    if total_sales >= 200 {
        2
    } else if total_sales >= 50 {
        1
    } else {
        0
    }
}

/// Прибавка к цене от титула.
pub fn title_price_bonus(total_sales: u32) -> f64 {
    0.05 * title_rank(total_sales) as f64
}

/// Прибавка к шансу VIP от титула.
pub fn title_vip_bonus(total_sales: u32) -> f64 {
    if title_rank(total_sales) >= 2 {
        0.05
    } else {
        0.0
    }
}

/// Название титула текущего игрока.
pub fn title_name(total_sales: u32) -> &'static str {
    match title_rank(total_sales) {
        0 => i18n::t("guild.title.0"),
        1 => i18n::t("guild.title.1"),
        _ => i18n::t("guild.title.2"),
    }
}

/// Ранг в гильдии: +1 за каждые 20 продаж, максимум 10.
pub fn guild_rank(state: &GameState) -> u32 {
    let sales = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.total_sales)
        .unwrap_or(0);
    (sales / balance::GUILD_RANK_SALES).min(balance::GUILD_RANK_MAX)
}

/// Цена товара в лавке с учётом наценки, титула, благословения и событий.
pub fn shop_price_for(state: &GameState, item_id: &str, price_mult: f32) -> f64 {
    let base = items::item(item_id)
        .map(|item| item.base_price as f64)
        .unwrap_or(0.0);
    let sales = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.total_sales)
        .unwrap_or(0);
    let mut mult = price_mult as f64 + title_price_bonus(sales);
    if blessing_active(state) {
        mult += balance::CHURCH_BLESSING_PRICE_BONUS;
    }
    if has_upgrade(state, "price_license") {
        mult += 0.05;
    }
    if competitor_active(state) {
        mult *= 0.8;
    }
    if rumored_item(state) == Some(item_id) {
        mult += balance::SHOP_RUMOR_PRICE_BONUS;
    }
    (base * mult).max(0.5)
}

/// Цена одной штуки выбранной позиции.
pub fn shop_price(state: &GameState, slot: &ShopSlot) -> f64 {
    shop_price_for(state, &slot.item_id, slot.price_mult)
}

/// Направление спроса: +1 растёт, −1 падает, 0 ровно.
///
/// Смотрим на настроение с памятью: тренд вниз — город насытился, вверх —
/// товар расходится. Порог отсекает мелкие колебания.
pub fn market_trend(state: &GameState, item_id: &str) -> i32 {
    let entry = demand_of(state, item_id);
    let threshold = (balance::MARKET_TREND_THRESHOLD / 2) as i32;
    if entry.trend >= threshold {
        1
    } else if entry.trend <= -threshold {
        -1
    } else {
        0
    }
}

/// Шанс, что покупатель окажется VIP.
pub fn vip_chance(state: &GameState) -> f64 {
    let shop_rating = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.rating as f64)
        .unwrap_or(0.0);
    let sales = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.total_sales)
        .unwrap_or(0);
    let mut chance = (0.05 + shop_rating * 0.02).min(0.15);
    chance += title_vip_bonus(sales);
    if blessing_active(state) {
        chance += balance::CHURCH_BLESSING_VIP_BONUS;
    }
    if has_upgrade(state, "vip_license") {
        chance += 0.05;
    }
    if festival_active(state) {
        chance *= 3.0;
    }
    chance.min(1.0)
}

/// Интервал между покупателями. `None` — эпидемия, покупателей нет.
pub fn buyer_interval(state: &GameState) -> Option<f64> {
    if epidemic_active(state) {
        return None;
    }
    let rating = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.rating as f64)
        .unwrap_or(balance::SHOP_STARTING_RATING as f64);
    let mut interval = (balance::SHOP_BUYER_BASE_INTERVAL * (1.0 - rating * 0.1)).max(10.0);
    if fair_active(state) {
        interval *= 0.5;
    }
    if competitor_active(state) {
        interval /= 0.7;
    }
    if has_upgrade(state, "advert") {
        interval *= 0.85;
    }
    Some(interval)
}

/// Сколько штук товара город берёт за один заказ при равновесии.
///
/// Тот же тир, что у торговца, умноженный на `DEMAND_CITY_SCALE`: экономика
/// одна, отличается только масштаб.
pub fn demand_capacity(item_id: &str) -> f64 {
    super::market::capacity_per_trend(item_id)
}

/// Спрос города на полке, выведенный из пары «спрос / предложение»:
/// дефицит поднимает ёмкость, избыток — опускает.
fn capacity_of(item_id: &str, demand: u8, supply: u8) -> f64 {
    let base = demand_capacity(item_id);
    let diff = demand as f64 - supply as f64;
    (base * (1.0 + diff * balance::DEMAND_TREND_UNITS)).max(1.0)
}

/// Текущий спрос на товар. Для товара без записи — ровное равновесие.
pub fn demand_of(state: &GameState, item_id: &str) -> DemandEntry {
    if let Some(entry) = state.city.demand.get(item_id) {
        // Старые сохранения писали только налитое, без ёмкости: заводим её
        // заново, иначе доля спроса делила бы на ноль.
        let capacity = if entry.capacity > 0.0 && entry.capacity.is_finite() {
            entry.capacity
        } else {
            demand_capacity(item_id)
        };
        let mut repaired = entry.clone();
        // Налитое никогда не больше ёмкости: мусор из старого сохранения
        // не должен превращаться в «774 793 388 419 / 500» на рынке.
        repaired.available = if repaired.available.is_finite() {
            repaired.available.clamp(0.0, capacity)
        } else {
            capacity
        };
        repaired.capacity = capacity;
        return repaired;
    }
    let capacity = demand_capacity(item_id);
    DemandEntry {
        available: capacity,
        capacity,
        demand: balance::DEMAND_EQUILIBRIUM,
        supply: balance::DEMAND_EQUILIBRIUM,
        trend: 0,
        step: 0,
    }
}

/// Доля налитого спроса 0..1.
pub fn demand_ratio(state: &GameState, item_id: &str) -> f64 {
    let entry = demand_of(state, item_id);
    super::market::ratio(entry.available, entry.capacity)
}

/// Статус рынка товара: дефицит, баланс или избыток.
pub fn demand_status(state: &GameState, item_id: &str) -> super::trader::MarketStatus {
    let entry = demand_of(state, item_id);
    super::market::status(entry.demand, entry.supply)
}

/// Создаёт запись спроса, если её ещё нет: город встречает товар полным спросом.
fn ensure_demand(state: &mut GameState, item_id: &str) {
    if state
        .city
        .demand
        .get(item_id)
        .is_some_and(|entry| entry.capacity > 0.0)
    {
        return;
    }
    let capacity = demand_capacity(item_id);
    let step = city_step(state);
    state.city.demand.insert(
        item_id.to_string(),
        DemandEntry {
            available: capacity,
            capacity,
            demand: balance::DEMAND_EQUILIBRIUM,
            supply: balance::DEMAND_EQUILIBRIUM,
            trend: 0,
            step,
        },
    );
}

/// Текущий шаг симуляции города: один шаг в `DEMAND_STEP_SECS`.
pub fn city_step(state: &GameState) -> u64 {
    (state.total_play_seconds / balance::DEMAND_STEP_SECS).max(0.0) as u64
}

/// Один шаг настроения: тренд гуляет, спрос и предложение тянутся к нему.
///
/// Дешёвый шум и несколько пунктов возврата к равновесию на каждом шаге
/// дают живую кривую: дефицит набирается минутами и так же медленно спадает.
fn step_entry(entry: &mut DemandEntry, seed: &mut u64) {
    let trend = super::market::drift_trend(entry.trend, seed);
    entry.trend = trend;
    let target_demand = (balance::DEMAND_EQUILIBRIUM as i32 + trend).clamp(5, 95) as u8;
    let target_supply = (balance::DEMAND_EQUILIBRIUM as i32 - trend).clamp(5, 95) as u8;
    entry.demand = (entry.demand as i32 + super::market::noise(seed, 2)).clamp(5, 95) as u8;
    entry.supply = (entry.supply as i32 + super::market::noise(seed, 2)).clamp(5, 95) as u8;
    entry.demand = super::market::approach(entry.demand, target_demand, balance::DEMAND_RETURN_STEP);
    entry.supply = super::market::approach(entry.supply, target_supply, balance::DEMAND_RETURN_STEP);
}

/// Продвигает спрос на `dt` секунд: цеховые шаги и налив полки.
fn advance_demand(state: &mut GameState, dt: f64) {
    if dt <= 0.0 {
        return;
    }
    // Сначала догоняем цеховые шаги за прошедшее время, потом наливаем полку.
    // `demand_at` — время последнего расчёта, его двигает только тик.
    let period = (state.total_play_seconds - state.city.demand_at).clamp(0.0, dt);
    // Город живёт по всему каталогу: товар тянет спрос и без полки в лавке,
    // а полка лишь добавляет к нему продажи. Иначе рынок стоял бы полным,
    // пока игрок ничего не выставил.
    let mut item_ids: Vec<String> = items::catalog()
        .iter()
        .filter(|item| is_shop_sellable(state, &item.id))
        .map(|item| item.id.clone())
        .collect();
    if let Some(shop) = state.city.shop.as_ref() {
        for slot in &shop.slots {
            if !item_ids.iter().any(|id| id == &slot.item_id) {
                item_ids.push(slot.item_id.clone());
            }
        }
    }
    if item_ids.is_empty() {
        return;
    }
    let rumored = rumored_item(state).map(str::to_string);
    let dead = dead_item(state).map(str::to_string);
    let blocked: Vec<String> = competitor_items(state).to_vec();
    let city_step = city_step(state);
    for item_id in item_ids {
        ensure_demand(state, &item_id);
        let is_dead = dead.as_deref() == Some(item_id.as_str());
        let is_blocked = blocked.iter().any(|id| id == &item_id);
        let is_rumored = rumored.as_deref() == Some(item_id.as_str());
        let mut seed = state.market_seed;
        let entry = state
            .city
            .demand
            .get_mut(&item_id)
            .expect("спрос создан выше");
        // Догоняем пропущенные шаги: город жил, пока игра не считала.
        let missed = city_step.saturating_sub(entry.step).min(240);
        for _ in 0..missed {
            step_entry(entry, &mut seed);
        }
        entry.step = entry.step.saturating_add(missed).max(city_step);
        // Слухи и конкуренты правят тренд своим весом.
        if is_dead {
            entry.trend = -super::market::TREND_LIMIT;
            entry.demand = entry.demand.min(5);
            entry.supply = entry.supply.max(95);
        } else if is_blocked {
            entry.trend = entry.trend.min(-super::market::TREND_LIMIT / 2);
        } else if is_rumored {
            entry.trend = entry.trend.max(super::market::TREND_LIMIT / 2);
        }
        let capacity = capacity_of(&item_id, entry.demand, entry.supply);
        let fill_mult = if is_dead || is_blocked {
            0.0
        } else if is_rumored {
            balance::DEMAND_RUMOR_FILL_MULT
        } else {
            1.0
        };
        // Полная ёмкость наливается за `DEMAND_FILL_SECS`; сравнялую ёмкость
        // подрезаем, чтобы полоска не показывала больше единицы.
        let filled = entry.available + capacity / balance::DEMAND_FILL_SECS * period * fill_mult;
        entry.capacity = capacity;
        entry.available = if is_dead { 0.0 } else { filled.min(capacity) };
        state.market_seed = seed;
    }
}

/// Навёрстывает спрос за время, пока игра была закрыта.
pub fn catch_up_offline(state: &mut GameState, elapsed: f64) {
    if !state.city.unlocked {
        state.city.demand_at = state.total_play_seconds;
        return;
    }
    // Город живёт и без игрока: зарплаты за пропущенный цикл списываются.
    // Больше одного цикла не начисляем, чтобы отпуск не обнулял кошелёк.
    if elapsed >= balance::CITY_TAX_INTERVAL && salary_total(state) > 1e-6 {
        pay_salaries(state);
    }
    let dt = elapsed.clamp(0.0, balance::DEMAND_FILL_SECS * 2.0);
    advance_demand(state, dt);
    state.city.demand_at = state.total_play_seconds;
}

/// Шанс, что покупатель уйдёт из-за наценки.
///
/// Кривая гладкая: до `SHOP_PRICE_LEAVE_FROM` берут все, дальше шанс растёт
/// квадратично и на `SHOP_PRICE_LEAVE_FULL` доходит до полного отказа. Она
/// действует на всех полках и на всех покупателей, включая VIP; ярмарка её
/// отключает.
pub fn price_leave_chance(price_mult: f32) -> f64 {
    let from = balance::SHOP_PRICE_LEAVE_FROM;
    let full = balance::SHOP_PRICE_LEAVE_FULL;
    if price_mult <= from {
        return 0.0;
    }
    if price_mult >= full {
        return 1.0;
    }
    let t = ((price_mult - from) / (full - from)) as f64;
    t * t
}

/// Можно ли выставить предмет в лавке.
pub fn is_shop_sellable(state: &GameState, item_id: &str) -> bool {
    if items::item(item_id).is_none() {
        return false;
    }
    if state::is_active_resource(item_id) {
        return false;
    }
    if SHOP_RAW.contains(&item_id) {
        return false;
    }
    if item_id == items::STEAM_BLUEPRINT_ID
        || item_id == items::STEAM_PROTOTYPE_ID
        || item_id == items::COMPASS_BLUEPRINT_ID
    {
        return false;
    }
    if let Some(location) = expedition::location_for_map(item_id) {
        return state.location_unlocked(location);
    }
    true
}

/// Открывает лавку, если её ещё нет (первое вступление в город).
pub fn open_shop(state: &mut GameState) {
    if state.city.shop.is_none() {
        state.city.shop = Some(Shop::default());
    }
}

/// Переоткрывает закрытую лавку за золото.
pub fn reopen_shop(state: &mut GameState) -> Result<(), ShopError> {
    if state.gold + 1e-9 < balance::CITY_SHOP_REOPEN_GOLD {
        return Err(ShopError::NoGold);
    }
    state.gold -= balance::CITY_SHOP_REOPEN_GOLD;
    let shop = Shop {
        rating: balance::CITY_SHOP_REOPEN_RATING,
        ..Shop::default()
    };
    state.city.shop = Some(shop);
    state.city.next_tax_at = now(state) + balance::CITY_TAX_INTERVAL;
    state.city.taxes_missed = 0;
    state.city.tax_due = 0.0;
    state.city.tax_warning_sent = false;
    state.letters.retain(|letter| letter.id != "tax_due");
    Ok(())
}

/// Забирает кассу в золото с учётом лимита. Возвращает забранное.
pub fn withdraw_cash(state: &mut GameState) -> f64 {
    let cap = state.gold_cap();
    let room = (cap - state.gold).max(0.0);
    let Some(shop) = state.city.shop.as_mut() else {
        return 0.0;
    };
    let amount = shop.cash.min(room);
    shop.cash -= amount;
    state.gold = (state.gold + amount).min(cap);
    amount
}

/// Кладёт предмет из сумки на полку. Одинаковые товары объединяются.
pub fn add_slot(
    state: &mut GameState,
    item_id: &str,
    quantity: u32,
    price_mult: f32,
) -> Result<(), ShopError> {
    if quantity == 0 {
        return Err(ShopError::MissingItem);
    }
    if state.city.shop.is_none() {
        return Err(ShopError::NoShop);
    }
    if !is_shop_sellable(state, item_id) {
        return Err(ShopError::Unsellable);
    }
    if state.amount_of(item_id) < quantity {
        return Err(ShopError::MissingItem);
    }
    add_shelf(state, item_id, quantity, price_mult)?;
    state.remove_item(item_id, quantity);
    Ok(())
}

/// Кладёт товар на полку, не проверяя рюкзак: так приказчик выставляет
/// товар со своего склада.
fn add_shelf(
    state: &mut GameState,
    item_id: &str,
    quantity: u32,
    price_mult: f32,
) -> Result<(), ShopError> {
    if quantity == 0 {
        return Err(ShopError::MissingItem);
    }
    let price = clamp_price(price_mult);
    {
        let shop = state.city.shop.as_mut().ok_or(ShopError::NoShop)?;
        if let Some(slot) = shop.slots.iter_mut().find(|slot| slot.item_id == item_id) {
            slot.quantity += quantity;
            slot.price_mult = price;
        } else {
            if shop.slots.len() >= shop.max_slots as usize {
                return Err(ShopError::SlotLimit);
            }
            shop.slots.push(ShopSlot {
                item_id: item_id.to_string(),
                quantity,
                price_mult: price,
            });
        }
    }
    Ok(())
}

/// Убирает предмет с полки и возвращает его в рюкзак.
pub fn remove_slot(state: &mut GameState, index: usize) -> Result<(), ShopError> {
    let slot = {
        let shop = state.city.shop.as_ref().ok_or(ShopError::NoShop)?;
        shop.slots.get(index).ok_or(ShopError::BadSlot)?.clone()
    };
    if slot.quantity > 0 && state.item_capacity_left(&slot.item_id) < slot.quantity {
        return Err(ShopError::NoSpace);
    }
    {
        let shop = state.city.shop.as_mut().ok_or(ShopError::NoShop)?;
        if index >= shop.slots.len() {
            return Err(ShopError::BadSlot);
        }
        shop.slots.remove(index);
    }
    if slot.quantity > 0 {
        state.add_item(&slot.item_id, slot.quantity);
    }
    Ok(())
}

/// Меняет наценку позиции, зажимая её в допустимые границы.
pub fn set_price(state: &mut GameState, index: usize, price_mult: f32) -> Result<(), ShopError> {
    let shop = state.city.shop.as_mut().ok_or(ShopError::NoShop)?;
    let slot = shop.slots.get_mut(index).ok_or(ShopError::BadSlot)?;
    slot.price_mult = clamp_price(price_mult);
    Ok(())
}

fn clamp_price(price_mult: f32) -> f32 {
    if price_mult.is_nan() {
        return 1.0;
    }
    price_mult.clamp(balance::SHOP_PRICE_MIN, balance::SHOP_PRICE_MAX)
}

/// Решение покупателя по выбранной полке.
///
/// `leave_roll` и `vip_roll` — значения 0..1: так тесты и тик используют
/// один и тот же детерминированный код.
pub fn buyer_decision(
    state: &GameState,
    index: usize,
    leave_roll: f64,
    vip_roll: f64,
) -> BuyerDecision {
    let Some(shop) = state.city.shop.as_ref() else {
        return BuyerDecision::Skip;
    };
    let Some(slot) = shop.slots.get(index) else {
        return BuyerDecision::Skip;
    };
    if slot.quantity == 0 {
        return BuyerDecision::Skip;
    }
    let fair = fair_active(state);
    let vip = vip_roll < vip_chance(state);
    // Вещь дороже порога берёт только VIP: обычный покупатель проходит мимо.
    if !vip && shop_price(state, slot) > balance::SHOP_VIP_ONLY_PRICE {
        return BuyerDecision::Skip;
    }
    // VIP — единственный, кто берёт насыщенный товар.
    if !vip && demand_of(state, &slot.item_id).available <= 0.0 {
        return BuyerDecision::Skip;
    }
    let price_chance = if fair {
        0.0
    } else {
        price_leave_chance(slot.price_mult)
    };
    // Пустая полка спроса: обычный покупатель уже прошёл мимо выше, а VIP
    // берёт остатки, но тоже не в восторге.
    let empty_chance = if vip && demand_of(state, &slot.item_id).available <= 0.0 {
        balance::DEMAND_LEAVE_MAX
    } else {
        0.0
    };
    if leave_roll < (price_chance + empty_chance).min(1.0) {
        return BuyerDecision::Leave;
    }
    BuyerDecision::Buy { vip }
}

/// Проданный товар съедает спрос города: налитое уменьшается, спрос падает,
/// предложение растёт — полка сама уходит в избыток.
///
/// Снимает только налитое. Сам спрос и предложение двигает один шаг рынка
/// (`advance_demand`), ровно как у торговцев: стоку нечего прибавлять к
/// паре, иначе продажа и тик считали бы её дважды.
pub fn consume_demand(state: &mut GameState, item_id: &str, quantity: u32) {
    ensure_demand(state, item_id);
    let Some(entry) = state.city.demand.get_mut(item_id) else {
        return;
    };
    entry.available = (entry.available - quantity as f64).max(0.0);
}

/// Снимает пустые полки: товар распродан — позиции в лавке быть не должно.
pub fn drop_empty_slots(state: &mut GameState) {
    if let Some(shop) = state.city.shop.as_mut() {
        shop.slots.retain(|slot| slot.quantity > 0);
    }
}

/// Продаёт конкретное количество с полки (не больше, чем лежит).
///
/// Касса не переполняется: если в неё влезает меньше товара, покупатель берёт
/// только то, что помещается, а остаток остаётся на полке.
pub fn sell_slot_amount(
    state: &mut GameState,
    index: usize,
    vip: bool,
    amount: u32,
) -> Result<SaleRecord, ShopError> {
    let (item_id, requested, unit_price, room) = {
        let shop = state.city.shop.as_ref().ok_or(ShopError::NoShop)?;
        let slot = shop.slots.get(index).ok_or(ShopError::BadSlot)?;
        if slot.quantity == 0 {
            return Err(ShopError::EmptySlot);
        }
        let requested = amount.clamp(1, slot.quantity);
        let unit = shop_price(state, slot);
        let unit = if vip {
            unit * (1.0 + balance::SHOP_VIP_PRICE_BONUS)
        } else {
            unit
        };
        let room = (shop.cash_cap - shop.cash).max(0.0);
        (slot.item_id.clone(), requested, unit, room)
    };
    let affordable = if unit_price <= 0.0 {
        requested
    } else {
        (room / unit_price).floor() as u32
    };
    let quantity = requested.min(affordable);
    if quantity == 0 {
        return Err(ShopError::CashFull);
    }
    let total = unit_price * quantity as f64;
    let timestamp = now(state);
    let record = SaleRecord {
        item_id,
        quantity,
        unit_price,
        total,
        timestamp,
        vip,
    };
    {
        let shop = state.city.shop.as_mut().ok_or(ShopError::NoShop)?;
        let slot = shop.slots.get_mut(index).ok_or(ShopError::BadSlot)?;
        slot.quantity -= quantity;
        // Товар кончился — полка освобождается: приказчик поставит на её
        // место что-то со склада, а не будет держать пустую карточку.
        if slot.quantity == 0 {
            shop.slots.remove(index);
        }
        shop.cash = (shop.cash + total).min(shop.cash_cap);
        shop.total_sales += 1;
        if shop.total_sales % balance::SHOP_RATING_SALES_PER_GAIN == 0 {
            shop.rating =
                (shop.rating + balance::SHOP_RATING_PER_GAIN).min(balance::SHOP_MAX_RATING);
        }
        shop.total_profit += total;
        shop.sales.push(record.clone());
        if shop.sales.len() > 100 {
            shop.sales.remove(0);
        }
    }
    // Покупка выпивает спрос города.
    ensure_demand(state, &record.item_id);
    if let Some(entry) = state.city.demand.get_mut(&record.item_id) {
        entry.available = (entry.available - quantity as f64).max(0.0);
    }
    Ok(record)
}

/// Продаёт всю позицию целиком (используется в тестах).
#[cfg(test)]
pub fn sell_slot(state: &mut GameState, index: usize, vip: bool) -> Result<SaleRecord, ShopError> {
    let quantity = state
        .city
        .shop
        .as_ref()
        .and_then(|shop| shop.slots.get(index))
        .map(|slot| slot.quantity)
        .ok_or(ShopError::BadSlot)?;
    sell_slot_amount(state, index, vip, quantity)
}

/// Сколько штук может унести покупатель: обычный ограничен спросом и
/// одной штукой за визит, VIP берёт до двух.
fn buyer_amount(quantity: u32, available: f64, vip: bool) -> u32 {
    let max_visit = if vip {
        balance::SHOP_VIP_MAX
    } else {
        balance::SHOP_BUYER_MAX
    };
    if vip {
        quantity.min(max_visit).max(1)
    } else {
        quantity
            .min(available.floor().max(1.0) as u32)
            .min(max_visit)
            .max(1)
    }
}

/// Дорогую вещь берёт только VIP.
fn too_expensive_for_common(state: &GameState, index: usize) -> bool {
    state
        .city
        .shop
        .as_ref()
        .and_then(|shop| shop.slots.get(index))
        .map(|slot| shop_price(state, slot) > balance::SHOP_VIP_ONLY_PRICE)
        .unwrap_or(false)
}

/// Один покупатель по расписанию. Возвращает запись, если покупка состоялась.
pub fn try_buyer(state: &mut GameState) -> Option<SaleRecord> {
    {
        // Полная касса: покупатель не приходит, пока золото не заберут.
        let shop = state.city.shop.as_ref()?;
        if shop.cash >= shop.cash_cap {
            return None;
        }
    }
    // Пустые полки покупателя не интересуют; дорогую вещь не-VIP не берут.
    let vip_roll = {
        let mut seed = state.market_seed;
        let roll = random_unit(&mut seed);
        state.market_seed = seed;
        roll
    };
    let vip = vip_roll < vip_chance(state);
    let options: Vec<usize> = {
        let shop = state.city.shop.as_ref()?;
        shop.slots
            .iter()
            .enumerate()
            .filter(|(index, slot)| {
                slot.quantity > 0 && (vip || !too_expensive_for_common(state, *index))
            })
            .map(|(index, _)| index)
            .collect()
    };
    if options.is_empty() {
        return None;
    }
    let mut seed = state.market_seed;
    let index = options[super::trader::random_below(&mut seed, options.len() as u64) as usize];
    let leave_roll = random_unit(&mut seed);
    let amount_roll = random_unit(&mut seed);
    state.market_seed = seed;

    match buyer_decision(state, index, leave_roll, vip_roll) {
        BuyerDecision::Buy { vip } => {
            // Покупатель берёт случайную часть стопки, но не больше спроса.
            let (quantity, item_id) = state
                .city
                .shop
                .as_ref()
                .and_then(|shop| shop.slots.get(index))
                .map(|slot| (slot.quantity, slot.item_id.clone()))
                .unwrap_or((0, String::new()));
            let available = demand_of(state, &item_id).available;
            let max_amount = buyer_amount(quantity, available, vip);
            // Город берёт партиями: не меньше двух штук за визит, если спрос есть.
            let batch = balance::SHOP_VISIT_MULT.ceil() as u32;
            let amount = (batch + (amount_roll * max_amount as f64) as u32).min(max_amount);
            let sold = sell_slot_amount(state, index, vip, amount);
            // Проданное съедает спрос: следующий покупатель придёт на меньшее.
            if let Ok(record) = &sold {
                consume_demand(state, &record.item_id, record.quantity);
            }
            sold.ok()
        }
        BuyerDecision::Leave => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.rating = (shop.rating - balance::SHOP_RATING_PER_LEAVE).max(0.0);
            }
            None
        }
        BuyerDecision::Skip => None,
    }
}

/// Учитывает стоимость потока производственных линий для налога с
/// производства. Вызывается только из тика, не из ручной добычи.
///
/// До открытия города налога нет: лесопилка и шахта работают с первого дня,
/// и иначе к приходу лавки на игрока вешался бы счёт за всю прошлую игру.
pub fn record_production(state: &mut GameState, item_id: &str, amount: f64) {
    if amount <= 0.0 || !state.city.unlocked {
        return;
    }
    state.city.production_value += amount * balance::production_cost(item_id);
}

/// Списывает сумму с кассы, затем с золота. Возвращает непокрытый остаток.
fn charge_tax(state: &mut GameState, amount: f64) -> f64 {
    let mut remaining = amount;
    if let Some(shop) = state.city.shop.as_mut() {
        let paid = shop.cash.min(remaining);
        shop.cash -= paid;
        remaining -= paid;
    }
    if remaining > 0.0 {
        let paid = state.gold.min(remaining);
        state.gold -= paid;
        remaining -= paid;
    }
    remaining
}

/// Налог: 15% выручки за последние 30 минут (+2% прибыли с помощником).
///
/// Сначала списывает кассу и золото. Если денег не хватило, остаток остаётся
/// счётом `tax_due`: его можно оплатить из колокольчика или из лавки до
/// следующего налога. Неоплаченный счёт к следующему сроку — пропуск налога.
pub fn pay_tax(state: &mut GameState) -> TaxOutcome {
    if state.city.shop.is_none() {
        state.city.tax_due = 0.0;
        state.city.production_value = 0.0;
        return TaxOutcome::NoShop;
    }
    let timestamp = now(state);

    // Прошлый счёт не оплатили — засчитываем пропуск налога.
    let mut missed_before = false;
    if state.city.tax_due > 1e-6 {
        missed_before = true;
        state.city.taxes_missed += 1;
        state.city.tax_due = 0.0;
        state.city.tax_warning_sent = false;
        state.letters.retain(|letter| letter.id != "tax_due");
        let missed = state.city.taxes_missed;
        let mut close = false;
        if let Some(shop) = state.city.shop.as_mut() {
            shop.rating = (shop.rating - balance::CITY_TAX_RATING_PENALTY).max(0.0);
            close = missed >= 2;
        }
        if close {
            state.city.shop = None;
            state.add_letter("shop_closed", None);
            return TaxOutcome::Closed;
        }
        state.add_letter("tax_missed", None);
    }

    let revenue = match state.city.shop.as_ref() {
        Some(shop) => shop
            .sales
            .iter()
            .filter(|sale| sale.timestamp >= timestamp - balance::CITY_TAX_INTERVAL)
            .map(|sale| sale.total)
            .sum::<f64>(),
        None => return TaxOutcome::NoShop,
    };
    let percent = balance::CITY_TAX_PERCENT;
    // Налог с производства: процент со стоимости потока линий за цикл.
    let production_tax = state.city.production_value * balance::CITY_PRODUCTION_TAX_PERCENT;
    state.city.production_value = 0.0;
    let amount = revenue * percent + production_tax;
    if amount <= 1e-6 {
        state.city.tax_warning_sent = false;
        state.city.tax_due = 0.0;
        state.letters.retain(|letter| letter.id != "tax_due");
        return if missed_before {
            TaxOutcome::Missed
        } else {
            TaxOutcome::Paid
        };
    }

    let remaining = charge_tax(state, amount);
    if remaining > 1e-6 {
        state.city.tax_due = remaining;
        state.city.tax_warning_sent = false;
        state.add_letter("tax_due", None);
        return TaxOutcome::Short;
    }

    state.city.taxes_paid += 1;
    state.city.taxes_missed = 0;
    state.city.tax_warning_sent = false;
    state.city.tax_due = 0.0;
    state.letters.retain(|letter| letter.id != "tax_due");
    state.add_letter("tax_paid", None);
    if missed_before {
        TaxOutcome::Missed
    } else {
        TaxOutcome::Paid
    }
}

/// Оплачивает выставленный счёт по налогу из кассы и золота.
pub fn pay_tax_due(state: &mut GameState) -> TaxOutcome {
    let due = state.city.tax_due;
    if due <= 1e-6 {
        return TaxOutcome::NothingDue;
    }
    if state.city.shop.is_none() {
        state.city.tax_due = 0.0;
        state.letters.retain(|letter| letter.id != "tax_due");
        return TaxOutcome::NoShop;
    }

    let remaining = charge_tax(state, due);
    if remaining > 1e-6 {
        state.city.tax_due = remaining;
        return TaxOutcome::Short;
    }

    state.city.tax_due = 0.0;
    state.city.taxes_paid += 1;
    state.city.taxes_missed = 0;
    state.city.tax_warning_sent = false;
    state.letters.retain(|letter| letter.id != "tax_due");
    state.add_letter("tax_paid", None);
    TaxOutcome::Paid
}

/// Цена следующей ячейки банка.
pub fn bank_cell_price(cells: u32) -> f64 {
    balance::BANK_CELL_BASE_PRICE * balance::BANK_CELL_PRICE_GROWTH.powi(cells as i32)
}

/// Покупает ячейку банка за золото.
pub fn buy_bank_cell(state: &mut GameState) -> Result<(), BankError> {
    if state.city.bank_cells >= balance::BANK_CELLS_MAX {
        return Err(BankError::MaxCells);
    }
    let price = bank_cell_price(state.city.bank_cells);
    if state.gold + 1e-9 < price {
        return Err(BankError::NoGold);
    }
    state.gold -= price;
    state.city.bank_cells += 1;
    Ok(())
}

/// Определение квеста банкира.
#[derive(Clone, Copy)]
pub struct BankQuest {
    pub item_id: &'static str,
    pub quantity: u32,
    pub cells: u32,
}

/// Три квеста банкира: 1, 2 и 3 ячейки.
pub fn bank_quests() -> &'static [BankQuest] {
    &BANK_QUEST_DEFS
}

static BANK_QUEST_DEFS: [BankQuest; 3] = [
    BankQuest {
        item_id: "iron_ingot",
        quantity: 10,
        cells: 1,
    },
    BankQuest {
        item_id: "silver_ingot",
        quantity: 20,
        cells: 2,
    },
    BankQuest {
        item_id: "gold_ingot",
        quantity: 5,
        cells: 3,
    },
];

/// Сдаёт следующий квест банкира.
pub fn complete_bank_quest(state: &mut GameState) -> Result<u32, ShopError> {
    let index = state.city.bank_quests_done as usize;
    let Some(quest) = BANK_QUEST_DEFS.get(index) else {
        return Err(ShopError::BadSlot);
    };
    if state.amount_of(quest.item_id) < quest.quantity {
        return Err(ShopError::MissingItem);
    }
    state.remove_item(quest.item_id, quest.quantity);
    let cells = quest
        .cells
        .min(balance::BANK_CELLS_MAX - state.city.bank_cells);
    state.city.bank_cells += cells;
    state.city.bank_quests_done += 1;
    Ok(cells)
}

/// Вступление в гильдию: 15 продаж и 4000 золота.
pub fn join_guild(state: &mut GameState) -> Result<(), GuildError> {
    if state.city.guild_unlocked {
        return Err(GuildError::AlreadyMember);
    }
    let sales = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.total_sales)
        .unwrap_or(0);
    if sales < balance::GUILD_SALES_REQUIRED {
        return Err(GuildError::NotEnoughSales);
    }
    if state.gold + 1e-9 < balance::GUILD_ENTRY_GOLD {
        return Err(GuildError::NotEnoughGold);
    }
    state.gold -= balance::GUILD_ENTRY_GOLD;
    state.city.guild_unlocked = true;
    Ok(())
}

/// Покупает улучшение гильдии.
pub fn buy_upgrade(state: &mut GameState, id: &str) -> Result<(), GuildError> {
    if !state.city.guild_unlocked {
        return Err(GuildError::NotInGuild);
    }
    let Some(def) = GUILD_UPGRADES.iter().find(|def| def.id == id) else {
        return Err(GuildError::UnknownUpgrade);
    };
    if has_upgrade(state, id) {
        return Err(GuildError::AlreadyBought);
    }
    if let Some(req) = def.requires {
        if !has_upgrade(state, req) {
            return Err(GuildError::PrerequisiteMissing);
        }
    }
    if state.gold + 1e-9 < def.price {
        return Err(GuildError::NotEnoughGold);
    }
    state.gold -= def.price;
    state.city.upgrades.insert(id.to_string());

    match id {
        "slots_1" | "slots_2" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.max_slots = (shop.max_slots + 1).min(balance::SHOP_MAX_SLOTS);
            }
        }
        "cash_1" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.cash_cap = (shop.cash_cap + 5_000.0).min(balance::SHOP_MAX_CASH_CAP);
            }
        }
        "cash_2" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.cash_cap = (shop.cash_cap + 10_000.0).min(balance::SHOP_MAX_CASH_CAP);
            }
        }
        "cash_3" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.cash_cap = (shop.cash_cap + 10_000.0).min(balance::SHOP_MAX_CASH_CAP);
            }
        }
        "cash_4" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.cash_cap = (shop.cash_cap + 20_000.0).min(balance::SHOP_MAX_CASH_CAP);
            }
        }
        "cash_5" => {
            if let Some(shop) = state.city.shop.as_mut() {
                shop.cash_cap = (shop.cash_cap + 50_000.0).min(balance::SHOP_MAX_CASH_CAP);
            }
        }
        "stock_1" | "stock_2" | "stock_3" => {
            state.city.stock_cells_bought += 1;
        }
        _ => {}
    }
    Ok(())
}

// ---------- Склад приказчика ----------

/// Сколько ячеек на складе приказчика: базовые плюс купленные улучшения.
pub fn clerk_stock_cells(state: &GameState) -> u32 {
    balance::CLERK_START_CELLS + state.city.stock_cells_bought * balance::CLERK_CELLS_PER_UPGRADE
}

/// Кладёт товар из сумки на склад приказчика.
pub fn add_stock(state: &mut GameState, item_id: &str, quantity: u32) -> Result<(), ShopError> {
    if quantity == 0 || state.amount_of(item_id) < quantity {
        return Err(ShopError::MissingItem);
    }
    if !is_shop_sellable(state, item_id) {
        return Err(ShopError::Unsellable);
    }
    if let Some(row) = state
        .city
        .stock
        .iter_mut()
        .find(|row| row.item_id == item_id)
    {
        row.quantity += quantity;
    } else {
        if state.city.stock.len() >= clerk_stock_cells(state) as usize {
            return Err(ShopError::SlotLimit);
        }
        state.city.stock.push(StockItem {
            item_id: item_id.to_string(),
            quantity,
        });
    }
    state.remove_item(item_id, quantity);
    Ok(())
}

/// Сколько штук со склада приказчик выставит на полку: не больше партии,
/// но и дорогую вещь он не выкладывает — такие берёт только VIP.
fn clerk_batch(item_id: &str) -> u32 {
    let price = items::item(item_id)
        .map(|item| item.base_price as f64)
        .unwrap_or(0.0);
    if price > balance::SHOP_VIP_ONLY_PRICE {
        1
    } else {
        balance::CLERK_BATCH
    }
}

/// Покупается слух в таверне: пара <мёртвый / горячий>.
pub fn buy_rumor(state: &mut GameState) -> Result<RumorPair, TavernError> {
    let timestamp = now(state);
    if timestamp < state.city.next_rumor_at {
        return Err(TavernError::Cooldown);
    }
    if state.gold + 1e-9 < balance::TAVERN_RUMOR_PRICE {
        return Err(TavernError::NotEnoughGold);
    }
    state.gold -= balance::TAVERN_RUMOR_PRICE;
    state.city.next_rumor_at = timestamp + balance::TAVERN_RUMOR_COOLDOWN;
    let pair = roll_rumor_pair(state).unwrap_or_else(|| RumorPair {
        dead: RUMOR_ITEMS[1].to_string(),
        hot: RUMOR_ITEMS[0].to_string(),
    });
    state.city.active_event = Some(CityEvent::Rumor {
        item_id: pair.hot.clone(),
        dead_item: pair.dead.clone(),
        ends_at: timestamp + balance::TAVERN_RUMOR_DURATION,
    });
    Ok(pair)
}

/// Игра в кости: ставка 100-1000, сумма ≥8 - ×2, ровно 7 - возврат.
pub fn play_dice(state: &mut GameState, bet: f64) -> Result<DiceResult, TavernError> {
    let bet = bet.round();
    if !(balance::TAVERN_DICE_MIN_BET..=balance::TAVERN_DICE_MAX_BET).contains(&bet) {
        return Err(TavernError::BadBet);
    }
    if state.gold + 1e-9 < bet {
        return Err(TavernError::NotEnoughGold);
    }
    state.gold -= bet;
    let first = super::trader::random_below(&mut state.market_seed, 6) + 1;
    let second = super::trader::random_below(&mut state.market_seed, 6) + 1;
    let sum = first + second;
    if sum >= 8 {
        state.gold = (state.gold + bet * 2.0).min(state.gold_cap());
        Ok(DiceResult::Win)
    } else if sum == 7 {
        state.gold = (state.gold + bet).min(state.gold_cap());
        Ok(DiceResult::Draw)
    } else {
        Ok(DiceResult::Lose)
    }
}

/// Пожертвование в храм: благословение тем дольше, чем больше сумма.
pub fn donate(state: &mut GameState, amount: f64) -> Result<(), ChurchError> {
    if !balance::CHURCH_DONATIONS
        .iter()
        .any(|allowed| (allowed - amount).abs() < 1e-6)
    {
        return Err(ChurchError::BadAmount);
    }
    if state.gold + 1e-9 < amount {
        return Err(ChurchError::NotEnoughGold);
    }
    state.gold -= amount;
    let duration = amount.sqrt() * balance::CHURCH_BLESSING_SECS_PER_SQRT_GOLD;
    state.city.blessing_until = now(state) + duration;
    Ok(())
}

/// Исповедь: 500 золота и сброс пропущенных налогов, раз в 24 часа.
pub fn confess(state: &mut GameState) -> Result<(), ChurchError> {
    let timestamp = now(state);
    if timestamp < state.city.next_confession_at {
        return Err(ChurchError::Cooldown);
    }
    if state.gold + 1e-9 < balance::CHURCH_CONFESSION_PRICE {
        return Err(ChurchError::NotEnoughGold);
    }
    state.gold -= balance::CHURCH_CONFESSION_PRICE;
    state.city.next_confession_at = timestamp + balance::CHURCH_CONFESSION_COOLDOWN;
    state.city.taxes_missed = 0;
    Ok(())
}

/// Починка после пожара (500 золота) или отказ (рейтинг −1).
pub fn resolve_fire(state: &mut GameState, repair: bool) -> bool {
    if !state.city.fire_pending {
        return false;
    }
    if repair {
        if state.gold + 1e-9 < balance::CITY_EVENT_FIRE_REPAIR_GOLD {
            return false;
        }
        state.gold -= balance::CITY_EVENT_FIRE_REPAIR_GOLD;
    } else if let Some(shop) = state.city.shop.as_mut() {
        shop.rating = (shop.rating - balance::CITY_TAX_RATING_PENALTY).max(0.0);
    }
    state.city.fire_pending = false;
    true
}

/// Подкуп конкурента за 6000 золота.
pub fn bribe_competitor(state: &mut GameState) -> bool {
    if !competitor_active(state) {
        return false;
    }
    if state.gold + 1e-9 < balance::CITY_EVENT_BRIBE_GOLD {
        return false;
    }
    state.gold -= balance::CITY_EVENT_BRIBE_GOLD;
    state.city.active_event = None;
    true
}

// ---------- Наёмные линии ----------

/// Линия, на которую можно нанять работников.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HireLine {
    Sawmill,
    Mine,
    Coal,
}

impl HireLine {
    pub const ALL: [HireLine; 3] = [Self::Sawmill, Self::Mine, Self::Coal];

    /// Название линии в интерфейсе.
    pub fn title(self) -> &'static str {
        match self {
            Self::Sawmill => i18n::t("build.sawmill"),
            Self::Mine => i18n::t("build.mine"),
            Self::Coal => i18n::t("build.coal_pit"),
        }
    }
}

/// Сколько наёмных работает на линии.
pub fn hired_count(state: &GameState, line: HireLine) -> u32 {
    match line {
        HireLine::Sawmill => state.city.hired_sawmill,
        HireLine::Mine => state.city.hired_mine,
        HireLine::Coal => state.city.hired_coal,
    }
}

/// Сколько наёмных линия может взять: по одному на уровень постройки.
pub fn hired_cap(state: &GameState, line: HireLine) -> u32 {
    match line {
        HireLine::Sawmill => state.sawmill_level,
        HireLine::Mine => state.mine_level,
        HireLine::Coal => state.coal_pit_level,
    }
}

/// Меняет число наёмных на линии. Зарплата списывается общим циклом.
pub fn adjust_hired(state: &mut GameState, line: HireLine, delta: i32) -> Result<(), HireError> {
    let cap = hired_cap(state, line);
    if cap == 0 {
        return Err(HireError::NoLine);
    }
    let current = hired_count(state, line);
    let next = if delta < 0 {
        current.saturating_sub((-delta) as u32)
    } else {
        if !state.city.hire_charter {
            return Err(HireError::NoCharter);
        }
        (current + delta as u32).min(cap)
    };
    match line {
        HireLine::Sawmill => state.city.hired_sawmill = next,
        HireLine::Mine => state.city.hired_mine = next,
        HireLine::Coal => state.city.hired_coal = next,
    }
    Ok(())
}

/// Зарплата всех наёмных за цикл.
pub fn hired_salary(state: &GameState) -> f64 {
    (state.city.hired_sawmill + state.city.hired_mine + state.city.hired_coal) as f64
        * balance::HIRE_SALARY
}

/// Вся зарплата за цикл: наёмные, кузнецы, приказчик.
pub fn salary_total(state: &GameState) -> f64 {
    let mut total = hired_salary(state);
    total += smith_wages(state);
    if state.city.clerk_hired {
        total += balance::CLERK_SALARY;
    }
    total
}

/// Накопленное жалованье всех построенных кузнецов.
pub fn smith_wages(state: &GameState) -> f64 {
    state
        .city
        .smiths
        .iter()
        .filter(|smith| smith.built)
        .map(|smith| smith.wage_due)
        .sum()
}

/// Жалованная грамота на наём: разовый документ за крупную сумму.
pub fn buy_charter(state: &mut GameState) -> Result<(), HireError> {
    if !state.city.town_hall_unlocked {
        return Err(HireError::NoTownHall);
    }
    if state.city.hire_charter {
        return Err(HireError::AlreadyHave);
    }
    if state.gold + 1e-9 < balance::CHARTER_COST {
        return Err(HireError::NotEnoughGold);
    }
    state.gold -= balance::CHARTER_COST;
    state.city.hire_charter = true;
    state.add_letter("charter_granted", None);
    Ok(())
}

/// Письмо-счёт по зарплатам. Живёт в колокольчике постоянно: прочитанное
/// письмо не исчезает, а показывает итог последнего начисления и момент
/// следующего. Отдельный `stage` нужен потому, что текст собирается на
/// лету: без него повторный тик переписывал бы «выплачено» на «не хватило».
pub fn record_salary_bill(state: &mut GameState, stage: state::SalaryBillStage) {
    state.salary_log_at = state.total_play_seconds;
    state.salary_bill_stage = stage;
    state.add_letter("salary_bill", None);
}

/// Текст письма-счёта: сумма за цикл, кто её набрал и итог прошлой выплаты.
pub fn salary_bill_line(state: &GameState) -> String {
    let total = salary_total(state);
    let workers = state.city.hired_sawmill + state.city.hired_mine + state.city.hired_coal;
    let smiths = state.city.smiths.iter().filter(|smith| smith.built).count();
    let clerk = u32::from(state.city.clerk_hired);
    i18n::tf(
        "salary.bill",
        &[
            &format!("{}", total.floor() as i64),
            &workers.to_string(),
            &smiths.to_string(),
            &clerk.to_string(),
        ],
    )
}

/// Сколько осталось до следующего счёта по зарплатам.
pub fn salary_due_in(state: &GameState) -> Option<f64> {
    if state.city.next_salary_at <= 0.0 {
        return None;
    }
    Some((state.city.next_salary_at - state.total_play_seconds).max(0.0))
}

/// Выплата зарплат раз в 30 минут.
///
/// Если золота не хватило, наёмные уходят, приказчик увольняется, а кузнецы
/// встают до следующего цикла. Жалованье кузнецов копится за каждую поделку
/// и гасится здесь же. Возвращает итог для тестов и писем.
pub fn pay_salaries(state: &mut GameState) -> SalaryOutcome {
    // Счёт выставляется всегда, даже пустой: тогда в колокольчике видно,
    // когда был прошлый расчёт и сколько он стоил.
    let total = salary_total(state);
    if total <= 1e-6 {
        // Счёт не создаём в пустом городе, но если он уже есть — обновляем,
        // чтобы в колокольчике всегда была последняя сумма.
        if state.letters.iter().any(|letter| letter.id == "salary_bill") {
            record_salary_bill(state, state::SalaryBillStage::Paid);
        }
        return SalaryOutcome::Nothing;
    }
    if state.gold + 1e-9 >= total {
        state.gold -= total;
        for smith in &mut state.city.smiths {
            smith.wage_due = 0.0;
            smith.paused = false;
        }
        record_salary_bill(state, state::SalaryBillStage::Paid);
        return SalaryOutcome::Paid;
    }
    let had_hired = state.city.hired_sawmill + state.city.hired_mine + state.city.hired_coal > 0;
    state.city.hired_sawmill = 0;
    state.city.hired_mine = 0;
    state.city.hired_coal = 0;
    if state.city.clerk_hired {
        state.city.clerk_hired = false;
    }
    for smith in &mut state.city.smiths {
        if smith.built {
            smith.paused = true;
        }
    }
    record_salary_bill(state, state::SalaryBillStage::Unpaid);
    if had_hired {
        state.add_letter("hired_left", None);
    }
    SalaryOutcome::Unpaid
}

// ---------- Путешественники в таверне ----------

/// Один постоянный путешественник: имя, прозвище и специализация.
pub struct TravelerProfileDef {
    /// Ключ имени: `traveler.name.<id>`.
    pub id: &'static str,
    /// Ключ специализации: `traveler.spec.<id>`.
    pub specialism: &'static str,
}

/// Три постоянных жителя таверны. После постройки Дома странников к ним
/// добавятся ещё двое (веха v0.22, пятый блок).
pub const TRAVELER_PROFILES: [TravelerProfileDef; 3] = [
    TravelerProfileDef {
        id: "ivan",
        specialism: "forester",
    },
    TravelerProfileDef {
        id: "maria",
        specialism: "mountain_guide",
    },
    TravelerProfileDef {
        id: "stefan",
        specialism: "miner",
    },
];

/// Сколько кружков дружбы нужно, чтобы путешественник задал очередной квест.
/// Два кружка — первый разговорный квест, три — второй, четыре — компас.
pub const TRAVELER_QUEST_STEPS: [u32; 3] = [2, 3, 4];

/// Сколько кружков дружбы набирается, чтобы стать другом.
pub const TRAVELER_FRIEND_CIRCLES: u32 = 5;

/// Сколько кружков рисуется в карточке.
pub const TRAVELER_CIRCLES: u32 = 5;

/// Предмет, который путешественник просит принести.
pub const TRAVELER_COMPASS_ITEM: &str = "compass";

/// Тип квеста путешественника.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelerQuestKind {
    /// Разговор: игрок выбирает ответ.
    Story,
    /// Принести изготовленный компас.
    Compass,
}

/// Вариант ответа в разговорном квесте.
pub struct TravelerChoiceDef {
    pub id: &'static str,
    /// Правильный ли ответ (успех — прибавка дружбы).
    pub good: bool,
}

/// Разговорный квест путешественника.
pub struct TravelerQuestDef {
    /// Ключ `traveler_quest.<id>.title/.intro`.
    pub id: &'static str,
    pub kind: TravelerQuestKind,
    pub choices: &'static [TravelerChoiceDef],
}

const KIOSK_CHOICES: &[TravelerChoiceDef] = &[
    TravelerChoiceDef {
        id: "market",
        good: true,
    },
    TravelerChoiceDef {
        id: "temple",
        good: false,
    },
    TravelerChoiceDef {
        id: "tower",
        good: false,
    },
];

const KAI_CHOICES: &[TravelerChoiceDef] = &[
    TravelerChoiceDef {
        id: "owner",
        good: true,
    },
    TravelerChoiceDef {
        id: "keep",
        good: false,
    },
    TravelerChoiceDef {
        id: "split",
        good: false,
    },
];

/// Перевал в тумане: совет старухи у костра.
const PASS_CHOICES: &[TravelerChoiceDef] = &[
    TravelerChoiceDef {
        id: "salt",
        good: true,
    },
    TravelerChoiceDef {
        id: "raven",
        good: false,
    },
    TravelerChoiceDef {
        id: "night",
        good: false,
    },
];

/// Разбойники без оружия: отдать половину и уйти живым.
const BANDITS_CHOICES: &[TravelerChoiceDef] = &[
    TravelerChoiceDef {
        id: "share",
        good: true,
    },
    TravelerChoiceDef {
        id: "run",
        good: false,
    },
    TravelerChoiceDef {
        id: "fight",
        good: false,
    },
];

/// Пул разговорных квестов плюс квест с компасом в конце.
/// Разговорные истории выдаются в случайном порядке, компас — последним.
pub const TRAVELER_QUESTS: [TravelerQuestDef; 13] = [
    TravelerQuestDef {
        id: "captain_compass",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "gave",
                good: true,
            },
            TravelerChoiceDef {
                id: "kept",
                good: false,
            },
            TravelerChoiceDef {
                id: "sold",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "merchant_leather",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "merchant",
                good: false,
            },
            TravelerChoiceDef {
                id: "tanner",
                good: true,
            },
            TravelerChoiceDef {
                id: "price",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "campfire_song",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "old_soldier",
                good: true,
            },
            TravelerChoiceDef {
                id: "guide",
                good: false,
            },
            TravelerChoiceDef {
                id: "wind",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "last_city",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "forge",
                good: false,
            },
            TravelerChoiceDef {
                id: "family",
                good: true,
            },
            TravelerChoiceDef {
                id: "name",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "road_language",
        kind: TravelerQuestKind::Story,
        choices: KIOSK_CHOICES,
    },
    TravelerQuestDef {
        id: "desert_well",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "drink",
                good: false,
            },
            TravelerChoiceDef {
                id: "test",
                good: true,
            },
            TravelerChoiceDef {
                id: "pass",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "forest_noise",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "wolf",
                good: false,
            },
            TravelerChoiceDef {
                id: "deer",
                good: true,
            },
            TravelerChoiceDef {
                id: "spirit",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "two_roads",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "forest",
                good: false,
            },
            TravelerChoiceDef {
                id: "field",
                good: true,
            },
            TravelerChoiceDef {
                id: "back",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "trade_quarrel",
        kind: TravelerQuestKind::Story,
        choices: &[
            TravelerChoiceDef {
                id: "seller",
                good: false,
            },
            TravelerChoiceDef {
                id: "guard",
                good: true,
            },
            TravelerChoiceDef {
                id: "nobody",
                good: false,
            },
        ],
    },
    TravelerQuestDef {
        id: "finders_keepers",
        kind: TravelerQuestKind::Story,
        choices: KAI_CHOICES,
    },
    TravelerQuestDef {
        id: "pass_fog",
        kind: TravelerQuestKind::Story,
        choices: PASS_CHOICES,
    },
    TravelerQuestDef {
        id: "bandits_unarmed",
        kind: TravelerQuestKind::Story,
        choices: BANDITS_CHOICES,
    },
    TravelerQuestDef {
        id: "captain_compass_ask",
        kind: TravelerQuestKind::Compass,
        choices: &[],
    },
];

/// Квест по id.
pub fn traveler_quest(quest_id: &str) -> Option<&'static TravelerQuestDef> {
    TRAVELER_QUESTS.iter().find(|quest| quest.id == quest_id)
}

/// Данные путешественника по номеру в списке.
pub fn traveler_profile(profile_id: usize) -> &'static TravelerProfileDef {
    TRAVELER_PROFILES
        .get(profile_id)
        .unwrap_or(&TRAVELER_PROFILES[0])
}

/// Имя с прозвищем на текущем языке (например, «Иван «Бородач»»).
pub fn traveler_name(profile_id: usize) -> &'static str {
    let profile = traveler_profile(profile_id);
    i18n::t(&format!("traveler.name.{}", profile.id))
}

/// Специализация на текущем языке.
pub fn traveler_specialism(profile_id: usize) -> &'static str {
    let profile = traveler_profile(profile_id);
    i18n::t(&format!("traveler.spec.{}", profile.specialism))
}

/// Сколько полных кружков дружбы у путешественника.
pub fn traveler_circles(traveler: &Traveler) -> u32 {
    (traveler.friendship / balance::TRAVELER_FRIENDSHIP_POINT).min(TRAVELER_CIRCLES)
}

/// Свободен ли путешественник (не в походе).
pub fn traveler_ready(traveler: &Traveler) -> bool {
    traveler.returns_at <= 0.0
}

/// Стал ли путешественник другом.
pub fn traveler_is_friend(traveler: &Traveler) -> bool {
    traveler_circles(traveler) >= TRAVELER_FRIEND_CIRCLES
}

/// Следующий квест персонажа, если дружба до него доросла.
pub fn next_traveler_quest(traveler: &Traveler) -> Option<&'static TravelerQuestDef> {
    if traveler.active_quest.is_some() || !traveler.carried.is_empty() {
        return None;
    }
    let circles = traveler_circles(traveler);
    if circles < TRAVELER_QUEST_STEPS[0] {
        return None;
    }
    if traveler_is_friend(traveler) {
        return None;
    }
    // Компас — только после двух разговорных квестов.
    if circles >= TRAVELER_QUEST_STEPS[2] {
        let last = TRAVELER_QUESTS
            .iter()
            .find(|quest| quest.kind == TravelerQuestKind::Compass)
            .expect("compass quest exists");
        if !traveler.quests_asked.iter().any(|id| id == last.id) {
            return Some(last);
        }
    }
    // Разговорные квесты выдаются в порядке пула.
    TRAVELER_QUESTS
        .iter()
        .filter(|quest| quest.kind == TravelerQuestKind::Story)
        .find(|quest| {
            !traveler.quests_asked.iter().any(|id| id == quest.id)
                && !traveler.quests_failed.iter().any(|id| id == quest.id)
        })
}

/// Что сейчас можно сделать с путешественником: строка квеста в карточке.
#[derive(Clone, Debug, PartialEq)]
pub enum TravelerQuestStep {
    /// Ничего: дружбы не хватает.
    None,
    /// Можно начать квест (`next_traveler_quest`).
    Start(String),
    /// Квест начат и ждёт решения игрока.
    Active(String),
    /// Квест пройден, нужно закрыть реплику.
    Result {
        quest: String,
        good: bool,
        friend: bool,
    },
    /// Путешественник стал другом и предлагает построить Дом странников.
    OfferHouse,
}

impl TravelerQuestStep {
    /// Что сейчас можно сделать с путешественником.
    pub fn of(traveler: &Traveler) -> Self {
        if let Some(quest) = traveler.active_quest.clone() {
            if traveler.quest_result {
                let friend = traveler_is_friend(traveler);
                let good = !traveler.quests_failed.contains(&quest);
                return Self::Result {
                    quest,
                    good,
                    friend,
                };
            }
            return Self::Active(quest);
        }
        if traveler_is_friend(traveler) {
            return Self::OfferHouse;
        }
        match next_traveler_quest(traveler) {
            Some(quest) => Self::Start(quest.id.to_string()),
            None => Self::None,
        }
    }
}

/// Создаёт троих постоянных жителей таверны, если их ещё нет.
///
/// До открытия города путешественников нет: их приносит с собой город.
pub fn ensure_travelers(state: &mut GameState) {
    if !state.city.unlocked {
        return;
    }
    if state.city.travelers.len() >= balance::TRAVELER_COUNT {
        return;
    }
    let existing: Vec<usize> = state
        .city
        .travelers
        .iter()
        .map(|traveler| traveler.profile_id)
        .collect();
    for profile_id in 0..balance::TRAVELER_COUNT {
        if existing.contains(&profile_id) {
            continue;
        }
        state.city.travelers.push(Traveler {
            profile_id,
            ..Traveler::default()
        });
    }
    state
        .city
        .travelers
        .sort_by_key(|traveler| traveler.profile_id);
}

/// Начинает квест путешественника.
pub fn start_traveler_quest(state: &mut GameState, index: usize) -> Result<(), TravelerError> {
    ensure_travelers(state);
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if !traveler_ready(traveler) {
        return Err(TravelerError::Busy);
    }
    if traveler.active_quest.is_some() {
        return Err(TravelerError::QuestDecided);
    }
    let Some(next) = next_traveler_quest(traveler) else {
        return Err(TravelerError::NoQuest);
    };
    // Компас — последним, а разговорные истории выдаются в случайном
    // порядке: одна и та же последовательность наскучила бы к третьему
    // персонажу.
    let id = if next.kind == TravelerQuestKind::Compass {
        next.id.to_string()
    } else {
        let traveler = &state.city.travelers[index];
        let pool: Vec<&'static str> = TRAVELER_QUESTS
            .iter()
            .filter(|quest| quest.kind == TravelerQuestKind::Story)
            .filter(|quest| {
                !traveler.quests_asked.iter().any(|id| id == quest.id)
                    && !traveler.quests_failed.iter().any(|id| id == quest.id)
            })
            .map(|quest| quest.id)
            .collect();
        if pool.is_empty() {
            return Err(TravelerError::NoQuest);
        }
        let mut seed = state.market_seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let picked = super::trader::random_below(&mut seed, pool.len() as u64) as usize;
        state.market_seed = seed;
        pool[picked].to_string()
    };
    let traveler = &mut state.city.travelers[index];
    traveler.quests_asked.push(id.clone());
    traveler.active_quest = Some(id);
    traveler.quest_result = false;
    Ok(())
}

/// Начисляет или снимает дружбу, не выходя за границы.
fn change_friendship(traveler: &mut Traveler, gain: u32, loss: u32) {
    if gain > 0 {
        let room = balance::TRAVELER_FRIENDSHIP_MAX.saturating_sub(traveler.friendship);
        traveler.friendship += gain.min(room);
    }
    if loss > 0 {
        traveler.friendship = traveler.friendship.saturating_sub(loss);
    }
}

/// Отвечает на разговорный квест. Возвращает `true`, если ответ угадан.
pub fn answer_traveler_quest(
    state: &mut GameState,
    index: usize,
    choice_id: &str,
) -> Result<bool, TravelerError> {
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if traveler.quest_result {
        return Err(TravelerError::QuestDecided);
    }
    let Some(quest_id) = traveler.active_quest.clone() else {
        return Err(TravelerError::NoQuest);
    };
    let Some(quest) = traveler_quest(&quest_id) else {
        return Err(TravelerError::NoQuest);
    };
    let Some(choice) = quest.choices.iter().find(|choice| choice.id == choice_id) else {
        return Err(TravelerError::NoQuest);
    };
    let good = choice.good;
    let traveler = &mut state.city.travelers[index];
    if good {
        change_friendship(traveler, balance::TRAVELER_QUEST_GAIN, 0);
    } else {
        // Промах не отнимает дружбу: история просто уходит из пула и
        // у этого персонажа больше не появится.
        traveler.quests_failed.push(quest_id);
    }
    traveler.quest_result = true;
    Ok(good)
}

/// Закрывает реплику о результате квеста.
pub fn finish_traveler_quest(state: &mut GameState, index: usize) -> Result<(), TravelerError> {
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if !traveler.quest_result {
        return Err(TravelerError::NoQuest);
    }
    let traveler = &mut state.city.travelers[index];
    traveler.active_quest = None;
    traveler.quest_result = false;
    Ok(())
}

/// Принимает принесённый компас: последний квест, после которого
/// путешественник становится другом.
pub fn deliver_compass(state: &mut GameState, index: usize) -> Result<(), TravelerError> {
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if traveler.quest_result {
        return Err(TravelerError::QuestDecided);
    }
    let Some(quest_id) = traveler.active_quest.clone() else {
        return Err(TravelerError::NoQuest);
    };
    let Some(quest) = traveler_quest(&quest_id) else {
        return Err(TravelerError::NoQuest);
    };
    if quest.kind != TravelerQuestKind::Compass {
        return Err(TravelerError::NoQuest);
    }
    if state.item_count(TRAVELER_COMPASS_ITEM) == 0 {
        return Err(TravelerError::NoCompass);
    }
    state.remove_item(TRAVELER_COMPASS_ITEM, 1);
    let traveler = &mut state.city.travelers[index];
    change_friendship(traveler, balance::TRAVELER_QUEST_GAIN, 0);
    traveler.quest_result = true;
    state.city.traveler_house_offered = true;
    state.add_letter("traveler_compass_brought", None);
    Ok(())
}

/// Разговор с путешественником: 100 золотых и один кружок дружбы.
pub fn talk_to_traveler(state: &mut GameState, index: usize) -> Result<(), TravelerError> {
    ensure_travelers(state);
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if traveler.friendship >= balance::TRAVELER_FRIENDSHIP_MAX {
        return Err(TravelerError::NoQuest);
    }
    let price = balance::TRAVELER_TALK_PRICE;
    if state.gold + 1e-9 < price {
        return Err(TravelerError::NotEnoughGold);
    }
    state.gold -= price;
    let traveler = &mut state.city.travelers[index];
    change_friendship(traveler, balance::TRAVELER_QUEST_GAIN, 0);
    Ok(())
}

/// Ответил ли игрок на предложение построить Дом странников.
pub fn traveler_house_decision(state: &mut GameState, agree: bool) {
    state.city.traveler_house_answer = Some(agree);
    // Землю под Дом друзья предлагают в любом случае: и «Да», и «Нет»
    // открывают аренду в Ратуше, разница только в том, согласился игрок
    // сразу или отложил стройку.
    state.city.traveler_house_land = true;
}

/// Отправляет путешественника в локацию за аванс.
pub fn send_traveler(
    state: &mut GameState,
    index: usize,
    location_id: &str,
) -> Result<(), TravelerError> {
    ensure_travelers(state);
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if !traveler_ready(traveler) {
        return Err(TravelerError::Busy);
    }
    if !traveler.carried.is_empty() || traveler.payout > 1e-9 {
        return Err(TravelerError::LootWaiting);
    }
    let Some(location) = balance::TRAVELER_LOCATIONS
        .iter()
        .find(|location| location.id == location_id)
    else {
        return Err(TravelerError::BadTraveler);
    };
    if location.needs_house && !traveler_house_built(state) {
        return Err(TravelerError::BadTraveler);
    }
    if state.gold + 1e-9 < location.advance {
        return Err(TravelerError::NotEnoughGold);
    }
    state.gold -= location.advance;
    let timestamp = now(state);
    let traveler = &mut state.city.travelers[index];
    traveler.returns_at = timestamp + location.minutes * 60.0;
    traveler.trip = location.id.to_string();
    traveler.advance = location.advance;
    traveler.carried.clear();
    traveler.payout = 0.0;
    traveler.last_empty = false;
    Ok(())
}

/// Снаряжение путешественника даёт прибавку к луту и к доплате.
///
/// `gather_bonus` и `expedition_bonus` предмета работают на всю добычу,
/// `trade_bonus`/`gold_bonus` — на доплату за лут.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TravelerGearBonus {
    /// Прибавка к любой добыче.
    pub all: f64,
    /// Прибавка к доплате за лут.
    pub payout: f64,
}

/// Считает бонусы снаряжения путешественника.
pub fn traveler_gear_bonus(state: &GameState, index: usize) -> TravelerGearBonus {
    let mut bonus = TravelerGearBonus::default();
    let Some(traveler) = state.city.travelers.get(index) else {
        return bonus;
    };
    for item_id in traveler.gear.iter().flatten() {
        let Some(item) = items::item(item_id) else {
            continue;
        };
        let stats = &item.stats;
        let step = balance::TRAVELER_GEAR_LOOT_STEP / 100.0;
        if let Some(value) = stats.gather_bonus {
            bonus.all += value as f64 * step;
        }
        if let Some(value) = stats.expedition_bonus {
            bonus.all += value as f64 * step;
        }
        if let Some(value) = stats.trade_bonus {
            bonus.payout += value as f64 / 100.0;
        }
        if let Some(value) = stats.gold_bonus {
            bonus.payout += value as f64 / 100.0;
        }
    }
    bonus
}

/// Прибавка снаряжения к добыче конкретного ресурса.
fn gear_share(bonus: TravelerGearBonus) -> f64 {
    bonus.all.clamp(0.0, 2.0)
}

/// Золотая стоимость партии лута по базовым ценам.
pub fn traveler_loot_value(loot: &[(String, u32)]) -> f64 {
    loot.iter()
        .filter_map(|(item_id, quantity)| {
            items::item(item_id).map(|item| item.base_price as f64 * *quantity as f64)
        })
        .sum()
}

/// Доплата за лут: 15 % стоимости, но не меньше 5 %, если в снаряжении
/// есть торговый бонус (от 15 % доля ползёт вплоть до полной стоимости).
pub fn traveler_payout_value(loot: &[(String, u32)], gear: TravelerGearBonus) -> f64 {
    let value = traveler_loot_value(loot);
    if value <= 0.0 {
        return 0.0;
    }
    let mut share = balance::TRAVELER_PAYOUT_LOOT_SHARE
        + (balance::TRAVELER_PAYOUT_PERCENT - balance::TRAVELER_PAYOUT_LOOT_SHARE)
            * gear.payout;
    if gear.payout > 0.0 {
        share = share.max(balance::TRAVELER_PAYOUT_MIN_WITH_TRADE);
    }
    (value * share.clamp(0.0, 1.0)).round()
}

/// Возвращает путешественников из похода, копит лут и доплату.
///
/// Лут не попадает в сумку сам: он ждёт у путешественника, пока игрок
/// не заберёт его кнопкой «Забрать лут».
pub fn tick_travelers(state: &mut GameState) {
    ensure_travelers(state);
    let timestamp = now(state);
    for index in 0..state.city.travelers.len() {
        let (returns_at, trip) = {
            let traveler = &state.city.travelers[index];
            (traveler.returns_at, traveler.trip.clone())
        };
        if returns_at <= 0.0 || timestamp < returns_at {
            continue;
        }
        let gear = traveler_gear_bonus(state, index);
        let mut loot = roll_traveler_loot(state, &trip);
        for (_, quantity) in loot.iter_mut() {
            let share = gear_share(gear);
            *quantity = ((*quantity as f64) * (1.0 + share)).round() as u32;
        }
        let payout = traveler_payout_value(&loot, gear);
        let carried: Vec<(String, u32)> = loot.iter().filter(|(_, qty)| *qty > 0).cloned().collect();
        let traveler = &mut state.city.travelers[index];
        traveler.returns_at = 0.0;
        traveler.trip.clear();
        traveler.advance = 0.0;
        traveler.last_empty = carried.is_empty();
        traveler.carried = carried;
        traveler.payout = payout;
        let letter_id = if traveler.last_empty {
            format!("traveler_empty:{}", traveler.profile_id)
        } else {
            format!("traveler_back:{}", traveler.profile_id)
        };
        if !state.letters.iter().any(|letter| letter.id == letter_id) {
            state.add_letter(&letter_id, None);
        } else if let Some(letter) = state
            .letters
            .iter_mut()
            .find(|letter| letter.id == letter_id)
        {
            letter.read = false;
        }
    }
}

/// Кладёт в сумку всё, что принёс путешественник, и платит доплату.
///
/// Что не влезло — остаётся у путешественника до следующего раза.
pub fn claim_traveler_loot(state: &mut GameState, index: usize) -> Result<(), TravelerError> {
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if traveler.carried.is_empty() && traveler.payout <= 0.0 {
        return Err(TravelerError::NothingToClaim);
    }
    let loot = traveler.carried.clone();
    let payout = traveler.payout;
    let mut left: Vec<(String, u32)> = Vec::new();
    for (item_id, quantity) in loot {
        let added = if super::state::is_active_resource(&item_id) {
            state.add_active(&item_id, quantity)
        } else if state.add_item(&item_id, quantity) {
            quantity
        } else {
            0
        };
        if added < quantity {
            left.push((item_id, quantity - added));
        }
    }
    let traveler = &mut state.city.travelers[index];
    traveler.carried = left;
    if traveler.carried.is_empty() {
        traveler.payout = 0.0;
        state.gold += payout;
    }
    Ok(())
}

/// Надевает снаряжение на путешественника — только когда Дом странников
/// построен, предмет лежит в сумке и не надет на героя.
pub fn equip_traveler_gear(
    state: &mut GameState,
    index: usize,
    slot: usize,
    item_id: &str,
) -> Result<(), TravelerError> {
    if !traveler_house_built(state) {
        return Err(TravelerError::BadTraveler);
    }
    if slot >= 3 {
        return Err(TravelerError::BadTraveler);
    }
    let Some(traveler) = state.city.travelers.get(index) else {
        return Err(TravelerError::BadTraveler);
    };
    if traveler.gear.iter().flatten().any(|id| id == item_id) {
        return Err(TravelerError::GearSlotTaken);
    }
    if state.item_count(item_id) == 0 {
        return Err(TravelerError::MissingGear);
    }
    state.remove_item(item_id, 1);
    state.city.travelers[index].gear[slot] = Some(item_id.to_string());
    Ok(())
}

/// Снимает снаряжение с путешественника обратно в сумку.
pub fn unequip_traveler_gear(
    state: &mut GameState,
    index: usize,
    slot: usize,
) -> Result<(), TravelerError> {
    let Some(traveler) = state.city.travelers.get_mut(index) else {
        return Err(TravelerError::BadTraveler);
    };
    let Some(item_id) = traveler.gear.get_mut(slot).and_then(|slot| slot.take()) else {
        return Err(TravelerError::MissingGear);
    };
    if !state.add_item(&item_id, 1) {
        // В сумке нет места — вещь возвращается на своё место.
        state.city.travelers[index].gear[slot] = Some(item_id);
        return Err(TravelerError::NoSpace);
    }
    Ok(())
}

/// Бросок лута: сначала шанс пустого, затем `TRAVELER_LOOT_ROLLS` строк.
fn roll_traveler_loot(state: &mut GameState, location_id: &str) -> Vec<(String, u32)> {
    let Some(location) = balance::TRAVELER_LOCATIONS
        .iter()
        .find(|location| location.id == location_id)
    else {
        return Vec::new();
    };
    if random_unit(&mut state.market_seed) < location.empty_chance {
        return Vec::new();
    }
    let total: u64 = location.loot.iter().map(|row| row.weight as u64).sum();
    let mut loot: Vec<(String, u32)> = Vec::new();
    if total == 0 {
        return loot;
    }
    for _ in 0..balance::TRAVELER_LOOT_ROLLS {
        let mut pick = super::trader::random_below(&mut state.market_seed, total);
        let mut chosen = location.loot[0];
        for row in location.loot {
            if pick < row.weight as u64 {
                chosen = *row;
                break;
            }
            pick -= row.weight as u64;
        }
        let span = (chosen.max_quantity - chosen.min_quantity + 1) as u64;
        let quantity =
            chosen.min_quantity + super::trader::random_below(&mut state.market_seed, span) as u32;
        if let Some(entry) = loot.iter_mut().find(|(id, _)| id == chosen.item_id) {
            entry.1 += quantity;
        } else {
            loot.push((chosen.item_id.to_string(), quantity));
        }
    }
    loot
}

// ---------- Дом странников ----------

/// Арендован ли участок под Дом странников; с этого момента путешественники
/// живут уже не в таверне, а в своём доме.
pub fn traveler_house_land(state: &GameState) -> bool {
    state.city.traveler_house_rented
}

/// Согласился ли игрок на предложение друзей построить дом.
pub fn traveler_house_agreed(state: &GameState) -> bool {
    state.city.traveler_house_answer == Some(true)
}

/// Можно ли уже арендовать землю под дом: путешественники позвали игрока
/// в дело, и он ответил на предложение — «Да» или «Нет».
pub fn traveler_house_offer_made(state: &GameState) -> bool {
    state.city.traveler_house_offered
        && (state.city.traveler_house_land || traveler_house_agreed(state))
}

/// Земля под Дом странников: разовый платёж в Ратуше.
pub fn rent_traveler_house(state: &mut GameState) -> Result<(), TravelerError> {
    if !state.city.town_hall_unlocked {
        return Err(TravelerError::BadTraveler);
    }
    if state.city.traveler_house_rented {
        return Err(TravelerError::QuestDecided);
    }
    if !traveler_house_offer_made(state) {
        return Err(TravelerError::HouseNotOffered);
    }
    let price = balance::TRAVELER_HOUSE_RENT;
    if state.gold + 1e-9 < price {
        return Err(TravelerError::NotEnoughGold);
    }
    state.gold -= price;
    state.city.traveler_house_rented = true;
    Ok(())
}

/// Арендована ли земля под дом.
#[cfg(test)]
pub fn traveler_house_rented(state: &GameState) -> bool {
    state.city.traveler_house_rented
}

/// Построен ли Дом странников.
pub fn traveler_house_built(state: &GameState) -> bool {
    state.city.traveler_house_stage >= balance::TRAVELER_HOUSE_STAGES
}

/// Идёт ли сейчас очередной этап стройки.
pub fn traveler_house_building(state: &GameState) -> bool {
    state.city.traveler_house_ends_at > now(state)
}

/// Начинает очередной этап стройки: списывает материалы.
pub fn start_traveler_house_stage(state: &mut GameState) -> Result<(), TravelerError> {
    if traveler_house_built(state) {
        return Err(TravelerError::HouseBuilt);
    }
    if !traveler_house_offer_made(state) {
        return Err(TravelerError::HouseNotOffered);
    }
    if !state.city.traveler_house_rented {
        return Err(TravelerError::NoLand);
    }
    if traveler_house_building(state) {
        return Err(TravelerError::Building);
    }
    let stage = state.city.traveler_house_stage;
    let cost = balance::traveler_house_cost(stage);
    if !actions::can_afford(state, cost) {
        return Err(TravelerError::NoMaterials);
    }
    actions::spend_cost(state, cost);
    state.city.traveler_house_ends_at = now(state) + balance::TRAVELER_HOUSE_STAGE_SECS;
    Ok(())
}

/// Завершает этапы, у которых вышло время.
pub fn tick_traveler_house(state: &mut GameState) {
    if state.city.traveler_house_ends_at <= 0.0 {
        return;
    }
    if now(state) < state.city.traveler_house_ends_at {
        return;
    }
    state.city.traveler_house_ends_at = 0.0;
    state.city.traveler_house_stage += 1;
    if traveler_house_built(state) {
        state.add_letter("traveler_house_built", None);
        // Путешественники переезжают в дом: добавляются двое новых.
        extend_travelers(state);
    }
}

/// После постройки дома в таверне живут пятеро.
fn extend_travelers(state: &mut GameState) {
    for profile_id in TRAVELER_PROFILES.len()..balance::TRAVELER_TARGET_COUNT {
        if state
            .city
            .travelers
            .iter()
            .any(|traveler| traveler.profile_id == profile_id)
        {
            continue;
        }
        state.city.travelers.push(Traveler {
            profile_id,
            ..Traveler::default()
        });
    }
    state
        .city
        .travelers
        .sort_by_key(|traveler| traveler.profile_id);
}

// ---------- Кузнецы в «Труде» ----------

/// Линия улучшений кузнеца.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmithLine {
    /// Скорость: пауза между ковками.
    Speed,
    /// Горн: ёмкость и огонь с угля.
    Forge,
    /// Ячейки рецептов в очереди.
    Slots,
}

impl SmithLine {
    /// Название линии для интерфейса.
    pub fn title(self) -> &'static str {
        match self {
            Self::Speed => i18n::t("smith.speed"),
            Self::Forge => i18n::t("smith.forge"),
            Self::Slots => i18n::t("smith.slots"),
        }
    }

    /// Максимальный уровень линии.
    pub fn max_level(self) -> u32 {
        match self {
            Self::Speed => balance::SMITH_SPEED_MAX_LEVEL,
            Self::Forge => balance::SMITH_FORGE_MAX_LEVEL,
            Self::Slots => balance::SMITH_SLOTS_MAX_LEVEL,
        }
    }

    /// Текущий уровень линии у кузнеца.
    pub fn level(self, smith: &SmithState) -> u32 {
        match self {
            Self::Speed => smith.speed_level,
            Self::Forge => smith.forge_level,
            Self::Slots => smith.slots_level,
        }
    }

    /// Стоимость следующего уровня.
    pub fn cost(self, level: u32) -> balance::Cost {
        match self {
            Self::Speed => balance::smith_speed_cost(level),
            Self::Forge => balance::smith_forge_cost(level),
            Self::Slots => balance::smith_slots_cost(level),
        }
    }
}

/// Переносит старое сохранение города на новую модель спроса.
///
/// Раньше у товара были только налитая ёмкость и цель дрейфа. Теперь спрос
/// и предложение считаются по-торговому, поэтому старые записи получают
/// пару 50/50 и полную полку — первое же действие расставит всё по местам.
pub fn migrate_demand(state: &mut GameState) {
    for (item_id, entry) in state.city.demand.iter_mut() {
        if entry.demand != 0 || entry.supply != 0 {
            continue;
        }
        entry.demand = balance::DEMAND_EQUILIBRIUM;
        entry.supply = balance::DEMAND_EQUILIBRIUM;
        let capacity = demand_capacity(item_id);
        // Старое налитое считалось от прежней ёмкости и после перехода
        // на новую модель оказывалось в разы больше неё — рынок показывал
        // «774 793 388 419 / 500». Возвращаем долю, а не сырое число.
        let ratio = if entry.capacity > 1e-6 && entry.available.is_finite() {
            (entry.available / entry.capacity).clamp(0.0, 1.0)
        } else {
            1.0
        };
        entry.capacity = capacity;
        entry.available = (capacity * ratio).clamp(0.0, capacity);
    }
}

/// Переводит старое сохранение с домом кузнеца в гильдии на двух мастеров.
pub fn migrate_smiths(state: &mut GameState) {
    let level = state.city.legacy_smith_level;
    if level == 0 || state.city.smiths[0].built {
        return;
    }
    let level = level.min(balance::SMITH_FORGE_MAX_LEVEL);
    let smith = &mut state.city.smiths[0];
    smith.built = true;
    smith.speed_level = level.saturating_sub(1);
    smith.forge_level = level.saturating_sub(1);
    smith.slots_level = 0;
    smith.recipes = std::mem::take(&mut state.city.legacy_smith_recipes);
    smith
        .recipes
        .truncate(balance::smith_slots(smith.slots_level));
    smith.fire = balance::smith_fire_capacity(smith.forge_level);
    smith.next_craft_at = state.city.legacy_next_smith_craft_at;
    smith.paused = state.city.legacy_smith_paused;
    smith.wage_due = 0.0;
}

/// Строит кузнеца. Второй мастер требует разрешения ратуши.
pub fn build_smith(state: &mut GameState, index: usize) -> Result<(), SmithError> {
    if !state.city.unlocked || index >= balance::SMITH_COUNT {
        return Err(SmithError::Locked);
    }
    if state.city.smiths[index].built {
        return Err(SmithError::Built);
    }
    if index > 0 && !state.city.smith_second_permit {
        return Err(SmithError::MissingPermit);
    }
    let cost = balance::smith_build_cost(index);
    if !actions::can_afford(state, cost) {
        return Err(SmithError::NotEnough);
    }
    actions::spend_cost(state, cost);
    let fire = balance::smith_fire_capacity(state.city.smiths[index].forge_level);
    let interval = balance::smith_interval(state.city.smiths[index].speed_level);
    let timestamp = now(state);
    let smith = &mut state.city.smiths[index];
    smith.built = true;
    smith.paused = false;
    smith.fire = fire;
    smith.next_craft_at = timestamp + interval;
    state.add_letter("smith_house_built", None);
    Ok(())
}

/// Улучшает одну из трёх линий кузнеца.
pub fn upgrade_smith(
    state: &mut GameState,
    index: usize,
    line: SmithLine,
) -> Result<(), SmithError> {
    if index >= balance::SMITH_COUNT || !state.city.smiths[index].built {
        return Err(SmithError::NotBuilt);
    }
    let level = line.level(&state.city.smiths[index]);
    if level >= line.max_level() {
        return Err(SmithError::MaxLevel);
    }
    let cost = line.cost(level);
    if !actions::can_afford(state, cost) {
        return Err(SmithError::NotEnough);
    }
    actions::spend_cost(state, cost);
    let smith = &mut state.city.smiths[index];
    match line {
        SmithLine::Speed => smith.speed_level += 1,
        SmithLine::Forge => smith.forge_level += 1,
        SmithLine::Slots => smith.slots_level += 1,
    }
    Ok(())
}

/// Покупает в ратуше разрешение на вторую кузницу.
pub fn buy_smith_permit(state: &mut GameState) -> Result<(), SmithError> {
    if !state.city.town_hall_unlocked {
        return Err(SmithError::NoTownHall);
    }
    if state.city.smith_second_permit {
        return Err(SmithError::PermitOwned);
    }
    if state.gold + 1e-9 < balance::SMITH_SECOND_PERMIT_GOLD {
        return Err(SmithError::NotEnough);
    }
    state.gold -= balance::SMITH_SECOND_PERMIT_GOLD;
    state.city.smith_second_permit = true;
    Ok(())
}

/// Сколько рецептов помещается в очередь кузнеца.
pub fn smith_slots(state: &GameState, index: usize) -> usize {
    state
        .city
        .smiths
        .get(index)
        .map(|smith| balance::smith_slots(smith.slots_level))
        .unwrap_or(0)
}

/// Ставит рецепт в очередь кузнеца.
pub fn assign_smith_recipe(
    state: &mut GameState,
    index: usize,
    recipe_id: &str,
) -> Result<(), SmithError> {
    if index >= balance::SMITH_COUNT || !state.city.smiths[index].built {
        return Err(SmithError::NotBuilt);
    }
    let Some(recipe) = items::recipe(recipe_id) else {
        return Err(SmithError::UnknownRecipe);
    };
    if !actions::recipe_visible(state, recipe) {
        return Err(SmithError::RecipeLocked);
    }
    if state.city.smiths[index]
        .recipes
        .iter()
        .any(|id| id == recipe_id)
    {
        return Err(SmithError::AlreadyAssigned);
    }
    if state.city.smiths[index].recipes.len() >= smith_slots(state, index) {
        return Err(SmithError::Full);
    }
    state.city.smiths[index].recipes.push(recipe_id.to_string());
    Ok(())
}

/// Убирает рецепт из очереди кузнеца.
pub fn unassign_smith_recipe(state: &mut GameState, index: usize, recipe_id: &str) {
    if let Some(smith) = state.city.smiths.get_mut(index) {
        smith.recipes.retain(|id| id != recipe_id);
    }
}

/// Двигает рецепт по очереди: `delta` −1 вверх, +1 вниз.
pub fn move_smith_recipe(
    state: &mut GameState,
    index: usize,
    recipe_id: &str,
    delta: i32,
) -> Result<(), SmithError> {
    let Some(smith) = state.city.smiths.get_mut(index) else {
        return Err(SmithError::NotBuilt);
    };
    let Some(position) = smith.recipes.iter().position(|id| id == recipe_id) else {
        return Err(SmithError::NotInQueue);
    };
    let target = position as i32 + delta;
    if target < 0 || target as usize >= smith.recipes.len() {
        return Ok(());
    }
    smith.recipes.swap(position, target as usize);
    Ok(())
}

/// Хватает ли в сумке и на складе ингредиентов рецепта (без огня).
fn smith_has_ingredients(state: &GameState, recipe: &items::Recipe) -> bool {
    recipe
        .ingredients
        .iter()
        .filter(|ingredient| ingredient.item_id != "fire")
        .all(|ingredient| state.ingredient_amount(ingredient.item_id) >= ingredient.quantity)
}

/// Есть ли куда положить поделку: снаряжение — на склад приказчика,
/// остальное — в сумку.
fn smith_has_output_room(state: &GameState, recipe: &items::Recipe) -> bool {
    let Some(item) = items::item(recipe.result_id) else {
        return false;
    };
    if item.slot.is_some() && smith_can_stock(state, recipe.result_id) {
        return true;
    }
    state.can_add_crafted(recipe)
}

/// Поместится ли товар на склад приказчика.
fn smith_can_stock(state: &GameState, item_id: &str) -> bool {
    if state.city.shop.is_none() || !is_shop_sellable(state, item_id) {
        return false;
    }
    state.city.stock.iter().any(|row| row.item_id == item_id)
        || state.city.stock.len() < clerk_stock_cells(state) as usize
}

/// Подкладывает уголь в горн, пока огня не хватит на ковку.
///
/// `reserve` — уголь, который нужен самому рецепту: его нельзя сжигать
/// в горне. Возвращает `false`, если угля не хватило. Огонь не утекает:
/// лишний остаётся в горне до следующей поделки.
fn refuel_smith(state: &mut GameState, index: usize, need: u32, reserve: u32) -> bool {
    if need == 0 {
        return true;
    }
    let forge_level = state.city.smiths[index].forge_level;
    let capacity = balance::smith_fire_capacity(forge_level);
    let per_coal = balance::smith_fire_per_coal(forge_level).max(1);
    while state.city.smiths[index].fire < need {
        if state.city.smiths[index].fire >= capacity || state.active_amount("coal") <= reserve {
            break;
        }
        state.spend_active("coal", 1);
        let smith = &mut state.city.smiths[index];
        smith.fire = (smith.fire + per_coal).min(capacity);
    }
    state.city.smiths[index].fire >= need
}

/// Одна ковка по рецепту: списывает материалы и огонь горна, кладёт
/// результат на склад или в сумку, копит жалованье и опыт.
fn perform_smith_craft(state: &mut GameState, index: usize, recipe: &items::Recipe) -> bool {
    let Some(item) = items::item(recipe.result_id) else {
        return false;
    };
    let quantity = recipe.result_quantity;
    let to_stock = item.slot.is_some() && smith_can_stock(state, recipe.result_id);
    if !to_stock && !state.can_add_crafted(recipe) {
        return false;
    }
    let fire = actions::fire_cost(state, recipe);
    if state.city.smiths[index].fire < fire {
        return false;
    }

    let discount = state.ingredient_discount.clamp(0.0, 0.9) as f64;
    let mut seed = state.market_seed;
    for ingredient in &recipe.ingredients {
        if ingredient.item_id == "fire" {
            continue;
        }
        let saved = discount > 0.0
            && super::trader::random_below(&mut seed, 100) < (discount * 100.0) as u64;
        if !saved && !state.spend_ingredient(ingredient.item_id, ingredient.quantity) {
            state.market_seed = seed;
            return false;
        }
    }
    state.market_seed = seed;
    state.city.smiths[index].fire -= fire;

    if to_stock {
        if let Some(row) = state
            .city
            .stock
            .iter_mut()
            .find(|row| row.item_id == recipe.result_id)
        {
            row.quantity += quantity;
        } else {
            state.city.stock.push(StockItem {
                item_id: recipe.result_id.to_string(),
                quantity,
            });
        }
    } else if !state.add_item(recipe.result_id, quantity) {
        return false;
    }

    let wage = balance::smith_wage(item.base_price, quantity);
    state.city.smiths[index].wage_due += wage;
    let xp = balance::smith_xp_per_craft(recipe.id, item.base_price, quantity)
        * state.income_multiplier()
        * state.smith_xp_bonus as f64;
    state.add_smith_xp(xp);
    true
}

/// Пробует выковать одну вещь: идёт по очереди и берёт первый рецепт,
/// на который хватает уровня, материалов, места и огня.
fn craft_in_smith(state: &mut GameState, index: usize) -> bool {
    let recipes = state.city.smiths[index].recipes.clone();
    for recipe_id in recipes {
        let Some(recipe) = items::recipe(&recipe_id) else {
            continue;
        };
        if state.smith_level() < items::recipe_min_smith_level(recipe.id) {
            continue;
        }
        if !actions::recipe_visible(state, recipe) {
            continue;
        }
        if !smith_has_ingredients(state, recipe) || !smith_has_output_room(state, recipe) {
            continue;
        }
        let fire = actions::fire_cost(state, recipe);
        let coal_reserve = recipe
            .ingredients
            .iter()
            .filter(|ingredient| ingredient.item_id == "coal")
            .map(|ingredient| ingredient.quantity)
            .sum();
        if !refuel_smith(state, index, fire, coal_reserve) {
            continue;
        }
        if perform_smith_craft(state, index, recipe) {
            return true;
        }
    }
    false
}

/// Тикает всех кузнецов. Шаг привязан к `total_play_seconds`, поэтому
/// тайм-варп не ускоряет ковку: пропущенные шаги не догоняются.
pub fn tick_smiths(state: &mut GameState) {
    if !state.city.unlocked {
        return;
    }
    let timestamp = now(state);
    for index in 0..balance::SMITH_COUNT {
        let smith = &state.city.smiths[index];
        if !smith.built || smith.paused || smith.recipes.is_empty() {
            continue;
        }
        if smith.next_craft_at <= 0.0 {
            let interval = balance::smith_interval(smith.speed_level);
            state.city.smiths[index].next_craft_at = timestamp + interval;
            continue;
        }
        if timestamp < smith.next_craft_at {
            continue;
        }
        craft_in_smith(state, index);
        let interval = balance::smith_interval(state.city.smiths[index].speed_level);
        state.city.smiths[index].next_craft_at = timestamp + interval;
    }
}

// ---------- Приказчик ----------

/// Нанимает приказчика в лавку.
pub fn hire_clerk(state: &mut GameState) -> Result<(), ShopError> {
    if state.city.shop.is_none() {
        return Err(ShopError::NoShop);
    }
    if state.city.clerk_hired {
        return Err(ShopError::NoGold);
    }
    if state.gold + 1e-9 < balance::CLERK_PRICE {
        return Err(ShopError::NoGold);
    }
    state.gold -= balance::CLERK_PRICE;
    state.city.clerk_hired = true;
    state.city.next_clerk_at = now(state) + balance::CLERK_INTERVAL;
    Ok(())
}

/// Меняет наценку приказчика.
pub fn set_clerk_mult(state: &mut GameState, mult: f32) {
    state.city.clerk_mult = mult.clamp(balance::CLERK_MULT_MIN, balance::CLERK_MULT_MAX);
}

/// Приказчик выставляет ходовой товар со своего склада на полку.
///
/// Полка — обычная позиция лавки: если такой товар уже стоит, приказчик
/// докладывает его, иначе занимает свободное место. Берёт то, у чего больше
/// невыпитого спроса, и не кладёт товар с мёртвым спросом.
pub fn tick_clerk(state: &mut GameState) {
    if !state.city.clerk_hired {
        return;
    }
    let timestamp = now(state);
    if state.city.next_clerk_at <= 0.0 {
        state.city.next_clerk_at = timestamp + balance::CLERK_INTERVAL;
        return;
    }
    if timestamp < state.city.next_clerk_at {
        return;
    }
    state.city.next_clerk_at = timestamp + balance::CLERK_INTERVAL;

    let stocked: Vec<String> = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.slots.iter().map(|slot| slot.item_id.clone()).collect())
        .unwrap_or_default();
    // Сначала докладываем товар на уже стоящие полки.
    let refill = stock_plan(state, &stocked, stocked.len(), None, false);
    let mult = state.city.clerk_mult;
    for (item_id, quantity) in refill {
        if add_shelf(state, &item_id, quantity, mult).is_err() {
            break;
        }
        take_from_stock(state, &item_id, quantity);
    }
    // Потом заводим новые позиции на свободные места.
    let free_slots = state
        .city
        .shop
        .as_ref()
        .map(|shop| (shop.max_slots as usize).saturating_sub(shop.slots.len()))
        .unwrap_or(0);
    if free_slots > 0 {
        let fresh = stock_plan(state, &stocked, free_slots, None, true);
        for (item_id, quantity) in fresh {
            if add_shelf(state, &item_id, quantity, mult).is_err() {
                break;
            }
            take_from_stock(state, &item_id, quantity);
        }
    }
}

/// Снимает товар со склада после того, как он попал на полку.
fn take_from_stock(state: &mut GameState, item_id: &str, quantity: u32) {
    if let Some(row) = state
        .city
        .stock
        .iter_mut()
        .find(|row| row.item_id == item_id)
    {
        row.quantity = row.quantity.saturating_sub(quantity);
    }
    state.city.stock.retain(|row| row.quantity > 0);
}

/// План приказчика: сколько какого товара поставить на полки.
///
/// Позиции идут по убыванию спроса. `stocked` — что уже стоит
/// на полках, `slots_left` — сколько новых позиций ещё влезет.
pub fn stock_plan(
    state: &GameState,
    stocked: &[String],
    slots_left: usize,
    only: Option<&str>,
    new_only: bool,
) -> Vec<(String, u32)> {
    let mut rows: Vec<&StockItem> = state
        .city
        .stock
        .iter()
        .filter(|row| row.quantity > 0 && is_shop_sellable(state, &row.item_id))
        .filter(|row| only.is_none_or(|id| id == row.item_id))
        .filter(|row| !(new_only && stocked.iter().any(|id| id == &row.item_id)))
        .collect();
    rows.sort_by(|left, right| {
        let a = demand_of(state, &left.item_id).available;
        let b = demand_of(state, &right.item_id).available;
        b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut plan: Vec<(String, u32)> = Vec::new();
    let mut used = 0usize;
    for row in rows {
        let on_shelf = stocked.iter().any(|id| id == &row.item_id);
        if !on_shelf {
            if used >= slots_left {
                continue;
            }
            used += 1;
        }
        let quantity = row.quantity.min(clerk_batch(&row.item_id));
        if quantity == 0 {
            continue;
        }
        plan.push((row.item_id.clone(), quantity));
    }
    plan
}

/// Забирает товар со склада в рюкзак.
pub fn take_stock_amount(
    state: &mut GameState,
    index: usize,
    quantity: u32,
) -> Result<(), ShopError> {
    let row = state
        .city
        .stock
        .get(index)
        .cloned()
        .ok_or(ShopError::BadSlot)?;
    let quantity = quantity.min(row.quantity);
    if quantity == 0 {
        return Err(ShopError::EmptySlot);
    }
    if state.item_capacity_left(&row.item_id) < quantity {
        return Err(ShopError::NoSpace);
    }
    if quantity >= row.quantity {
        state.city.stock.remove(index);
    } else if let Some(entry) = state.city.stock.get_mut(index) {
        entry.quantity -= quantity;
    }
    state.add_item(&row.item_id, quantity);
    Ok(())
}

/// Выставляет товар со склада на полку руками.
pub fn stock_to_shelf(state: &mut GameState, index: usize, quantity: u32) -> Result<(), ShopError> {
    let row = state
        .city
        .stock
        .get(index)
        .cloned()
        .ok_or(ShopError::BadSlot)?;
    let quantity = quantity.min(row.quantity);
    if quantity == 0 {
        return Err(ShopError::EmptySlot);
    }
    let mult = state.city.clerk_mult;
    add_shelf(state, &row.item_id, quantity, mult).map_err(|error| match error {
        ShopError::SlotLimit => ShopError::ShelfFull,
        other => other,
    })?;
    if quantity >= row.quantity {
        state.city.stock.remove(index);
    } else if let Some(entry) = state.city.stock.get_mut(index) {
        entry.quantity -= quantity;
    }
    Ok(())
}

/// Средний чек лавки.
pub fn average_check(shop: &Shop) -> f64 {
    if shop.total_sales == 0 {
        0.0
    } else {
        shop.total_profit / shop.total_sales as f64
    }
}
/// Топ товаров по количеству проданных штук.
pub fn top_goods(shop: &Shop, limit: usize) -> Vec<(String, u32)> {
    let mut totals: Vec<(String, u32)> = Vec::new();
    for sale in &shop.sales {
        if let Some(entry) = totals.iter_mut().find(|(id, _)| id == &sale.item_id) {
            entry.1 += sale.quantity;
        } else {
            totals.push((sale.item_id.clone(), sale.quantity));
        }
    }
    totals.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    totals.truncate(limit);
    totals
}

/// Пора ли начинать город заново: перерождение сбрасывает город целиком,
/// поэтому квест Мартина появляется раз в цикл заново.
///
/// В первом цикле города нет: до двух перерождений сюжет ещё неоткрыт.
pub fn city_cycle_pending(state: &GameState) -> bool {
    state.cycles >= 2 && !state.city.unlocked
}

/// Добавляет сюжетный квест Мартина, если пора и его ещё нет.
pub fn ensure_intro_quest(state: &mut GameState) {
    if !city_cycle_pending(state) {
        return;
    }
    if state
        .quests
        .iter()
        .any(|quest| quest.template_id == INTRO_TEMPLATE_ID)
    {
        return;
    }
    state.quests.push(Quest {
        id: INTRO_QUEST_ID.to_string(),
        author: QuestAuthor::Citizen("martin".to_string()),
        request: Vec::new(),
        reward: QuestReward::Gold(INTRO_REWARD_GOLD),
        status: QuestStatus::Available,
        created_at: state.quest_clock,
        expires_at: 0.0,
        declined: false,
        template_id: INTRO_TEMPLATE_ID,
    });
}

/// Ответ игрока Мартину. `false` — отказ вернуть квест на доску.
pub fn martin_answer(state: &mut GameState, accepts: bool) -> bool {
    if state.city.martin != MartinStage::AskCraft {
        return false;
    }
    if accepts {
        if !state.add_item(items::COMPASS_BLUEPRINT_ID, 1) {
            return false;
        }
        state.city.martin = MartinStage::WaitCompass;
        if let Some(quest) = state
            .quests
            .iter_mut()
            .find(|quest| quest.template_id == INTRO_TEMPLATE_ID)
        {
            quest.request = vec![(items::COMPASS_ID.to_string(), 1)];
            quest.expires_at = 0.0;
        }
        return true;
    }

    state.city.martin = MartinStage::NotMet;
    if let Some(quest) = state
        .quests
        .iter_mut()
        .find(|quest| quest.template_id == INTRO_TEMPLATE_ID)
    {
        quest.status = QuestStatus::Available;
        quest.expires_at = 0.0;
        quest.request.clear();
    }
    state.remove_quest_letter(INTRO_QUEST_ID);
    true
}

/// Завершает сюжетную цепочку: город открыт, лавка создана.
pub fn unlock_city(state: &mut GameState) {
    state.city.unlocked = true;
    state.city.martin = MartinStage::Complete;
    // Всё, что цеха намолотили до открытия города, налогом не облагается.
    state.city.production_value = 0.0;
    open_shop(state);
    state.add_letter("city_unlocked", None);
}

/// Пара слуха: мёртвый и горячий товар из одной категории.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RumorPair {
    /// Товар, который слух объявил подделкой.
    pub dead: String,
    /// Товар, который слух назвал «тем самым».
    pub hot: String,
}

/// Выбирает пару товаров одной категории для слуха.
fn roll_rumor_pair(state: &mut GameState) -> Option<RumorPair> {
    let mut pools: Vec<Vec<String>> = Vec::new();
    for category in [
        items::Category::Material,
        items::Category::Gear,
        items::Category::Jewelry,
        items::Category::Goods,
        items::Category::Consumable,
    ] {
        let pool: Vec<String> = items::catalog()
            .iter()
            .filter(|item| item.category == category && is_shop_sellable(state, &item.id))
            .map(|item| item.id.clone())
            .collect();
        if pool.len() >= 2 {
            pools.push(pool);
        }
    }
    if pools.is_empty() {
        return None;
    }
    let pool =
        &pools[super::trader::random_below(&mut state.market_seed, pools.len() as u64) as usize];
    let hot_index = super::trader::random_below(&mut state.market_seed, pool.len() as u64) as usize;
    let mut dead_index =
        super::trader::random_below(&mut state.market_seed, (pool.len() - 1) as u64) as usize;
    if dead_index >= hot_index {
        dead_index += 1;
    }
    Some(RumorPair {
        dead: pool[dead_index].clone(),
        hot: pool[hot_index].clone(),
    })
}

/// 1–2 товара, которым конкурент перекроет налив: сначала с полок игрока.
fn roll_competitor_items(state: &mut GameState) -> Vec<String> {
    let mut pool: Vec<String> = state
        .city
        .shop
        .as_ref()
        .map(|shop| shop.slots.iter().map(|slot| slot.item_id.clone()).collect())
        .unwrap_or_default();
    if pool.is_empty() {
        // Полки пусты — конкурент бьёт по ходовому товару.
        pool = RUMOR_ITEMS
            .iter()
            .filter(|id| is_shop_sellable(state, id))
            .map(|id| (*id).to_string())
            .collect();
    }
    if pool.is_empty() {
        return Vec::new();
    }
    let count =
        (1 + super::trader::random_below(&mut state.market_seed, 2) as usize).min(pool.len());
    let mut picked = Vec::with_capacity(count);
    while picked.len() < count {
        let index = super::trader::random_below(&mut state.market_seed, pool.len() as u64) as usize;
        picked.push(pool.remove(index));
    }
    picked
}

/// Случайное городское событие. Вызывается по таймеру раз в 30 минут.
pub fn try_spawn_event(state: &mut GameState) {
    let timestamp = now(state);
    let chance = super::trader::random_below(&mut state.market_seed, 100);
    if chance >= balance::CITY_EVENT_CHANCE {
        return;
    }
    let pick = super::trader::random_below(&mut state.market_seed, 100) as u32;
    state.city.active_event = match pick {
        0..=24 => Some(CityEvent::Competitor {
            ends_at: timestamp + balance::CITY_EVENT_COMPETITOR_SECS,
            items: roll_competitor_items(state),
        }),
        25..=49 => {
            let pair = roll_rumor_pair(state).unwrap_or_else(|| RumorPair {
                dead: RUMOR_ITEMS[1].to_string(),
                hot: RUMOR_ITEMS[0].to_string(),
            });
            Some(CityEvent::Rumor {
                item_id: pair.hot,
                dead_item: pair.dead,
                ends_at: timestamp + balance::TAVERN_RUMOR_DURATION,
            })
        }
        50..=69 => {
            state.city.fire_pending = true;
            None
        }
        70..=79 => Some(CityEvent::Epidemic {
            ends_at: timestamp + balance::CITY_EVENT_EPIDEMIC_SECS,
        }),
        80..=89 => Some(CityEvent::Festival {
            ends_at: timestamp + balance::CITY_EVENT_FESTIVAL_SECS,
        }),
        _ => Some(CityEvent::Fair {
            ends_at: timestamp + balance::CITY_EVENT_FAIR_SECS,
        }),
    };
}

/// Городской тик: сюжет, налоги, события, покупатели.
pub fn tick_city(state: &mut GameState) {
    ensure_intro_quest(state);
    if !state.city.unlocked {
        return;
    }
    let timestamp = now(state);

    if state.city.next_tax_at <= 0.0 {
        state.city.next_tax_at = timestamp + balance::CITY_TAX_INTERVAL;
    }
    if state.city.next_event_at <= 0.0 {
        state.city.next_event_at = timestamp + balance::CITY_TAX_INTERVAL;
    }
    if state.city.next_salary_at <= 0.0 {
        state.city.next_salary_at = timestamp + balance::CITY_TAX_INTERVAL;
    }

    // Истёкшее событие убираем.
    if let Some(event) = state.city.active_event.as_ref() {
        if !event.is_active(timestamp) {
            state.city.active_event = None;
        }
    }

    // Спрос города: налив и дрейф цели.
    let demand_dt = (timestamp - state.city.demand_at).clamp(0.0, balance::DEMAND_MAX_STEP);
    state.city.demand_at = timestamp;
    advance_demand(state, demand_dt);

    // Письмо за 5 минут до налога.
    if state.city.shop.is_some()
        && !state.city.tax_warning_sent
        && state.city.next_tax_at > timestamp
        && state.city.next_tax_at - timestamp <= balance::CITY_TAX_WARNING_SECS
        && !epidemic_active(state)
    {
        state.add_letter("tax_soon", None);
        state.city.tax_warning_sent = true;
    }

    if timestamp >= state.city.next_tax_at {
        pay_tax(state);
        state.city.next_tax_at = timestamp + balance::CITY_TAX_INTERVAL;
    }

    // Зарплаты идут тем же ритмом: раз в 30 минут.
    if timestamp >= state.city.next_salary_at {
        pay_salaries(state);
        state.city.next_salary_at = timestamp + balance::CITY_TAX_INTERVAL;
    }

    // Стройка Дома странников: этап длится своё время и засчитывается сам.
    tick_traveler_house(state);

    if timestamp >= state.city.next_event_at {
        try_spawn_event(state);
        let jitter = super::trader::random_below(&mut state.market_seed, 1_201) as f64 - 600.0;
        state.city.next_event_at = timestamp + balance::CITY_TAX_INTERVAL + jitter;
    }

    // Покупатели.
    let buyer_due = state
        .city
        .shop
        .as_ref()
        .map(|shop| timestamp >= shop.next_buyer_at)
        .unwrap_or(false);
    if buyer_due {
        let interval = buyer_interval(state);
        let empty = state
            .city
            .shop
            .as_ref()
            .map(|shop| shop.slots.iter().all(|slot| slot.quantity == 0))
            .unwrap_or(false);
        if let Some(delay) = interval {
            if empty {
                if let Some(shop) = state.city.shop.as_mut() {
                    shop.rating = (shop.rating - balance::SHOP_EMPTY_RATING_PENALTY)
                        .max(balance::SHOP_STARTING_RATING);
                }
            } else {
                try_buyer(state);
            }
            if let Some(shop) = state.city.shop.as_mut() {
                shop.next_buyer_at = timestamp + delay;
            }
        } else if let Some(shop) = state.city.shop.as_mut() {
            shop.next_buyer_at = timestamp + balance::SHOP_BUYER_BASE_INTERVAL;
        }
    }

    // Ратуша открывается на 60 продажах при рейтинге выше 3.
    if !state.city.town_hall_unlocked {
        if let Some(shop) = state.city.shop.as_ref() {
            if shop.total_sales >= balance::TOWN_HALL_SALES_REQUIRED
                && shop.rating > balance::TOWN_HALL_MIN_RATING
            {
                state.city.town_hall_unlocked = true;
            }
        }
    }

    // Путешественники возвращаются, кузнецы куют, приказчик выставляет товар.
    tick_travelers(state);
    tick_smiths(state);
    tick_clerk(state);
}

#[cfg(test)]
mod tests {

    /// Кладёт товар на склад напрямую, без сумки.
    fn stock_in_test(state: &mut GameState, item_id: &str, quantity: u32) -> Result<(), ShopError> {
        if let Some(row) = state
            .city
            .stock
            .iter_mut()
            .find(|row| row.item_id == item_id)
        {
            row.quantity += quantity;
            return Ok(());
        }
        if state.city.stock.len() >= clerk_stock_cells(state) as usize {
            return Err(ShopError::SlotLimit);
        }
        state.city.stock.push(StockItem {
            item_id: item_id.to_string(),
            quantity,
        });
        Ok(())
    }
    use super::*;
    use crate::game::trader::MarketStatus;
    use crate::game::{actions, quests};

    fn city_with_shop() -> GameState {
        let mut state = GameState::default();
        state.city.unlocked = true;
        state.city.martin = MartinStage::Complete;
        state.city.shop = Some(Shop::default());
        state
    }

    fn sale(total: f64, timestamp: f64) -> SaleRecord {
        SaleRecord {
            item_id: "robe".to_string(),
            quantity: 1,
            unit_price: total,
            total,
            timestamp,
            vip: false,
        }
    }

    #[test]
    fn open_shop_creates_shop() {
        let mut state = GameState::default();
        assert!(state.city.shop.is_none());
        open_shop(&mut state);
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.rating, balance::SHOP_STARTING_RATING);
        assert_eq!(shop.max_slots, balance::SHOP_STARTING_SLOTS);
    }

    #[test]
    fn add_slot_removes_from_inventory() {
        let mut state = city_with_shop();
        state.add_item("robe", 2);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        assert_eq!(state.amount_of("robe"), 1);
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.slots.len(), 1);
        assert_eq!(shop.slots[0].quantity, 1);
    }

    #[test]
    fn add_slot_fails_without_item() {
        let mut state = city_with_shop();
        assert_eq!(
            add_slot(&mut state, "robe", 1, 1.0),
            Err(ShopError::MissingItem)
        );
    }

    #[test]
    fn add_slot_fails_for_unstudied_map() {
        let mut state = city_with_shop();
        state.add_item("map_forest", 1);
        assert_eq!(
            add_slot(&mut state, "map_forest", 1, 1.0),
            Err(ShopError::Unsellable)
        );
    }

    #[test]
    fn add_slot_succeeds_for_studied_map() {
        let mut state = city_with_shop();
        state.unlock_location("deep_forest");
        state.add_item("map_forest", 1);
        add_slot(&mut state, "map_forest", 1, 1.0).unwrap();
        assert_eq!(state.city.shop.as_ref().unwrap().slots.len(), 1);
    }

    #[test]
    fn remove_slot_returns_to_inventory() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        remove_slot(&mut state, 0).unwrap();
        assert_eq!(state.amount_of("robe"), 1);
        assert!(state.city.shop.as_ref().unwrap().slots.is_empty());
    }

    #[test]
    fn set_price_clamps() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        set_price(&mut state, 0, 0.1).unwrap();
        assert!((state.city.shop.as_ref().unwrap().slots[0].price_mult - 0.5).abs() < 1e-6);
        set_price(&mut state, 0, 9.0).unwrap();
        assert!((state.city.shop.as_ref().unwrap().slots[0].price_mult - 2.0).abs() < 1e-6);
    }

    #[test]
    fn buyer_pays_cash() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        let record = sell_slot(&mut state, 0, false).unwrap();
        {
            let shop = state.city.shop.as_ref().unwrap();
            assert!(shop.cash > 0.0);
            assert!((shop.cash - record.total).abs() < 1e-6);
            assert_eq!(shop.total_sales, 1);
            // Рейтинг растёт не за каждую продажу, а раз в несколько продаж.
            assert!((shop.rating - balance::SHOP_STARTING_RATING).abs() < 1e-6);
            // Распроданная полка снимается из лавки.
            assert!(shop.slots.is_empty());
        }

        // Пятая продажа даёт первую прибавку к рейтингу.
        for _ in 0..4 {
            state.add_item("robe", 1);
            add_slot(&mut state, "robe", 1, 1.0).unwrap();
            sell_slot(&mut state, 0, false).unwrap();
        }
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.total_sales, 5);
        assert!(shop.rating > balance::SHOP_STARTING_RATING);
    }

    #[test]
    fn rating_grows_every_five_sales() {
        let mut state = city_with_shop();
        for sale in 1..=10 {
            state.add_item("robe", 1);
            add_slot(&mut state, "robe", 1, 1.0).unwrap();
            sell_slot(&mut state, 0, false).unwrap();
            let gained =
                (sale / balance::SHOP_RATING_SALES_PER_GAIN) as f32 * balance::SHOP_RATING_PER_GAIN;
            let rating = state.city.shop.as_ref().unwrap().rating;
            assert!(
                (rating - (balance::SHOP_STARTING_RATING + gained)).abs() < 1e-6,
                "продажа {sale}: рейтинг {rating}"
            );
        }
        // 1 → 5 при +0.25 за 5 продаж занимает 80 продаж.
        let state = state.city.shop.as_ref().unwrap();
        let needs = ((balance::SHOP_MAX_RATING - balance::SHOP_STARTING_RATING)
            / balance::SHOP_RATING_PER_GAIN) as u32
            * balance::SHOP_RATING_SALES_PER_GAIN;
        assert!((50..=100).contains(&needs), "{needs}");
        assert_eq!(state.total_sales, 10);
    }

    #[test]
    fn buyers_come_more_often() {
        let state = city_with_shop();
        let interval = buyer_interval(&state).unwrap();
        assert!(
            (balance::SHOP_BUYER_BASE_INTERVAL - 25.0).abs() < 1e-6,
            "базовый интервал должен быть 20–30 секунд"
        );
        assert!(interval < 30.0, "интервал {interval}");
    }

    #[test]
    fn buyer_can_take_part_of_the_stack() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        let record = sell_slot_amount(&mut state, 0, false, 3).unwrap();
        assert_eq!(record.quantity, 3);
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.slots[0].quantity, 7);
        assert!((record.total - record.unit_price * 3.0).abs() < 1e-6);

        // Больше остатка не продаёт.
        let record = sell_slot_amount(&mut state, 0, false, 99).unwrap();
        assert_eq!(record.quantity, 7);
        assert!(state.city.shop.as_ref().unwrap().slots.is_empty());
    }

    #[test]
    fn buyer_takes_only_what_fits_the_cash_box() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        {
            let shop = state.city.shop.as_mut().unwrap();
            // В кассе остаётся место ровно на одну вещь по 62 золота.
            shop.cash = shop.cash_cap - 100.0;
        }

        let record = sell_slot_amount(&mut state, 0, false, 10).unwrap();
        assert_eq!(record.quantity, 1);

        let shop = state.city.shop.as_ref().unwrap();
        assert!(shop.cash <= shop.cash_cap);
        assert!((shop.cash - (shop.cash_cap - 100.0 + record.total)).abs() < 1e-6);
        // Остаток товара остался на полке, золото не сгорело.
        assert_eq!(shop.slots[0].quantity, 9);
    }

    #[test]
    fn full_cash_box_blocks_the_sale() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        {
            let shop = state.city.shop.as_mut().unwrap();
            // Места в кассе меньше, чем стоит одна вещь.
            shop.cash = shop.cash_cap - 10.0;
        }

        assert_eq!(
            sell_slot_amount(&mut state, 0, false, 1),
            Err(ShopError::CashFull)
        );
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.slots[0].quantity, 1);
        assert!((shop.cash - (shop.cash_cap - 10.0)).abs() < 1e-6);
    }

    #[test]
    fn full_cash_box_sends_the_buyer_away() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = shop.cash_cap;
        }

        assert!(try_buyer(&mut state).is_none());
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.total_sales, 0);
        assert_eq!(shop.slots[0].quantity, 1);
    }

    #[test]
    fn buyer_skips_expensive_goods() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.6).unwrap();
        // Наценка 1.6 — шанс ухода около 40%.
        assert_eq!(buyer_decision(&state, 0, 0.0, 0.5), BuyerDecision::Leave);
        assert!(matches!(
            buyer_decision(&state, 0, 0.9, 0.5),
            BuyerDecision::Buy { .. }
        ));
        // На ярмарке штраф за цену отключён.
        state.city.active_event = Some(CityEvent::Fair { ends_at: 10_000.0 });
        assert!(matches!(
            buyer_decision(&state, 0, 0.0, 0.99),
            BuyerDecision::Buy { .. }
        ));
    }

    #[test]
    fn price_curve_punishes_greed() {
        // До 1.3 берут все, к 2.0 — никто. Между ними шанс растёт плавно.
        assert!((price_leave_chance(1.0) - 0.0).abs() < 1e-6);
        assert!((price_leave_chance(1.3) - 0.0).abs() < 1e-6);
        let gentle = price_leave_chance(1.4);
        let mid = price_leave_chance(1.5);
        let steep = price_leave_chance(1.8);
        assert!(gentle > 0.0 && gentle < mid, "{gentle} {mid}");
        assert!(mid < steep && steep < 1.0, "{mid} {steep}");
        // Кривая гладкая: шаг 0.01 не даёт скачков вроде 25% → 70%.
        let mut previous = 0.0;
        let mut bump: f64 = 0.0;
        for step in 0..=100 {
            let value = price_leave_chance(1.0 + step as f32 / 100.0);
            bump = bump.max(value - previous);
            previous = value;
        }
        assert!(bump < 0.05, "ступенька {bump}");
        assert!((price_leave_chance(2.0) - 1.0).abs() < 1e-6);
        assert!((price_leave_chance(2.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn two_point_zero_price_sends_everyone_away() {
        let mut state = city_with_shop();
        state.add_item("robe", 5);
        add_slot(&mut state, "robe", 5, 2.0).unwrap();
        assert_eq!(buyer_decision(&state, 0, 0.0, 0.5), BuyerDecision::Leave);
        assert_eq!(buyer_decision(&state, 0, 0.99, 0.5), BuyerDecision::Leave);
        // Ярмарка — единственное исключение.
        state.city.active_event = Some(CityEvent::Fair { ends_at: 10_000.0 });
        assert!(matches!(
            buyer_decision(&state, 0, 0.0, 0.5),
            BuyerDecision::Buy { .. }
        ));
    }

    #[test]
    fn demand_capacity_follows_the_price() {
        // Дешёвой свечи городу нужно больше, чем дорогой кольчуги.
        assert!(demand_capacity("candle") > demand_capacity("chainmail"));
        // И город берёт цехами: ёмкость больше десятка штук даже у дорогого.
        assert!(demand_capacity("chainmail") >= 10.0);
    }

    #[test]
    fn sales_drink_demand_and_dead_shelf_sends_buyers_away() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        let before = demand_of(&state, "robe").available;
        sell_slot_amount(&mut state, 0, false, 5).unwrap();
        let entry = state.city.demand.get("robe").cloned().unwrap();
        let diff = entry.available - (before - 5.0);
        assert!(
            diff.abs() <= (before * 1e-6).max(1e-3),
            "налито {} было {}",
            entry.available,
            before
        );

        // Выпиваем спрос до нуля: обычный покупатель уходит.
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = 0.0;
        }
        assert_eq!(buyer_decision(&state, 0, 0.0, 0.99), BuyerDecision::Skip);

        // Пару двигает только тик рынка: продажа её не трогает, поэтому
        // шаг и продажа не считают одно и то же дважды.
        let pair_before = demand_of(&state, "robe");
        consume_demand(&mut state, "robe", 20);
        let pair_after = demand_of(&state, "robe");
        assert_eq!(pair_after.demand, pair_before.demand);
        assert_eq!(pair_after.supply, pair_before.supply);
        assert!(pair_after.available.abs() < 1e-6);
    }

    #[test]
    fn vip_takes_goods_nobody_else_wants() {
        // Дорогая вещь в лавке: обычный покупатель её даже не рассматривает,
        // берёт только VIP.
        let mut state = city_with_shop();
        state.add_item("chainmail", 1);
        add_slot(&mut state, "chainmail", 1, 1.0).unwrap();
        let price = shop_price(&state, &state.city.shop.as_ref().unwrap().slots[0]);
        assert!(
            price > balance::SHOP_VIP_ONLY_PRICE,
            "кольчуга должна быть дороже порога VIP: {price}"
        );
        assert!(demand_of(&state, "chainmail").available > 0.0);
        assert_eq!(buyer_decision(&state, 0, 0.0, 0.99), BuyerDecision::Skip);
        assert!(matches!(
            buyer_decision(&state, 0, 0.0, 0.0),
            BuyerDecision::Buy { vip: true }
        ));
        // Наценка выше `SHOP_PRICE_LEAVE_FULL` отпугивает и VIP.
        set_price(&mut state, 0, balance::SHOP_PRICE_LEAVE_FULL).unwrap();
        assert_eq!(buyer_decision(&state, 0, 0.0, 0.0), BuyerDecision::Leave);
    }

    #[test]
    fn half_drained_shelf_still_sells() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        let capacity = state.city.demand.get("robe").unwrap().capacity;
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = capacity * 0.2;
        }
        // Наценки нет — обычный покупатель берёт, даже если спрос поредел.
        assert!(matches!(
            buyer_decision(&state, 0, 0.0, 0.99),
            BuyerDecision::Buy { .. }
        ));
    }

    #[test]
    fn demand_refills_over_time() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = 0.0;
        }
        state.total_play_seconds = 600.0;
        catch_up_offline(&mut state, 600.0);
        let ratio = demand_ratio(&state, "robe");
        assert!(
            (0.25..=0.40).contains(&ratio),
            "за 10 минут должна налиться треть, а не {ratio}"
        );
    }

    #[test]
    fn offline_catch_up_refills_demand() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = 0.0;
        }
        state.total_play_seconds = 3_600.0;
        catch_up_offline(&mut state, 3_600.0);
        assert!(demand_ratio(&state, "robe") > 0.9);
    }

    #[test]
    fn buyer_takes_no_more_than_the_demand() {
        // Обычный покупатель не берёт больше налитого спроса и чем лежит,
        // и не больше трёх штук за визит: город берёт партиями.
        assert_eq!(buyer_amount(50, 2.0, false), 2);
        assert_eq!(buyer_amount(50, 0.0, false), 1);
        assert_eq!(buyer_amount(7, 900.0, false), 3);
        // VIP берёт до пяти штук — включая мёртвую полку.
        assert_eq!(buyer_amount(50, 0.0, true), 5);
        assert_eq!(buyer_amount(1, 900.0, true), 1);
    }

    #[test]
    fn expensive_goods_are_vip_only() {
        let mut state = city_with_shop();
        state.add_item("chainmail", 5);
        add_slot(&mut state, "chainmail", 5, 2.0).unwrap();
        let index = 0;
        let price = {
            let shop = state.city.shop.as_ref().unwrap();
            // Цена без всех бонусов: наценка и так упирается в потолок полки.
            shop_price_for(&state, &shop.slots[index].item_id, 2.0)
        };
        assert!(price > balance::SHOP_VIP_ONLY_PRICE, "{price}");
        // Не-VIP проходит мимо дорогого товара даже при налитом спросе.
        assert_eq!(
            buyer_decision(&state, index, 0.0, 0.99),
            BuyerDecision::Skip
        );
        state.city.demand.insert(
            "chainmail".to_string(),
            DemandEntry {
                available: 100.0,
                capacity: 500.0,
                ..DemandEntry::default()
            },
        );
        assert_eq!(
            buyer_decision(&state, index, 0.0, 0.0),
            BuyerDecision::Leave
        );
    }

    #[test]
    fn normal_buyer_never_takes_more_than_the_demand() {
        let mut state = city_with_shop();
        state.add_item("robe", 50);
        add_slot(&mut state, "robe", 50, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = 2.0;
        }
        let mut bought = 0;
        for _ in 0..200 {
            if let Some(record) = try_buyer(&mut state) {
                if !record.vip {
                    assert!(record.quantity <= 2, "продано {}", record.quantity);
                    bought += record.quantity;
                }
            }
        }
        assert!(bought <= 4, "спрос выпит дважды: {bought}");
    }

    #[test]
    fn old_save_without_demand_gets_a_full_town() {
        let json = r#"{"city":{"unlocked":true,"shop":{"slots":[{"item_id":"robe","quantity":3,"price_mult":1.0}]}}}"#;
        let state: GameState = serde_json::from_str(json).unwrap();
        assert!(state.city.demand.is_empty());
        assert!((demand_ratio(&state, "robe") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn vip_pays_more() {
        let mut plain = city_with_shop();
        plain.add_item("robe", 1);
        add_slot(&mut plain, "robe", 1, 1.0).unwrap();
        let normal = sell_slot(&mut plain, 0, false).unwrap();

        let mut vip = city_with_shop();
        vip.add_item("robe", 1);
        add_slot(&mut vip, "robe", 1, 1.0).unwrap();
        let rich = sell_slot(&mut vip, 0, true).unwrap();

        assert!(rich.unit_price > normal.unit_price);
        assert!(rich.total > normal.total);
    }

    #[test]
    fn tax_takes_15_percent() {
        let mut state = city_with_shop();
        state.total_play_seconds = 100.0;
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = 1_000.0;
            shop.sales.push(sale(1_000.0, 50.0));
        }
        assert_eq!(pay_tax(&mut state), TaxOutcome::Paid);
        assert!((state.city.shop.as_ref().unwrap().cash - 850.0).abs() < 1e-6);
        assert_eq!(state.city.taxes_paid, 1);
    }

    #[test]
    fn tax_miss_lowers_rating() {
        let mut state = city_with_shop();
        state.total_play_seconds = 100.0;
        state.gold = 0.0;
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = 0.0;
            shop.sales.push(sale(1_000.0, 50.0));
        }
        // Первый срок: налог выставлен счётом, рейтинг ещё не падает.
        assert_eq!(pay_tax(&mut state), TaxOutcome::Short);
        assert!((state.city.tax_due - 150.0).abs() < 1e-6);
        assert!(state.letters.iter().any(|letter| letter.id == "tax_due"));
        assert_eq!(state.city.taxes_missed, 0);

        // Счёт не оплатили к следующему сроку — это пропуск налога.
        state.total_play_seconds = 100.0 + balance::CITY_TAX_INTERVAL;
        assert_eq!(pay_tax(&mut state), TaxOutcome::Missed);
        let shop = state.city.shop.as_ref().unwrap();
        assert!((shop.rating - 0.0).abs() < 1e-6);
        assert_eq!(state.city.taxes_missed, 1);
    }

    #[test]
    fn unpaid_tax_can_be_paid_from_the_bill() {
        let mut state = city_with_shop();
        state.total_play_seconds = 100.0;
        state.gold = 0.0;
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = 0.0;
            shop.sales.push(sale(1_000.0, 50.0));
        }
        assert_eq!(pay_tax(&mut state), TaxOutcome::Short);
        assert!((state.city.tax_due - 150.0).abs() < 1e-6);

        state.gold = 200.0;
        assert_eq!(pay_tax_due(&mut state), TaxOutcome::Paid);
        assert!((state.city.tax_due).abs() < 1e-6);
        assert!((state.gold - 50.0).abs() < 1e-6);
        assert_eq!(state.city.taxes_paid, 1);
        assert!(!state.letters.iter().any(|letter| letter.id == "tax_due"));
        assert!(state.letters.iter().any(|letter| letter.id == "tax_paid"));
    }

    #[test]
    fn paying_part_of_the_tax_keeps_the_rest_due() {
        let mut state = city_with_shop();
        state.total_play_seconds = 100.0;
        state.gold = 50.0;
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = 0.0;
            shop.sales.push(sale(1_000.0, 50.0));
        }
        assert_eq!(pay_tax(&mut state), TaxOutcome::Short);
        assert!((state.city.tax_due - 100.0).abs() < 1e-6);
        assert_eq!(state.city.taxes_paid, 0);

        state.gold = 30.0;
        assert_eq!(pay_tax_due(&mut state), TaxOutcome::Short);
        assert!((state.city.tax_due - 70.0).abs() < 1e-6);
    }

    #[test]
    fn second_miss_closes_shop() {
        let mut state = city_with_shop();
        state.total_play_seconds = 100.0;
        state.gold = 0.0;
        state.city.taxes_missed = 1;
        state.city.tax_due = 35.0;
        {
            let shop = state.city.shop.as_mut().unwrap();
            shop.cash = 0.0;
            shop.sales.push(sale(1_000.0, 50.0));
        }
        assert_eq!(pay_tax(&mut state), TaxOutcome::Closed);
        assert!(state.city.shop.is_none());
        assert!((state.city.tax_due).abs() < 1e-6);
    }

    #[test]
    fn bank_cell_raises_gold_cap() {
        let mut state = GameState::default();
        let before = state.gold_cap();
        state.gold = 10_000.0;
        buy_bank_cell(&mut state).unwrap();
        assert_eq!(state.city.bank_cells, 1);
        assert!((state.gold_cap() - (before + balance::BANK_CELL_BONUS)).abs() < 1e-6);
    }

    #[test]
    fn bank_cell_price_grows() {
        assert!(bank_cell_price(0) < bank_cell_price(1));
        assert!(bank_cell_price(10) > bank_cell_price(0));
    }

    #[test]
    fn bank_max_cells() {
        let mut state = GameState::default();
        state.city.bank_cells = balance::BANK_CELLS_MAX;
        state.gold = 1_000_000.0;
        assert_eq!(buy_bank_cell(&mut state), Err(BankError::MaxCells));
    }

    #[test]
    fn compass_recipe_needs_fire() {
        let recipe = items::recipe(items::COMPASS_RECIPE).unwrap();
        assert_eq!(recipe.fire_cost, 100);
        assert_eq!(recipe.result_id, items::COMPASS_ID);
        assert!(recipe
            .ingredients
            .iter()
            .any(|ingredient| ingredient.item_id == "fire" && ingredient.quantity == 100));
        assert!(recipe
            .ingredients
            .iter()
            .any(|ingredient| ingredient.item_id == "gold_ingot"));
    }

    #[test]
    fn compass_blueprint_study_unlocks_recipe() {
        let mut state = GameState::default();
        state.add_item(items::COMPASS_BLUEPRINT_ID, 1);
        let recipe = items::recipe(items::COMPASS_RECIPE).unwrap();
        assert!(!actions::recipe_visible(&state, recipe));
        actions::study_blueprint(&mut state, items::COMPASS_BLUEPRINT_ID).unwrap();
        assert!(state.compass_blueprint_studied);
        assert_eq!(state.amount_of(items::COMPASS_BLUEPRINT_ID), 0);
        assert!(actions::recipe_visible(&state, recipe));
    }

    #[test]
    fn martin_chain_unlocks_city() {
        let mut state = GameState::default();
        state.cycles = 2;
        ensure_intro_quest(&mut state);
        assert!(state
            .quests
            .iter()
            .any(|quest| quest.template_id == INTRO_TEMPLATE_ID));
        quests::take_quest(&mut state, INTRO_QUEST_ID).unwrap();
        assert_eq!(state.city.martin, MartinStage::AskCraft);
        assert!(martin_answer(&mut state, true));
        assert_eq!(state.city.martin, MartinStage::WaitCompass);
        assert_eq!(state.amount_of(items::COMPASS_BLUEPRINT_ID), 1);
        state.add_item(items::COMPASS_ID, 1);
        quests::complete_quest(&mut state, INTRO_QUEST_ID).unwrap();
        assert!(state.city.unlocked);
        assert_eq!(state.city.martin, MartinStage::Complete);
        assert!(state.city.shop.is_some());
        assert!(state
            .letters
            .iter()
            .any(|letter| letter.id == "city_unlocked"));
    }

    #[test]
    fn intro_quest_spawns_at_cycle_2() {
        let mut state = GameState::default();
        state.cycles = 1;
        tick_city(&mut state);
        assert!(!state
            .quests
            .iter()
            .any(|quest| quest.template_id == INTRO_TEMPLATE_ID));

        state.cycles = 2;
        tick_city(&mut state);
        assert!(state
            .quests
            .iter()
            .any(|quest| quest.template_id == INTRO_TEMPLATE_ID));
    }

    #[test]
    fn withdraw_cash_respects_cap() {
        let mut state = city_with_shop();
        state.gold = state.gold_cap() - 100.0;
        state.city.shop.as_mut().unwrap().cash = 1_000.0;
        let taken = withdraw_cash(&mut state);
        assert!((taken - 100.0).abs() < 1e-6);
        assert!((state.gold - state.gold_cap()).abs() < 1e-6);
        assert!((state.city.shop.as_ref().unwrap().cash - 900.0).abs() < 1e-6);
    }

    #[test]
    fn rumor_lasts_five_minutes() {
        let mut state = city_with_shop();
        state.gold = 1_000.0;
        let pair = buy_rumor(&mut state).expect("слух");
        assert!(!pair.hot.is_empty());
        assert!(!pair.dead.is_empty());
        assert_ne!(pair.hot, pair.dead);
        match state.city.active_event {
            Some(CityEvent::Rumor {
                item_id,
                dead_item,
                ends_at,
            }) => {
                assert_eq!(item_id, pair.hot);
                assert_eq!(dead_item, pair.dead);
                assert!((ends_at - 5.0 * 60.0).abs() < 1e-6);
            }
            _ => panic!("слух не появился"),
        }
    }

    #[test]
    fn rumor_pair_is_from_one_category() {
        let mut state = city_with_shop();
        for _ in 0..20 {
            let pair = roll_rumor_pair(&mut state).expect("пара");
            let hot = items::item(&pair.hot).expect("горячий товар");
            let dead = items::item(&pair.dead).expect("мёртвый товар");
            assert_eq!(hot.category, dead.category, "{} и {}", pair.hot, pair.dead);
        }
    }

    #[test]
    fn dead_item_loses_demand_and_hot_item_keeps_it() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        state.add_item("jacket", 10);
        add_slot(&mut state, "jacket", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        sell_slot_amount(&mut state, 1, false, 1).unwrap();
        state.total_play_seconds = 600.0;
        state.city.active_event = Some(CityEvent::Rumor {
            item_id: "jacket".to_string(),
            dead_item: "robe".to_string(),
            ends_at: 100_000.0,
        });
        advance_demand(&mut state, 60.0);
        assert!((demand_ratio(&state, "robe")).abs() < 1e-6);
        assert!(demand_ratio(&state, "jacket") > 0.5);
    }

    #[test]
    fn hot_rumor_fills_demand_faster() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        state.add_item("jacket", 10);
        add_slot(&mut state, "jacket", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        sell_slot_amount(&mut state, 1, false, 1).unwrap();
        for id in ["robe", "jacket"] {
            if let Some(entry) = state.city.demand.get_mut(id) {
                entry.available = 0.0;
            }
        }
        state.total_play_seconds = 300.0;
        state.city.active_event = Some(CityEvent::Rumor {
            item_id: "jacket".to_string(),
            dead_item: "candle".to_string(),
            ends_at: 100_000.0,
        });
        advance_demand(&mut state, 300.0);
        let plain = demand_ratio(&state, "robe");
        let hot = demand_ratio(&state, "jacket");
        assert!(hot > plain * 1.5, "горячий {hot}, обычный {plain}");
    }

    #[test]
    fn competitor_blocks_demand_growth() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        sell_slot_amount(&mut state, 0, false, 1).unwrap();
        if let Some(entry) = state.city.demand.get_mut("robe") {
            entry.available = 0.0;
        }
        state.total_play_seconds = 600.0;
        state.city.active_event = Some(CityEvent::Competitor {
            ends_at: 100_000.0,
            items: vec!["robe".to_string()],
        });
        advance_demand(&mut state, 600.0);
        assert!((demand_ratio(&state, "robe")).abs() < 1e-6);

        // Без конкурента спрос наливается.
        state.city.active_event = None;
        state.total_play_seconds = 1_200.0;
        advance_demand(&mut state, 600.0);
        assert!(demand_ratio(&state, "robe") > 0.0);
    }

    #[test]
    fn competitor_hits_the_shelves() {
        let mut state = city_with_shop();
        state.add_item("robe", 1);
        add_slot(&mut state, "robe", 1, 1.0).unwrap();
        state.add_item("candle", 1);
        add_slot(&mut state, "candle", 1, 1.0).unwrap();
        for _ in 0..10 {
            let items = roll_competitor_items(&mut state);
            assert!(!items.is_empty());
            assert!(items.iter().all(|id| id == "robe" || id == "candle"));
            assert!(items.len() <= 2);
        }
    }

    #[test]
    fn competitor_falls_back_to_trade_goods_when_shelves_are_empty() {
        let mut state = city_with_shop();
        let items = roll_competitor_items(&mut state);
        assert!(!items.is_empty());
        assert!(items.iter().all(|id| is_shop_sellable(&state, id)));
    }

    #[test]
    fn town_market_lives_on_its_own() {
        // Город сам, без единой продажи, уводит товары из баланса: тренд
        // копится и держит дефицит или избыток часами.
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        state.city.demand.insert(
            "robe".to_string(),
            DemandEntry {
                available: demand_capacity("robe"),
                capacity: demand_capacity("robe"),
                ..DemandEntry::default()
            },
        );
        let mut statuses: Vec<MarketStatus> = Vec::new();
        let mut trends = Vec::new();
        // Час города: по 20 секунд на шаг.
        for step in 1..=180u32 {
            state.total_play_seconds = step as f64 * balance::DEMAND_STEP_SECS;
            advance_demand(&mut state, balance::DEMAND_STEP_SECS);
            let entry = demand_of(&state, "robe");
            let state_status = demand_status(&state, "robe");
            if !statuses.contains(&state_status) {
                statuses.push(state_status);
            }
            trends.push(entry.trend);
        }
        assert!(statuses.len() >= 2, "город застыл в одном статусе");
        // Тренд не залипает на нуле и не бросается каждый шаг заново.
        assert!(trends.iter().any(|value| value.abs() >= 8));
        // И спрос с предложением живут: пара не держится ровно 50/50.
        let entry = demand_of(&state, "robe");
        assert!(entry.demand != 50 || entry.supply != 50 || entry.trend != 0);
    }

    #[test]
    fn huge_demand_from_an_old_save_is_clamped_to_capacity() {
        // Регрессия: старое сохранение держало налитое от прежней модели,
        // и рынок показывал «774 793 388 419 / 500».
        let mut state = city_with_shop();
        state.city.demand.insert(
            "robe".to_string(),
            DemandEntry {
                available: 774_793_388_419.0,
                capacity: 500.0,
                ..DemandEntry::default()
            },
        );
        migrate_demand(&mut state);
        let entry = demand_of(&state, "robe");
        assert!(entry.available <= entry.capacity);
        assert!((entry.available - entry.capacity).abs() < 1e-3);
    }

    #[test]
    fn town_market_moves_even_with_an_empty_shop() {
        // Регрессия: раньше город шагал только по полкам лавки, и пустая
        // лавка означала вечное «50 / 50» на рынке. Теперь торговля идёт
        // по всему каталогу, даже если игрок ничего не выставил.
        let mut state = city_with_shop();
        assert!(state.city.shop.is_some());
        state.city.shop.as_mut().unwrap().slots.clear();
        let mut statuses: Vec<MarketStatus> = Vec::new();
        for step in 1..=180u32 {
            state.total_play_seconds = step as f64 * balance::DEMAND_STEP_SECS;
            advance_demand(&mut state, balance::DEMAND_STEP_SECS);
            let status = demand_status(&state, "robe");
            if !statuses.contains(&status) {
                statuses.push(status);
            }
        }
        // За час город увёл хоть какой-то товар из равновесия.
        assert!(
            statuses.len() >= 2,
            "пустая лавка заморозила рынок: {statuses:?}"
        );
        // И записи живут в самом городе, а не только на полках.
        assert!(!state.city.demand.is_empty());
    }

    #[test]
    fn town_limits_are_the_trader_tier_in_city_scale() {
        // Одна экономика: город берёт ровно ту же норму, что торговец,
        // только в `DEMAND_CITY_SCALE` раз больше.
        for item_id in ["candle", "robe", "iron_ingot", "chainmail", "gold_ring"] {
            let trader = super::super::trader::tier_of(item_id).values().1 as f64;
            let city_units = demand_capacity(item_id);
            assert!(
                (city_units - trader * balance::DEMAND_CITY_SCALE).abs() < 1e-6,
                "{item_id}: город {city_units}, торговец {trader}"
            );
        }
        // Лимиты остаются читаемыми числами, а не миллиардами.
        assert!(demand_capacity("candle") <= 2_000.0);
        assert!(demand_capacity("chainmail") >= 30.0);
    }

    #[test]
    fn salary_bill_stays_in_the_bell_after_reading() {
        // Регрессия: счёт по зарплатам раньше исчезал после прочтения.
        // Теперь он живёт в колокольчике постоянно и обновляется.
        let mut state = city_with_shop();
        state.city.hire_charter = true;
        state.city.hired_sawmill = 2;
        state.gold = 10_000.0;

        assert_eq!(pay_salaries(&mut state), SalaryOutcome::Paid);
        assert!(state.letters.iter().any(|letter| letter.id == "salary_bill"));
        // Прочитали — письмо остаётся на месте.
        state.open_letter("salary_bill");
        assert!(state.letters.iter().any(|letter| letter.id == "salary_bill"));

        // Состояние счёта не переписывается повторным тиком.
        assert_eq!(state.salary_bill_stage, state::SalaryBillStage::Paid);
        let line = salary_bill_line(&state);
        assert!(!line.is_empty());
        assert!(line.contains("300"), "в счёте нет суммы: {line}");

        // Нечем платить — счёт меняется на «не хватило», а письмо то же.
        state.gold = 0.0;
        assert_eq!(pay_salaries(&mut state), SalaryOutcome::Unpaid);
        assert_eq!(state.salary_bill_stage, state::SalaryBillStage::Unpaid);
        assert_eq!(
            state
                .letters
                .iter()
                .filter(|letter| letter.id == "salary_bill")
                .count(),
            1
        );
    }

    #[test]
    fn new_demand_starts_at_the_equilibrium_capacity() {
        let mut state = city_with_shop();
        state.add_item("robe", 10);
        add_slot(&mut state, "robe", 10, 1.0).unwrap();
        advance_demand(&mut state, balance::DEMAND_STEP_SECS);
        let entry = state.city.demand.get("robe").cloned().unwrap();
        // Ёмкость — от тира, а не «бюджет / цена»: дешёвой робы не бывает
        // полутора миллионов штук.
        assert!((entry.capacity - demand_capacity("robe")).abs() < 400.0);
        assert!(entry.available <= entry.capacity + 1e-6);
        assert!(entry.capacity < 5_000.0, "ёмкость {}", entry.capacity);
    }

    #[test]
    fn town_status_thresholds_match_the_traders() {
        let mut state = city_with_shop();
        let status_of = |state: &mut GameState, demand: u8, supply: u8| {
            state.city.demand.insert(
                "robe".to_string(),
                DemandEntry {
                    demand,
                    supply,
                    ..DemandEntry::default()
                },
            );
            demand_status(state, "robe")
        };
        assert_eq!(status_of(&mut state, 80, 50), MarketStatus::Deficit);
        assert_eq!(status_of(&mut state, 50, 80), MarketStatus::Surplus);
        assert_eq!(status_of(&mut state, 55, 50), MarketStatus::Balance);
    }

    #[test]
    fn city_market_resets_with_rebirth() {
        let mut state = city_with_shop();
        state.city.shop.as_mut().unwrap().rating = 4.0;
        state.city.travelers.push(Traveler::default());
        state.city.hire_charter = true;
        state.cycles = 3;
        state.reset_for_cycle();
        assert!(!state.city.unlocked);
        assert!(state.city.shop.is_none());
        assert!(state.city.travelers.is_empty());
        assert!(!state.city.hire_charter);
        // И сюжет начинается заново: на доске снова квест Мартина.
        assert!(state
            .quests
            .iter()
            .any(|quest| quest.template_id == INTRO_TEMPLATE_ID));
    }

    #[test]
    fn old_demand_saves_migrate_to_the_pair_model() {
        let mut state = city_with_shop();
        state.city.demand.insert(
            "robe".to_string(),
            DemandEntry {
                available: 400.0,
                capacity: 500.0,
                ..DemandEntry::default()
            },
        );
        migrate_demand(&mut state);
        let entry = demand_of(&state, "robe");
        assert_eq!(entry.demand, balance::DEMAND_EQUILIBRIUM);
        assert_eq!(entry.supply, balance::DEMAND_EQUILIBRIUM);
        assert!(entry.available <= entry.capacity);
    }

    #[test]
    fn donation_scales_blessing_with_amount() {
        let mut state = city_with_shop();
        state.gold = 20_000.0;

        donate(&mut state, 100.0).expect("пожертвование");
        assert!((state.city.blessing_until - 10.0 * 60.0).abs() < 1e-6);

        state.city.blessing_until = 0.0;
        donate(&mut state, 10_000.0).expect("пожертвование");
        assert!((state.city.blessing_until - 100.0 * 60.0).abs() < 1e-6);
    }

    #[test]
    fn event_fair_doubles_flow() {
        let mut state = city_with_shop();
        state.city.shop.as_mut().unwrap().rating = 3.0;
        let base = buyer_interval(&state).unwrap();
        state.city.active_event = Some(CityEvent::Fair { ends_at: 10_000.0 });
        let fair = buyer_interval(&state).unwrap();
        assert!((fair - base / 2.0).abs() < 1e-6);
    }

    #[test]
    fn event_epidemic_blocks() {
        let mut state = city_with_shop();
        state.city.active_event = Some(CityEvent::Epidemic { ends_at: 10_000.0 });
        assert!(buyer_interval(&state).is_none());
        assert!(epidemic_active(&state));
    }

    #[test]
    fn cash_upgrades_reach_a_hundred_thousand() {
        let mut state = city_with_shop();
        state.city.guild_unlocked = true;
        state.gold = 1_000_000.0;
        for id in ["cash_1", "cash_2", "cash_3", "cash_4", "cash_5"] {
            buy_upgrade(&mut state, id).expect(id);
        }
        let shop = state.city.shop.as_ref().unwrap();
        assert!((shop.cash_cap - 100_000.0).abs() < 1e-6);
        assert!((shop.cash_cap - balance::SHOP_MAX_CASH_CAP).abs() < 1e-6);
    }

    #[test]
    fn guild_upgrades_follow_their_chains() {
        let mut state = city_with_shop();
        state.city.guild_unlocked = true;
        state.gold = 1_000_000.0;

        // Первые звенья цепочек видны, а вторые — нет.
        let visible: Vec<&str> = available_upgrades(&state)
            .into_iter()
            .map(|def| def.id)
            .collect();
        assert!(visible.contains(&"cash_1"));
        assert!(visible.contains(&"slots_1"));
        assert!(!visible.contains(&"cash_2"));

        // Без предшественника купить нельзя.
        assert_eq!(
            buy_upgrade(&mut state, "cash_2"),
            Err(GuildError::PrerequisiteMissing)
        );

        // Купил первое звено — на его месте появилось следующее.
        buy_upgrade(&mut state, "cash_1").expect("cash_1");
        let visible: Vec<&str> = available_upgrades(&state)
            .into_iter()
            .map(|def| def.id)
            .collect();
        assert!(!visible.contains(&"cash_1"));
        assert!(visible.contains(&"cash_2"));
    }

    #[test]
    fn assistant_upgrade_is_gone() {
        let mut state = city_with_shop();
        state.city.guild_unlocked = true;
        assert!(GUILD_UPGRADES.iter().all(|def| def.id != "assistant"));
        assert!(available_upgrades(&state)
            .iter()
            .all(|def| def.id != "assistant"));
        assert_eq!(
            buy_upgrade(&mut state, "assistant"),
            Err(GuildError::UnknownUpgrade)
        );
    }

    #[test]
    fn guild_requires_sales_and_gold() {
        let mut state = city_with_shop();
        state.gold = 10_000.0;
        state.city.shop.as_mut().unwrap().total_sales = 14;
        assert_eq!(join_guild(&mut state), Err(GuildError::NotEnoughSales));

        state.city.shop.as_mut().unwrap().total_sales = 15;
        state.gold = 100.0;
        assert_eq!(join_guild(&mut state), Err(GuildError::NotEnoughGold));

        state.gold = 10_000.0;
        join_guild(&mut state).unwrap();
        assert!(state.city.guild_unlocked);
        assert!((state.gold - 6_000.0).abs() < 1e-6);
    }

    #[test]
    fn charter_opens_paid_hiring() {
        let mut state = city_with_shop();
        state.sawmill_level = 2;

        assert_eq!(buy_charter(&mut state), Err(HireError::NoTownHall));
        assert_eq!(
            adjust_hired(&mut state, HireLine::Sawmill, 1),
            Err(HireError::NoCharter)
        );

        state.city.town_hall_unlocked = true;
        state.gold = balance::CHARTER_COST;
        buy_charter(&mut state).unwrap();
        assert!(state.city.hire_charter);
        assert!(state.gold.abs() < 1e-6);
        assert_eq!(buy_charter(&mut state), Err(HireError::AlreadyHave));

        for _ in 0..3 {
            adjust_hired(&mut state, HireLine::Sawmill, 1).unwrap();
        }
        assert_eq!(hired_count(&state, HireLine::Sawmill), 2);
        assert_eq!(hired_cap(&state, HireLine::Sawmill), 2);
    }

    #[test]
    fn hired_workers_gather_more_than_communal() {
        let mut state = GameState::default();
        state.sawmill_level = 1;
        state.sawmill_workers = 1;
        state.city.hired_sawmill = 1;
        let communal = state.sawmill_wood_per_sec();
        let hired = state.hired_sawmill_wood_per_sec();
        assert!((hired - communal * balance::HIRE_YIELD_MULT).abs() < 1e-9);

        state.city.hired_coal = 1;
        assert!((state.hired_coal_workers() - balance::HIRE_YIELD_MULT).abs() < 1e-9);
    }

    #[test]
    fn missing_wages_send_hired_home() {
        let mut state = city_with_shop();
        state.city.hire_charter = true;
        state.city.hired_sawmill = 2;
        state.city.clerk_hired = true;
        state.city.smiths[0].built = true;
        state.city.smiths[0].wage_due = 50.0;
        state.gold = 100.0;

        assert_eq!(pay_salaries(&mut state), SalaryOutcome::Unpaid);
        assert_eq!(state.city.hired_sawmill, 0);
        assert!(!state.city.clerk_hired);
        assert!(state.city.smiths[0].paused);
        assert!(state.letters.iter().any(|letter| letter.id == "hired_left"));

        state.city.hired_sawmill = 1;
        state.gold = 1_000.0;
        let due = salary_total(&state);
        assert_eq!(pay_salaries(&mut state), SalaryOutcome::Paid);
        assert!(!state.city.smiths[0].paused);
        assert!((state.city.smiths[0].wage_due).abs() < 1e-9);
        assert!((state.gold - (1_000.0 - due)).abs() < 1e-6);
    }

    /// Город с деньгами и материалами на постройку кузницы.
    fn smith_ready_city() -> GameState {
        let mut state = city_with_shop();
        // Склад у тестового города маленький: кладём материалы напрямую.
        state.active_resources.insert("wood".to_string(), 3_000);
        state.active_resources.insert("stone".to_string(), 2_000);
        state.gold = 10_000.0;
        state
    }

    #[test]
    fn smith_crafts_into_the_bag_and_owes_wages() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        assert!(state.city.smiths[0].built);

        assign_smith_recipe(&mut state, 0, "board").unwrap();
        assert_eq!(
            assign_smith_recipe(&mut state, 0, "board"),
            Err(SmithError::AlreadyAssigned)
        );
        assert_eq!(state.city.smiths[0].recipes.len(), 1);

        // Ровно две единицы дерева: постройка уже съела материал.
        state.active_resources.insert("wood".to_string(), 10);
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;
        tick_smiths(&mut state);
        assert_eq!(state.amount_of("board"), 1);
        assert_eq!(state.active_amount("wood"), 8);
        assert!(state.city.smiths[0].wage_due >= balance::SMITH_WAGE_MIN);
    }

    #[test]
    fn smith_stops_without_wages_or_materials() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        assign_smith_recipe(&mut state, 0, "board").unwrap();
        state.add_active("wood", 100);
        state.city.smiths[0].wage_due = 40.0;
        state.gold = 0.0;

        assert_eq!(pay_salaries(&mut state), SalaryOutcome::Unpaid);
        assert!(state.city.smiths[0].paused);
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;
        tick_smiths(&mut state);
        assert_eq!(state.amount_of("board"), 0);

        // Снова с деньгами, но без дерева — тоже стоит.
        state.city.smiths[0].paused = false;
        state.active_resources.insert("wood".to_string(), 0);
        state.city.smiths[0].next_craft_at = 1_000.0;
        tick_smiths(&mut state);
        assert_eq!(state.amount_of("board"), 0);
    }

    #[test]
    fn second_smith_needs_the_town_hall_permit() {
        let mut state = smith_ready_city();
        assert_eq!(build_smith(&mut state, 1), Err(SmithError::MissingPermit));

        state.city.town_hall_unlocked = true;
        state.gold = 30_000.0;
        buy_smith_permit(&mut state).unwrap();
        assert!(state.city.smith_second_permit);
        assert_eq!(buy_smith_permit(&mut state), Err(SmithError::PermitOwned));

        state.gold = 10_000.0;
        state.active_resources.insert("wood".to_string(), 3_000);
        state.active_resources.insert("stone".to_string(), 2_000);
        build_smith(&mut state, 1).unwrap();
        assert!(state.city.smiths[1].built);
        assert_eq!(build_smith(&mut state, 1), Err(SmithError::Built));
    }

    #[test]
    fn smith_queue_respects_slots_and_priority() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        let ids: Vec<&'static str> = items::recipes()
            .iter()
            .map(|recipe| recipe.id)
            .take(6)
            .collect();
        for id in ids.iter().take(5) {
            assign_smith_recipe(&mut state, 0, id).unwrap();
        }
        assert_eq!(
            assign_smith_recipe(&mut state, 0, ids[5]),
            Err(SmithError::Full)
        );

        move_smith_recipe(&mut state, 0, ids[4], -1).unwrap();
        assert_eq!(state.city.smiths[0].recipes[3], ids[4]);
        assert_eq!(state.city.smiths[0].recipes[4], ids[3]);

        unassign_smith_recipe(&mut state, 0, ids[4]);
        assert!(!state.city.smiths[0].recipes.iter().any(|id| id == ids[4]));

        state.gold = 50_000.0;
        upgrade_smith(&mut state, 0, SmithLine::Slots).unwrap();
        assert_eq!(smith_slots(&state, 0), balance::SMITH_START_SLOTS + 1);
    }

    #[test]
    fn smith_fire_is_his_own_and_comes_from_coal() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        state.city.smiths[0].fire = 0;
        state.add_item("iron", 10);
        state.add_active("coal", 10);
        state.current_fire = 0;
        assign_smith_recipe(&mut state, 0, "iron_ingot").unwrap();
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;

        tick_smiths(&mut state);
        assert_eq!(state.amount_of("iron_ingot"), 1);
        // Огонь игрока не тронут, а горн кузнеца заправился углём.
        assert_eq!(state.current_fire, 0);
        assert!(state.city.smiths[0].fire > 0);
        assert!(state.active_amount("coal") < 10);
    }

    #[test]
    fn smith_sends_gear_to_the_shop_stock() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        let recipe_id = items::recipes()
            .iter()
            .find(|recipe| {
                items::item(recipe.result_id)
                    .map(|item| item.slot.is_some())
                    .unwrap_or(false)
            })
            .map(|recipe| recipe.id)
            .expect("в каталоге есть снаряжение");
        let recipe = items::recipe(recipe_id).unwrap();
        state.smith_level = items::recipe_min_smith_level(recipe_id).max(1);
        for ingredient in &recipe.ingredients {
            if ingredient.item_id == "fire" {
                continue;
            }
            if state::is_active_resource(ingredient.item_id) {
                state.add_active(ingredient.item_id, ingredient.quantity);
            } else {
                state.add_item(ingredient.item_id, ingredient.quantity);
            }
        }
        state.city.smiths[0].fire = balance::smith_fire_capacity(0);
        assign_smith_recipe(&mut state, 0, recipe_id).unwrap();
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;

        tick_smiths(&mut state);
        assert!(
            state
                .city
                .stock
                .iter()
                .any(|row| row.item_id == recipe.result_id && row.quantity > 0),
            "снаряжение должно уйти на склад приказчика"
        );
        assert_eq!(state.amount_of(recipe.result_id), 0);
    }

    #[test]
    fn smith_does_not_catch_up_after_a_long_gap() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        assign_smith_recipe(&mut state, 0, "board").unwrap();
        state.add_active("wood", 100);
        state.city.smiths[0].next_craft_at = 100.0;
        // Пропущено десять минут: кузнец кует один раз, а не «догоняет».
        state.total_play_seconds = 700.0;
        tick_smiths(&mut state);
        assert_eq!(state.amount_of("board"), 1);
        assert!(state.city.smiths[0].next_craft_at >= 700.0);
    }

    #[test]
    fn old_smith_house_becomes_the_first_smith() {
        let mut state = GameState::default();
        state.city.legacy_smith_level = 3;
        state.city.legacy_smith_recipes = vec!["board".to_string(), "cloth".to_string()];
        migrate_smiths(&mut state);
        let smith = &state.city.smiths[0];
        assert!(smith.built);
        assert_eq!(smith.speed_level, 2);
        assert_eq!(smith.forge_level, 2);
        assert_eq!(smith.recipes, vec!["board", "cloth"]);
    }

    #[test]
    fn migrating_twice_keeps_the_same_smith() {
        let mut state = smith_ready_city();
        state.gold = 10_000.0;
        build_smith(&mut state, 0).unwrap();
        state.city.legacy_smith_level = 9;
        migrate_smiths(&mut state);
        assert_eq!(state.city.smiths[0].speed_level, 0);
    }

    #[test]
    fn smith_does_not_burn_the_coal_the_recipe_needs() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        // Свеча сама требует уголь: горн не должен его съесть.
        state.add_item("fiber", 5);
        state.active_resources.insert("coal".to_string(), 1);
        state.city.smiths[0].fire = 0;
        state.current_fire = 300;
        assign_smith_recipe(&mut state, 0, "candle").unwrap();
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;

        tick_smiths(&mut state);
        // Ровно один уголь: он нужен рецепту, горн не вправе его сжечь.
        let candles = state.amount_of("candle");
        assert_eq!(state.current_fire, 300);
        // Уголь либо нужен рецепту, либо его сожгли вместе со свечой.
        assert_eq!(state.city.smiths[0].fire, 0);
        if candles > 0 {
            assert_eq!(state.city.smiths[0].wage_due, balance::SMITH_WAGE_MIN);
        }
    }

    #[test]
    fn smith_keeps_ingredients_when_there_is_no_room() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        state.active_resources.insert("wood".to_string(), 50);
        // Забиваем сумку чужими предметами, чтобы для доски не было ячейки.
        for item in items::catalog().iter() {
            if item.id == "board" {
                continue;
            }
            let free = state.inventory_free_slots();
            if free == 0 {
                break;
            }
            state.inventory.insert(item.id.to_string(), 1);
        }
        assert_eq!(state.inventory_free_slots(), 0);
        assign_smith_recipe(&mut state, 0, "board").unwrap();
        state.total_play_seconds = 1_000.0;
        state.city.smiths[0].next_craft_at = 1_000.0;

        tick_smiths(&mut state);
        assert_eq!(state.amount_of("board"), 0);
        assert_eq!(state.active_amount("wood"), 50);
    }

    #[test]
    fn time_warp_does_not_speed_up_smiths() {
        let mut state = smith_ready_city();
        build_smith(&mut state, 0).unwrap();
        state.active_resources.insert("wood".to_string(), 100);
        assign_smith_recipe(&mut state, 0, "board").unwrap();
        state.time = 10.0;
        let before = state.total_play_seconds;
        actions::time_warp(&mut state);
        // Кузнец не кует «задним числом»: тика нет, варп город не двигает.
        assert_eq!(state.amount_of("board"), 0);
        assert!(state.total_play_seconds >= before);
    }

    #[test]
    fn clerk_lists_goods_with_his_markup() {
        let mut state = city_with_shop();
        state.gold = balance::CLERK_PRICE;
        hire_clerk(&mut state).unwrap();
        assert!(state.city.clerk_hired);
        set_clerk_mult(&mut state, 1.4);
        assert!((state.city.clerk_mult - 1.4).abs() < 1e-6);

        stock_in_test(&mut state, "robe", 5).unwrap();
        state.city.next_clerk_at = 100.0;
        state.total_play_seconds = 100.0;
        tick_clerk(&mut state);

        // Весь запас помещается в одну партию, склад опустел.
        assert!(state.city.stock.is_empty());
        let shop = state.city.shop.as_ref().unwrap();
        let slot = shop
            .slots
            .iter()
            .find(|slot| slot.item_id == "robe")
            .expect("robe is on the shelf");
        assert_eq!(slot.quantity, 5);
        assert!((slot.price_mult - 1.4).abs() < 1e-6);
    }

    #[test]
    fn clerk_fills_free_sheets_until_slots_run_out() {
        let mut state = city_with_shop();
        state.gold = balance::CLERK_PRICE;
        hire_clerk(&mut state).unwrap();
        for id in ["robe", "jug", "board", "cloth", "brick"] {
            stock_in_test(&mut state, id, 5).unwrap();
        }
        state.city.next_clerk_at = 100.0;
        state.total_play_seconds = 100.0;
        tick_clerk(&mut state);

        // Приказчик занимает все полки,
        // и ни один товар не теряется.
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.slots.len(), balance::SHOP_STARTING_SLOTS as usize);
        let on_shelves: u32 = shop.slots.iter().map(|slot| slot.quantity).sum();
        let in_stock: u32 = state.city.stock.iter().map(|row| row.quantity).sum();
        assert_eq!(on_shelves + in_stock, 25);
        assert!(on_shelves > 0);
    }

    #[test]
    fn warehouse_goods_reach_the_shelves() {
        let mut state = city_with_shop();
        state.gold = balance::CLERK_PRICE;
        hire_clerk(&mut state).unwrap();
        // Вёдра лежат только на складе — приказчик должен их выставить.
        stock_in_test(&mut state, "wooden_bucket", 3).unwrap();
        state.city.next_clerk_at = 100.0;
        state.total_play_seconds = 100.0;
        tick_clerk(&mut state);

        let shop = state.city.shop.as_ref().unwrap();
        let slot = shop
            .slots
            .iter()
            .find(|slot| slot.item_id == "wooden_bucket")
            .expect("buckets are on the shelf");
        assert_eq!(slot.quantity, 3);
        assert!(state
            .city
            .stock
            .iter()
            .all(|row| row.item_id != "wooden_bucket"));
    }

    #[test]
    fn stock_can_be_put_on_the_shelf_by_hand() {
        let mut state = city_with_shop();
        stock_in_test(&mut state, "wooden_bucket", 4).unwrap();
        let index = state
            .city
            .stock
            .iter()
            .position(|row| row.item_id == "wooden_bucket")
            .unwrap();
        stock_to_shelf(&mut state, index, 3).unwrap();
        {
            let shop = state.city.shop.as_ref().unwrap();
            let slot = shop
                .slots
                .iter()
                .find(|slot| slot.item_id == "wooden_bucket")
                .unwrap();
            assert_eq!(slot.quantity, 3);
        }
        assert_eq!(state.city.stock[0].quantity, 1);

        // Полки забиты — новый товар упирается в лимит.
        state.city.shop.as_mut().unwrap().max_slots = 1;
        stock_in_test(&mut state, "robe", 1).unwrap();
        let robe = state
            .city
            .stock
            .iter()
            .position(|row| row.item_id == "robe")
            .unwrap();
        assert_eq!(
            stock_to_shelf(&mut state, robe, 1),
            Err(ShopError::ShelfFull)
        );
    }

    #[test]
    fn stock_takes_goods_from_the_bag() {
        let mut state = city_with_shop();
        state.add_item("wooden_bucket", 4);
        add_stock(&mut state, "wooden_bucket", 3).unwrap();
        assert_eq!(state.amount_of("wooden_bucket"), 1);
        assert_eq!(state.city.stock[0].quantity, 3);

        // Ещё раз — в ту же ячейку, без новой.
        add_stock(&mut state, "wooden_bucket", 1).unwrap();
        assert_eq!(state.city.stock.len(), 1);
        assert_eq!(state.city.stock[0].quantity, 4);
        assert_eq!(state.amount_of("wooden_bucket"), 0);

        // Нет в сумке — нет на складе.
        assert_eq!(
            add_stock(&mut state, "wooden_bucket", 1),
            Err(ShopError::MissingItem)
        );
    }

    #[test]
    fn stock_goods_come_back_to_the_bag() {
        let mut state = city_with_shop();
        stock_in_test(&mut state, "wooden_bucket", 6).unwrap();
        take_stock_amount(&mut state, 0, 2).unwrap();
        assert_eq!(state.amount_of("wooden_bucket"), 2);
        assert_eq!(state.city.stock[0].quantity, 4);

        // Больше, чем на складе — последняя ячейка выдаётся целиком.
        take_stock_amount(&mut state, 0, 10).unwrap();
        assert_eq!(state.amount_of("wooden_bucket"), 6);
        assert!(state.city.stock.is_empty());
    }

    #[test]
    fn clerk_stock_grows_with_guild_upgrades() {
        let mut state = city_with_shop();
        assert_eq!(clerk_stock_cells(&state), balance::CLERK_START_CELLS);
        state.city.stock_cells_bought = 1;
        assert_eq!(clerk_stock_cells(&state), 10);
        stock_in_test(&mut state, "robe", 1).unwrap();
        // Забираем со склада обратно в рюкзак.
        let index = state
            .city
            .stock
            .iter()
            .position(|row| row.item_id == "robe")
            .unwrap();
        take_stock_amount(&mut state, index, u32::MAX).unwrap();
        assert!(state.city.stock.is_empty());
        assert_eq!(state.amount_of("robe"), 1);
    }

    #[test]
    fn sold_out_shelf_frees_the_slot_for_the_clerk() {
        let mut state = city_with_shop();
        state.gold = balance::CLERK_PRICE;
        hire_clerk(&mut state).unwrap();
        stock_in_test(&mut state, "robe", 10).unwrap();
        state.city.next_clerk_at = 100.0;
        state.total_play_seconds = 100.0;
        tick_clerk(&mut state);
        let index = state
            .city
            .shop
            .as_ref()
            .unwrap()
            .slots
            .iter()
            .position(|slot| slot.item_id == "robe")
            .unwrap();
        let before = state.city.shop.as_ref().unwrap().slots.len();
        sell_slot(&mut state, index, true).unwrap();
        // Распроданная полка снимается: карточка ×0 не висит в лавке.
        let shop = state.city.shop.as_ref().unwrap();
        assert_eq!(shop.slots.len(), before - 1);
        assert!(shop.slots.iter().all(|slot| slot.item_id != "robe"));
        assert!(shop.slots.iter().all(|slot| slot.quantity > 0));
    }

    #[test]
    fn stock_upgrade_chain_is_three_steps() {
        let mut state = city_with_shop();
        state.city.guild_unlocked = true;
        state.gold = 1_000_000.0;
        for id in ["stock_1", "stock_2", "stock_3"] {
            buy_upgrade(&mut state, id).expect(id);
        }
        assert_eq!(state.city.stock_cells_bought, 3);
        assert_eq!(clerk_stock_cells(&state), 20);
    }

    #[test]
    fn travelers_start_with_three_characters() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        assert_eq!(state.city.travelers.len(), balance::TRAVELER_COUNT);
        let ids: Vec<usize> = state
            .city
            .travelers
            .iter()
            .map(|traveler| traveler.profile_id)
            .collect();
        assert_eq!(ids, vec![0, 1, 2]);
        // Повторный вызов ничего не добавляет и не сбрасывает дружбу.
        state.city.travelers[0].friendship = 20;
        ensure_travelers(&mut state);
        assert_eq!(state.city.travelers.len(), balance::TRAVELER_COUNT);
        assert_eq!(state.city.travelers[0].friendship, 20);
    }

    #[test]
    fn talk_costs_gold_and_adds_friendship() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);

        state.gold = 50.0;
        assert_eq!(
            talk_to_traveler(&mut state, 0),
            Err(TravelerError::NotEnoughGold)
        );

        state.gold = balance::TRAVELER_TALK_PRICE * 10.0;
        talk_to_traveler(&mut state, 0).unwrap();
        assert_eq!(
            state.city.travelers[0].friendship,
            balance::TRAVELER_FRIENDSHIP_POINT
        );
        assert_eq!(traveler_circles(&state.city.travelers[0]), 1);

        // Потолок дружбы — пять кружков, дальше платить не дадут.
        for _ in 0..4 {
            talk_to_traveler(&mut state, 0).unwrap();
        }
        assert_eq!(
            state.city.travelers[0].friendship,
            balance::TRAVELER_FRIENDSHIP_MAX
        );
        assert_eq!(talk_to_traveler(&mut state, 0), Err(TravelerError::NoQuest));
    }

    #[test]
    fn traveler_quest_chain_leads_to_friendship() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.city.travelers[0].friendship = 20;

        // Разговорные истории выдаются случайно: подбираем верный ответ
        // по самому квесту, а не по его месту в пуле.
        let quest = next_traveler_quest(&state.city.travelers[0]).unwrap();
        assert_eq!(quest.kind, TravelerQuestKind::Story);
        start_traveler_quest(&mut state, 0).unwrap();
        assert_eq!(
            start_traveler_quest(&mut state, 0),
            Err(TravelerError::QuestDecided)
        );
        assert!(answer_traveler_quest(&mut state, 0, "base").is_err());
        let first_id = state.city.travelers[0].active_quest.clone().unwrap();
        let good_choice = traveler_quest(&first_id)
            .unwrap()
            .choices
            .iter()
            .find(|choice| choice.good)
            .unwrap()
            .id;
        let good = answer_traveler_quest(&mut state, 0, good_choice).unwrap();
        assert!(good);
        assert_eq!(state.city.travelers[0].friendship, 30);
        finish_traveler_quest(&mut state, 0).unwrap();
        assert_eq!(traveler_circles(&state.city.travelers[0]), 3);

        // Второй разговорный квест на трёх кружках — другая история.
        let quest = next_traveler_quest(&state.city.travelers[0]).unwrap();
        assert_eq!(quest.kind, TravelerQuestKind::Story);
        start_traveler_quest(&mut state, 0).unwrap();
        let second_id = state.city.travelers[0].active_quest.clone().unwrap();
        assert_ne!(second_id, first_id, "история повторилась");
        let good_choice = traveler_quest(&second_id)
            .unwrap()
            .choices
            .iter()
            .find(|choice| choice.good)
            .unwrap()
            .id;
        answer_traveler_quest(&mut state, 0, good_choice).unwrap();
        finish_traveler_quest(&mut state, 0).unwrap();
        assert_eq!(state.city.travelers[0].friendship, 40);

        // На четырёх кружках — просьба о компасе.
        let quest = next_traveler_quest(&state.city.travelers[0]).unwrap();
        assert_eq!(quest.kind, TravelerQuestKind::Compass);
        assert_eq!(
            TravelerQuestStep::of(&state.city.travelers[0]),
            TravelerQuestStep::Start("captain_compass_ask".to_string())
        );
        start_traveler_quest(&mut state, 0).unwrap();
        assert_eq!(
            deliver_compass(&mut state, 0),
            Err(TravelerError::NoCompass)
        );

        state.add_item(TRAVELER_COMPASS_ITEM, 1);
        deliver_compass(&mut state, 0).unwrap();
        assert_eq!(state.item_count(TRAVELER_COMPASS_ITEM), 0);
        finish_traveler_quest(&mut state, 0).unwrap();
        assert!(traveler_is_friend(&state.city.travelers[0]));
        assert_eq!(state.city.travelers[0].friendship, 50);
        assert!(state.city.traveler_house_offered);
        assert!(state
            .letters
            .iter()
            .any(|letter| letter.id == "traveler_compass_brought"));
        assert_eq!(
            TravelerQuestStep::of(&state.city.travelers[0]),
            TravelerQuestStep::OfferHouse
        );
    }

    #[test]
    fn failed_quest_leaves_the_pool_and_drops_friendship() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.city.travelers[0].friendship = 30;

        start_traveler_quest(&mut state, 0).unwrap();
        let failed_id = state.city.travelers[0].active_quest.clone().unwrap();
        let bad_choice = traveler_quest(&failed_id)
            .unwrap()
            .choices
            .iter()
            .find(|choice| !choice.good)
            .unwrap()
            .id;
        let good = answer_traveler_quest(&mut state, 0, bad_choice).unwrap();
        assert!(!good);
        // Ошибка не отнимает дружбу: история просто блокируется.
        assert_eq!(state.city.travelers[0].friendship, 30);
        finish_traveler_quest(&mut state, 0).unwrap();

        assert!(state.city.travelers[0]
            .quests_failed
            .iter()
            .any(|id| id == &failed_id));

        // Проваленный квест больше не предлагают — идёт другая история.
        let quest = next_traveler_quest(&state.city.travelers[0]).unwrap();
        assert_eq!(quest.kind, TravelerQuestKind::Story);
        assert_ne!(quest.id, failed_id);
    }

    #[test]
    fn old_save_gets_three_fresh_travelers() {
        let mut state = city_with_shop();
        state.city.travelers.clear();
        ensure_travelers(&mut state);
        assert_eq!(state.city.travelers.len(), balance::TRAVELER_COUNT);
        assert!(state
            .city
            .travelers
            .iter()
            .all(|traveler| traveler.friendship == 0 && traveler.active_quest.is_none()));
    }

    #[test]
    fn traveler_leaves_and_comes_back() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        let location = &balance::TRAVELER_LOCATIONS[0];
        state.gold = location.advance + 10.0;
        state.total_play_seconds = 100.0;
        send_traveler(&mut state, 0, location.id).unwrap();
        assert!(!traveler_ready(&state.city.travelers[0]));
        assert!((state.gold - 10.0).abs() < 1e-6);
        assert_eq!(
            send_traveler(&mut state, 0, location.id),
            Err(TravelerError::Busy)
        );

        // Локация за домом закрыта, пока нет аренды земли.
        state.gold = 10_000.0;
        assert_eq!(
            send_traveler(&mut state, 1, "old_mines"),
            Err(TravelerError::BadTraveler)
        );

        state.total_play_seconds = 100.0 + location.minutes * 60.0 + 1.0;
        tick_travelers(&mut state);
        assert!(traveler_ready(&state.city.travelers[0]));
        assert!(state.letters.iter().any(|letter| {
            letter.id == format!("traveler_back:{}", state.city.travelers[0].profile_id)
                || letter.id == format!("traveler_empty:{}", state.city.travelers[0].profile_id)
        }));
    }

    #[test]
    fn traveler_payout_is_fifteen_percent_of_loot() {
        let loot = vec![("wood".to_string(), 100u32)];
        let value = traveler_loot_value(&loot);
        let payout = traveler_payout_value(&loot, TravelerGearBonus::default());
        assert!((payout - value * balance::TRAVELER_PAYOUT_LOOT_SHARE).abs() < 1.0);
        // Пустой поход — ни доплаты, ни денег.
        assert!(traveler_payout_value(&[], TravelerGearBonus::default()) < 1e-9);
    }

    #[test]
    fn trade_gear_raises_the_payout_share() {
        // У дерева целая цена — сравниваем на дорогом товаре.
        let loot = vec![("silver_ingot".to_string(), 100u32)];
        let plain = traveler_payout_value(&loot, TravelerGearBonus::default());
        let with_trade = traveler_payout_value(
            &loot,
            TravelerGearBonus {
                all: 0.0,
                payout: 0.5,
            },
        );
        assert!(with_trade >= plain);
        // Даже нулевой лут с торговым бонусом не даёт больше полной стоимости.
        let value = traveler_loot_value(&loot);
        assert!(with_trade <= value);
    }

    #[test]
    fn empty_trip_burns_the_advance_and_pays_nothing() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.total_play_seconds = 20.0;
        let traveler = &mut state.city.travelers[0];
        traveler.returns_at = 10.0;
        traveler.trip = "no_such_location".to_string();
        traveler.advance = 20.0;
        traveler.carried.clear();
        let before = state.gold;

        // Неизвестной локации нет — лут пустой, аванс не возвращается.
        tick_travelers(&mut state);
        let traveler = &state.city.travelers[0];
        assert!(traveler.carried.is_empty());
        assert!(traveler.payout.abs() < 1e-9);
        assert!(traveler.last_empty);
        assert!((state.gold - before).abs() < 1e-9);
        assert!(state
            .letters
            .iter()
            .any(|letter| letter.id == format!("traveler_empty:{}", traveler.profile_id)));
    }

    #[test]
    fn returned_traveler_keeps_loot_until_claimed() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.total_play_seconds = 20.0;
        let traveler = &mut state.city.travelers[0];
        traveler.returns_at = 10.0;
        traveler.trip = balance::TRAVELER_LOCATIONS[0].id.to_string();
        traveler.carried.clear();

        tick_travelers(&mut state);
        let traveler = &state.city.travelers[0];
        // Лут остаётся у путешественника, доплата посчитана от его стоимости.
        if !traveler.carried.is_empty() {
            assert!(traveler.payout > 0.0);
            let value = traveler_loot_value(&traveler.carried);
            assert!(traveler.payout <= value);
        }
    }

    #[test]
    fn claim_moves_loot_and_pays_the_payout() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.gold = 0.0;
        let traveler = &mut state.city.travelers[0];
        traveler.carried = vec![("wood".to_string(), 10)];
        traveler.payout = 25.0;
        traveler.returns_at = 0.0;

        claim_traveler_loot(&mut state, 0).unwrap();
        assert!(state.active_amount("wood") >= 10);
        assert!((state.gold - 25.0).abs() < 1e-6);
        assert!(state.city.travelers[0].carried.is_empty());
        assert!(state.city.travelers[0].payout.abs() < 1e-9);
        // Повторно забирать нечего.
        assert_eq!(
            claim_traveler_loot(&mut state, 0),
            Err(TravelerError::NothingToClaim)
        );
    }

    #[test]
    fn traveler_with_loot_cannot_be_sent_again() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.gold = 10_000.0;
        state.city.travelers[0].carried = vec![("wood".to_string(), 1)];
        let location = &balance::TRAVELER_LOCATIONS[0];
        assert_eq!(
            send_traveler(&mut state, 0, location.id),
            Err(TravelerError::LootWaiting)
        );
    }

    #[test]
    fn gear_needs_the_house() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.inventory.insert("leather_hat".to_string(), 1);
        assert_eq!(
            equip_traveler_gear(&mut state, 0, 0, "leather_hat"),
            Err(TravelerError::BadTraveler)
        );
        assert_eq!(state.item_count("leather_hat"), 1);
    }

    #[test]
    fn gear_bonus_raises_loot_and_payout() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.city.traveler_house_rented = true;
        state.city.traveler_house_stage = balance::TRAVELER_HOUSE_STAGES;
        // Снаряжение выдаётся только после постройки дома.
        state.city.traveler_house_ends_at = 0.0;
        state.inventory.insert("leather_hat".to_string(), 1);
        equip_traveler_gear(&mut state, 0, 2, "leather_hat").unwrap();
        assert_eq!(state.item_count("leather_hat"), 0);
        assert_eq!(
            state.city.travelers[0].gear[2].as_deref(),
            Some("leather_hat")
        );
        let bonus = traveler_gear_bonus(&state, 0);
        assert!(bonus.all > 0.0);
        // Снять обратно — вещь возвращается в сумку.
        unequip_traveler_gear(&mut state, 0, 2).unwrap();
        assert_eq!(state.item_count("leather_hat"), 1);
        assert!(state.city.travelers[0].gear[2].is_none());
    }

    #[test]
    fn land_needs_a_friend_and_an_agreement() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        state.city.town_hall_unlocked = true;
        state.gold = 1_000_000.0;

        // Дружбы нет — Ратуша землю не выделяет.
        assert_eq!(rent_traveler_house(&mut state), Err(TravelerError::HouseNotOffered));
        assert!(!traveler_house_offer_made(&state));

        // Друг позвал, но игрок ещё не ответил.
        state.city.traveler_house_offered = true;
        assert_eq!(rent_traveler_house(&mut state), Err(TravelerError::HouseNotOffered));

        // Согласился — землю можно арендовать, стройка тоже открыта.
        traveler_house_decision(&mut state, true);
        assert!(traveler_house_offer_made(&state));
        assert_eq!(start_traveler_house_stage(&mut state), Err(TravelerError::NoLand));
        rent_traveler_house(&mut state).unwrap();
        assert!(traveler_house_rented(&state));
    }

    #[test]
    fn one_friend_is_enough_for_the_house() {
        let mut state = city_with_shop();
        ensure_travelers(&mut state);
        // Сблизился только первый — третий по-прежнему чужой.
        state.city.travelers[0].friendship = balance::TRAVELER_FRIENDSHIP_MAX;
        assert!(traveler_is_friend(&state.city.travelers[0]));
        assert!(!traveler_is_friend(&state.city.travelers[2]));
        // Друг сам позвал строить дом — согласие открывает землю.
        state.city.traveler_house_offered = true;

        traveler_house_decision(&mut state, true);
        assert!(traveler_house_offer_made(&state));
    }

    #[test]
    fn offline_charges_wages_once() {
        let mut state = city_with_shop();
        state.city.hire_charter = true;
        state.city.hired_mine = 1;
        state.gold = 1_000.0;

        catch_up_offline(&mut state, balance::CITY_TAX_INTERVAL * 3.0);
        assert!((state.gold - (1_000.0 - balance::HIRE_SALARY)).abs() < 1e-6);
    }

    #[test]
    fn production_tax_is_charged_with_the_city_tax() {
        let mut state = city_with_shop();
        state.gold = 5_000.0;
        state.city.production_value = 10_000.0;

        assert_eq!(pay_tax(&mut state), TaxOutcome::Paid);
        // 5% от 10 000 = 500: касса пуста, списывается с золота.
        assert!((state.gold - 4_500.0).abs() < 1e-6);
        assert!(state.city.production_value.abs() < 1e-6);
    }

    #[test]
    fn unpaid_production_tax_leaves_a_bill() {
        let mut state = city_with_shop();
        state.gold = 0.0;
        state.city.production_value = 10_000.0;

        assert_eq!(pay_tax(&mut state), TaxOutcome::Short);
        assert!((state.city.tax_due - 500.0).abs() < 1e-6);
    }

    #[test]
    fn lines_report_production_value_for_the_tax() {
        let mut state = city_with_shop();
        state.sawmill_level = 1;
        state.sawmill_workers = 1;

        actions::tick(&mut state, 10.0);
        // 5 дерева по 0.5 — 2.5 стоимости потока.
        assert!((state.city.production_value - 2.5).abs() < 1e-6);
    }

    #[test]
    fn production_before_the_city_opens_is_not_taxed() {
        // Регрессия: лесопилка молотит с первого дня, и раньше её поток
        // копился в `production_value`. Открыв лавку, игрок получал счёт
        // за всё, что произвёл до города.
        let mut state = GameState::default();
        assert!(!state.city.unlocked);
        state.energy = 10_000.0;
        state.sawmill_level = 1;
        state.sawmill_workers = 1;
        actions::tick(&mut state, 10.0);
        assert!(state.city.production_value.abs() < 1e-6);

        unlock_city(&mut state);
        state.energy = 10_000.0;
        actions::tick(&mut state, 10.0);
        // После открытия города налог снова считает поток.
        assert!(state.city.production_value > 0.0);
    }

    #[test]
    fn unlocking_clears_the_old_production_bill() {
        let mut state = GameState::default();
        state.city.production_value = 10_000.0;
        unlock_city(&mut state);
        assert!(state.city.production_value.abs() < 1e-6);
    }
}
