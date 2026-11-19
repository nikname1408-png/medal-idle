use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::balance;
use super::city::{self, CityState};
use super::events::{self, ActionTrigger, BuffInstance, PendingEvent};
use super::i18n;
use super::items::{self, EquipSlot};
use super::quests::Quest;
use super::trader::{self, TraderMarket};

/// Виды ресурсов в баре: дерево, камень, уголь, огонь, золото. Руда и прочие
/// находки живут в рюкзаке.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Wood,
    Stone,
    Coal,
    Fire,
    Gold,
}

impl ResourceKind {
    /// Название ресурса для текстов (задел на будущие экраны).
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::Wood => i18n::t("res.wood"),
            Self::Stone => i18n::t("res.stone"),
            Self::Coal => i18n::t("res.coal"),
            Self::Fire => i18n::t("res.fire"),
            Self::Gold => i18n::t("res.gold"),
        }
    }
}

/// Ключ относится к активным ресурсам склада (а не к предметам рюкзака).
pub fn is_active_resource(key: &str) -> bool {
    matches!(key, "wood" | "stone" | "coal")
}

/// Направления изучения.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchKind {
    CoalPit,
    Forge,
    Bank,
}

#[allow(dead_code)]
impl ResearchKind {
    pub const ALL: [ResearchKind; 3] = [Self::CoalPit, Self::Forge, Self::Bank];

    pub fn name(self) -> &'static str {
        match self {
            Self::CoalPit => i18n::t("build.coal_pit"),
            Self::Forge => i18n::t("build.forge"),
            Self::Bank => i18n::t("build.bank"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::CoalPit => i18n::t("research.coal_pit"),
            Self::Forge => i18n::t("research.forge"),
            Self::Bank => i18n::t("research.bank"),
        }
    }

    /// Что нужно изучить перед этим направлением.
    pub fn requirement(self) -> Option<&'static str> {
        match self {
            Self::CoalPit => None,
            Self::Forge => Some(i18n::t("build.coal_pit")),
            Self::Bank => Some(i18n::t("build.forge")),
        }
    }
}

/// Надетая экипировка. Один предмет на слот.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EquippedItems {
    pub hand: Option<String>,
    pub body: Option<String>,
    pub back: Option<String>,
    pub head: Option<String>,
    pub feet: Option<String>,
    pub accessory1: Option<String>,
    pub accessory2: Option<String>,
    /// Легаси-поле: старые сохранения хранили один аксессуар здесь.
    pub accessory: Option<String>,
}

impl EquippedItems {
    pub fn get(&self, slot: EquipSlot) -> Option<&str> {
        match slot {
            EquipSlot::Hand => self.hand.as_deref(),
            EquipSlot::Body => self.body.as_deref(),
            EquipSlot::Back => self.back.as_deref(),
            EquipSlot::Head => self.head.as_deref(),
            EquipSlot::Feet => self.feet.as_deref(),
            EquipSlot::Accessory1 => self.accessory1.as_deref(),
            EquipSlot::Accessory2 => self.accessory2.as_deref(),
        }
    }

    pub fn set(&mut self, slot: EquipSlot, item_id: Option<String>) {
        match slot {
            EquipSlot::Hand => self.hand = item_id,
            EquipSlot::Body => self.body = item_id,
            EquipSlot::Back => self.back = item_id,
            EquipSlot::Head => self.head = item_id,
            EquipSlot::Feet => self.feet = item_id,
            EquipSlot::Accessory1 => self.accessory1 = item_id,
            EquipSlot::Accessory2 => self.accessory2 = item_id,
        }
    }

    pub fn clear(&mut self, slot: EquipSlot) {
        self.set(slot, None);
    }

    /// Старое поле `accessory` → первый слот аксессуаров.
    pub fn migrate_legacy_accessory(&mut self) {
        if self.accessory1.is_none() {
            if let Some(old) = self.accessory.take() {
                self.accessory1 = Some(old);
            }
        } else {
            self.accessory = None;
        }
    }
}

/// Переносит плащ или рюкзак из старого слота `Body` в новый `Back`.
///
/// Нужно для сохранений до v11: слот `Back` появился позже, и вещи
/// оставались надетым на тело.
fn move_body_item_to_back(equipment: &mut EquippedItems) {
    if equipment.back.is_some() {
        return;
    }
    let Some(body_id) = equipment.body.clone() else {
        return;
    };
    let is_back_item = matches!(
        body_id.as_str(),
        "backpack" | "beggars_cloak" | "leather_coat" | "researchers_cloak" | "travelers_cloak"
    );
    if is_back_item {
        equipment.back = Some(body_id);
        equipment.body = None;
    }
}

/// Ошибка надевания предмета.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipError {
    UnknownItem,
    NotOwned,
    NotEquippable,
    AlreadyEquipped,
    SlotOccupied,
}

impl EquipError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownItem => i18n::t("err.unknown_item"),
            Self::NotOwned => i18n::t("err.not_owned"),
            Self::NotEquippable => i18n::t("err.not_equippable"),
            Self::AlreadyEquipped => i18n::t("err.already_equipped"),
            Self::SlotOccupied => i18n::t("err.slot_occupied"),
        }
    }
}

/// Статистика за всё время (не сбрасывается циклом).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub wood_gathered: f64,
    pub stone_gathered: f64,
    pub coal_produced: f64,
    pub iron_mined: f64,
    pub gold_earned: f64,
    pub clicks: u64,
    pub rests: u64,
    pub play_seconds: f64,
}

/// Состояние одного достижения для экрана статистики.
#[derive(Clone, Debug, PartialEq)]
pub struct AchievementStatus {
    /// Ключ достижения: `ach.<id>.name` / `ach.<id>.desc` в словаре.
    pub id: &'static str,
    pub progress: f64,
    pub done: bool,
    pub value_text: String,
}

impl AchievementStatus {
    /// Название достижения на текущем языке.
    pub fn name(&self) -> &'static str {
        i18n::t_or(&format!("ach.{}.name", self.id), "")
    }

    /// Описание достижения на текущем языке.
    pub fn description(&self) -> &'static str {
        i18n::t_or(&format!("ach.{}.desc", self.id), "")
    }
}

/// Визуальная тема игры.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Classic,
    Dark,
}

/// Временные настройки разработчика. В сохранение не попадают.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DevSettings {
    /// Добавка к пассивной регенерации энергии в секунду.
    pub energy_regen: f64,
    /// Добавка к энергии за одно нажатие «Отдых».
    pub rest_bonus: f64,
    /// Добавка к количеству ресурса за клик добычи.
    pub gather_yield: f64,
    /// Автодобыча ресурсов в секунду.
    pub wood_per_sec: f64,
    pub stone_per_sec: f64,
    pub iron_per_sec: f64,
    pub coal_per_sec: f64,
    pub gold_per_sec: f64,
}

/// Стадия счёта по зарплатам: счёт остаётся в колокольчике после прочтения,
/// поэтому в нём должно стоять одно из трёх, а не «сколько насчитали в
/// прошлый тик».
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SalaryBillStage {
    /// Счёт ещё не выставляли.
    #[default]
    None,
    /// Выплатить сумели — счёт закрыт.
    Paid,
    /// Не хватило золота: наёмные ушли, кузнецы встали.
    Unpaid,
}

/// Письмо в колокольчике: отложенное предложение постройки.
///
/// Заголовок и текст берутся из словаря по `id` письма
/// (`letter.<id>.title` / `letter.<id>.body`), поэтому письмо меняет язык
/// вместе с интерфейсом.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Letter {
    pub id: String,
    /// Старое поле: раньше текст письма хранился прямо в сохранении. Теперь
    /// здесь пусто, а `migrate` его очищает.
    pub title: String,
    pub body: String,
    /// ID предложения, которое откроется по нажатию на письмо.
    pub offer: Option<String>,
    /// ID заказа, к которому относится письмо. У таких писем заголовок и
    /// текст собираются на лету из живых данных квеста.
    pub quest: Option<String>,
    pub read: bool,
}

impl Letter {
    /// Часть id до двоеточия: у писем путешественников id
    /// вида `traveler_back:0`, а текст лежит под `letter.traveler_back.0.*`.
    fn key(&self) -> String {
        self.id.replace(':', ".")
    }

    /// Заголовок письма на текущем языке.
    pub fn title(&self) -> &'static str {
        i18n::t_or(&format!("letter.{}.title", self.key()), "")
    }

    /// Текст письма на текущем языке.
    pub fn body(&self) -> &'static str {
        i18n::t_or(&format!("letter.{}.body", self.key()), "")
    }

    /// Ждёт ли письмо действия игрока: такие записки не исчезают после
    /// прочтения, а висят в колокольчике, пока дело не закроют.
    ///
    /// `offer` открывает окно постройки, `quest` — карточка заказа, а счёт по
    /// налогу и счёт за работников несут кнопку и итог последнего расчёта.
    pub fn waits_for_the_player(&self) -> bool {
        self.offer.is_some()
            || self.quest.is_some()
            || matches!(self.id.as_str(), "tax_due" | "salary_bill" | "debt")
    }
}

/// Активный поход. Живёт в состоянии игры, поэтому идёт вне вкладки «Походы».
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveExpedition {
    /// Идентификатор локации.
    pub location_id: String,
    /// Сколько секунд уже в пути.
    pub elapsed: u32,
    /// Найдено за поход: (id предмета, количество, на склад).
    pub found: Vec<(String, u32, bool)>,
    /// Последнее сообщение (переполнение склада и т.п.).
    pub notice: Option<String>,
    /// Поход окончен (энергия или рюкзак).
    pub done: bool,
    /// Поход на паузе: кнопка «Стоп» останавливает путь, но не закрывает
    /// поход; находки и время сохраняются до продолжения.
    #[serde(default)]
    pub paused: bool,
}

impl ActiveExpedition {
    pub fn new(location_id: &str) -> Self {
        Self {
            location_id: location_id.to_string(),
            elapsed: 0,
            found: Vec::new(),
            notice: None,
            done: false,
            paused: false,
        }
    }

    /// Плюс находка к списку с склейкой одинаковых.
    pub fn push_found(&mut self, item_id: &str, quantity: u32, active: bool) {
        if let Some(entry) = self
            .found
            .iter_mut()
            .find(|entry| entry.0 == item_id && entry.2 == active)
        {
            entry.1 += quantity;
        } else {
            self.found.push((item_id.to_string(), quantity, active));
        }
    }
}

/// Лучшее предложение от торговцев за текущий предмет (для ажиотажа).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BestOffer {
    pub item_id: String,
    pub trader_index: usize,
    /// Итоговая сумма сделки, а не цена за штуку.
    pub price: f64,
    /// Сколько штук было в сделке: по нему считаем цену за штуку, иначе
    /// партия из десяти предметов «перебивала» бы цену одного.
    #[serde(default)]
    pub quantity: u32,
}

/// Всё состояние игры. Логика не зависит от Dioxus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameState {
    pub version: u32,

    pub energy: f64,
    pub energy_max: f64,

    pub wood: f64,
    pub stone: f64,
    pub iron: f64,
    pub coal: f64,
    /// Золото. Старое сохранение без поля получает нуль, а не стартовое значение.
    #[serde(default)]
    pub gold: f64,

    /// Активные ресурсы (склад): ID вида -> количество.
    pub active_resources: HashMap<String, u32>,
    /// Дробные остатки добычи: 0.5/с должны когда-то стать целым ресурсом.
    #[serde(default)]
    pub resource_fraction: HashMap<String, f64>,

    /// Сколько раз игрок нажимал «Отдых».
    pub rest_clicks: u32,
    /// Уровень своего дома (0 — ещё не построен).
    pub house_level: u32,
    /// Уровень «Уюта»: повышает максимум энергии.
    pub comfort_level: u32,
    /// Уровень склада.
    pub storage_level: u32,
    /// Уровень амбара: каждый уровень даёт +500 к лимиту склада.
    #[serde(default)]
    pub barn_level: u32,
    /// Уровень дома для работников (каждый уровень — один работник).
    pub worker_house_level: u32,
    /// Уровень дома для изучения (каждый уровень — один исследователь).
    pub research_house_level: u32,
    /// Уровень лесопилки (каждый уровень — одно рабочее место).
    pub sawmill_level: u32,
    /// Уровень шахты (каждый уровень — одно рабочее место).
    pub mine_level: u32,
    /// Уровень банка (повышает лимит золота).
    pub bank_level: u32,
    /// Уровень кузнеца (навык). Сбрасывается при перерождении.
    #[serde(default = "default_smith_level")]
    pub smith_level: u32,
    /// Накопленный опыт кузнеца.
    #[serde(default)]
    pub smith_xp: f64,
    /// Открыта ли угольная яма (оплачено письмо). Постройка 0→1 — отдельно.
    #[serde(default)]
    pub coal_pit_unlocked: bool,
    /// Открыта ли кузница (оплачено письмо). Постройка 0→1 — отдельно.
    #[serde(default)]
    pub forge_unlocked: bool,
    /// Открыт ли банк (оплачено письмо). Постройка 0→1 — отдельно.
    #[serde(default)]
    pub bank_unlocked: bool,
    /// Уровень угольной ямы (открывается исследованием).
    pub coal_pit_level: u32,

    /// Изучена ли кузница.
    pub forge_researched: bool,
    /// Накопленный опыт изучения кузницы.
    pub forge_research: f64,
    /// Сколько исследователей занято кузницей.
    pub forge_researchers: u32,
    /// Уровень кузницы (0 — ещё не построена).
    pub forge_level: u32,
    /// Текущий огонь кузницы в секундах.
    pub current_fire: u32,
    /// Автоматически подбрасывать уголь, когда огонь гаснет.
    pub forge_auto_feed: bool,
    /// Накопленное время до следующей автоподкидки угля.
    #[serde(default)]
    pub forge_auto_timer: f64,

    /// Изучен ли торговец.
    pub trader_researched: bool,
    /// Накопленный опыт изучения торговца.
    pub trader_research: f64,
    /// Сколько исследователей занято торговцем.
    pub trader_researchers: u32,

    /// Инвентарь: ID предмета -> количество.
    pub inventory: HashMap<String, u32>,
    /// Гардероб: отдельное хранилище для экипировки. В рюкзаке лежит только
    /// то, что игрок может продать/надеть; переключаемые наборы живут здесь.
    #[serde(default)]
    pub wardrobe: HashMap<String, u32>,
    /// Надетая экипировка активного набора.
    pub equipment: EquippedItems,
    /// Сохранённые наборы экипировки (Set 1–4).
    pub equipment_sets: [EquippedItems; 4],
    /// Индекс активного набора экипировки.
    pub active_set: usize,
    /// Бонус к цене продажи от экипировки.
    pub trade_bonus: f32,
    /// Бонус к добыче в походах от экипировки (задел на будущее).
    pub expedition_bonus: f32,
    /// +% ресурсов от экипировки.
    pub gather_bonus: f32,
    /// +% к опыту кузнеца от экипировки.
    #[serde(default = "default_smith_xp_bonus")]
    pub smith_xp_bonus: f32,
    /// +% к шансу торга от экипировки.
    pub haggle_bonus: f32,
    /// % шанс не потратить ингредиент при крафте.
    pub ingredient_discount: f32,
    /// −% расход огня кузницы от экипировки.
    pub fire_discount: f32,
    /// −% расход энергии в походах от экипировки.
    pub expedition_energy_discount: f32,
    /// Рынки всех торговцев.
    pub traders: Vec<TraderMarket>,
    /// Лучшее предложение от торговцев за текущий предмет (для ажиотажа).
    #[serde(default)]
    pub best_offer: Option<BestOffer>,
    /// Доска объявлений: 6 квестов.
    #[serde(default)]
    pub quests: Vec<Quest>,
    /// Момент следующего обновления доски (в `total_play_seconds`).
    #[serde(default)]
    pub quest_board_refresh_at: f64,
    /// Текущий долг (в золоте). Копится при провале квестов, растёт
    /// на +40% в минуту, максимум 5000.
    #[serde(default)]
    pub debt: f64,
    /// Момент следующего роста долга (в `total_play_seconds`).
    #[serde(default)]
    pub debt_tick_at: f64,
    /// Момент следующего визита коллекторов (в `total_play_seconds`).
    #[serde(default)]
    pub collector_tick_at: f64,
    /// Игровое время для квестов, доски и долга. Идёт только в реальных
    /// тиках (1 c/с), не ускоряется тайм-варпом.
    #[serde(default)]
    pub quest_clock: f64,
    /// Игровое время для событий (баффы, сюжетка). Не ускоряется тайм-варпом.
    #[serde(default)]
    pub event_clock: f64,
    /// Накопленное время до следующего шага рынка.
    pub market_timer: f64,
    /// Детерминированное зерно рынка.
    pub market_seed: u64,
    /// Накопленная дробная секунда пути в походе.
    pub expedition_timer: f64,
    /// Сколько секунд похода уже покрыто свечой.
    #[serde(default)]
    pub expedition_candle_timer: u32,
    /// Активный поход: `None` — игрок не в пути. Хранится в состоянии, чтобы
    /// поход шёл и при закрытой вкладке.
    pub active_expedition: Option<ActiveExpedition>,
    /// Открытые картами локации походов (кроме стартовых поля и оврага).
    pub unlocked_locations: Vec<String>,

    /// Предложение постройки, которое сейчас показано в центре экрана.
    #[serde(default)]
    pub pending_offer: Option<String>,
    /// Чертёж паровой машины изучен — в кузнице открыт рецепт прототипа.
    #[serde(default)]
    pub steam_blueprint_studied: bool,
    /// Прототип паровой машины собран.
    #[serde(default)]
    pub steam_prototype_built: bool,
    /// Модальное окно «Прототип собран» ещё не закрыто.
    #[serde(default)]
    pub steam_prototype_event: bool,
    /// Игрок вступил в Эпоху Пара.
    #[serde(default)]
    pub steam_era: bool,
    /// Чертёж компаса изучен — в кузнице открыт рецепт компаса.
    #[serde(default)]
    pub compass_blueprint_studied: bool,
    /// Город: лавка, банк, гильдия, события и налоги.
    #[serde(default)]
    pub city: CityState,
    /// Письма в колокольчике.
    #[serde(default)]
    pub letters: Vec<Letter>,
    /// Момент (в `total_play_seconds`), когда выплатили/насчитали зарплаты.
    /// С него считается следующее начисление вместе с процентом и разбивкой.
    #[serde(default)]
    pub salary_log_at: f64,
    /// Где лежит последний счёт по зарплатам, чтобы не класть его каждый тик.
    #[serde(default)]
    pub salary_bill_stage: SalaryBillStage,
    /// Банк уже предложен игроку (порог 800 золота).
    #[serde(default)]
    pub bank_seen: bool,

    // ---------- Динамические события ----------
    /// Событие, которое сейчас на экране (обмен, бафф, кризис).
    #[serde(default)]
    pub pending_event: Option<PendingEvent>,
    /// Очередь оффлайн-событий. Не конкурирует с фоновыми: пока непусто —
    /// фоновый тик в `events::tick` не спавнит новое.
    #[serde(default)]
    pub offline_events: Vec<PendingEvent>,
    /// Момент следующего фонового события в базе `total_play_seconds`.
    #[serde(default)]
    pub next_baseline_at: f64,
    /// Отложенные action-based триггеры (см. `events::schedule`).
    #[serde(default)]
    pub action_queue: Vec<ActionTrigger>,
    /// Активные баффы и дебаффы. Протухшие вычищаются в `events::tick`.
    #[serde(default)]
    pub buffs: Vec<BuffInstance>,

    /// Сколько циклов (перерождений) пройдено.
    pub cycles: u32,
    /// Накопленный множитель дохода: растёт от перерождения к перерождению.
    #[serde(default = "default_earned_income")]
    pub earned_income: f64,
    /// Максимальный уровень кузнеца за всю игру (рекорд, не сбрасывается).
    #[serde(default)]
    pub max_smith_level_ever: u32,
    /// Максимальное превышение порога за всю игру: (gathered - threshold) / threshold.
    #[serde(default)]
    pub max_overgather_ever: f64,
    /// Максимальное время в цикле за всю игру (часы).
    #[serde(default)]
    pub max_hours_ever: f64,
    /// Момент старта текущего цикла (в total_play_seconds).
    #[serde(default)]
    pub cycle_started_at: f64,
    /// Сколько секунд длился прошлый цикл.
    #[serde(default)]
    pub last_cycle_time: f64,
    /// Дерево, добытое за текущий цикл (для порога перерождения).
    #[serde(default)]
    pub cycle_wood: f64,
    /// Камень, добытый за текущий цикл.
    #[serde(default)]
    pub cycle_stone: f64,
    /// Ресурс «время» за оффлайн-прогресс.
    pub time: f64,
    /// Общее время, проведённое в игре (секунды). Копится в тике и не
    /// сбрасывается ни перерождением, ни сбросом прогресса.
    #[serde(default)]
    pub total_play_seconds: f64,
    /// Время последнего сохранения (Unix-секунды) для оффлайн-прогресса.
    pub last_seen: u64,
    /// Статистика за всё время.
    pub stats: Stats,

    /// Сколько работников назначено на лесопилку.
    pub sawmill_workers: u32,
    /// Сколько работников назначено на шахту.
    pub mine_workers: u32,
    /// Сколько работников назначено на угольную яму.
    pub coal_pit_workers: u32,
    /// Сколько исследователей занято изучением угольной ямы.
    pub coal_pit_researchers: u32,

    /// Изучена ли угольная яма.
    pub coal_pit_researched: bool,
    /// Накопленный опыт изучения угольной ямы.
    pub coal_pit_research: f64,
    /// Прогресс текущей партии угольной ямы от 0.0 до 1.0.
    pub coal_pit_progress: f64,

    /// Значок дерева уже показывался в баре.
    pub wood_seen: bool,
    /// Открыты ли камень, дом и склад.
    pub stone_seen: bool,
    /// Показывать ли уголь в баре ресурсов.
    pub coal_seen: bool,

    pub theme: Theme,

    /// Временные настройки разработчика. В сохранение не попадают.
    #[serde(skip)]
    pub dev: DevSettings,
}

fn default_smith_level() -> u32 {
    1
}

fn default_smith_xp_bonus() -> f32 {
    1.0
}

fn default_earned_income() -> f64 {
    1.0
}

impl Default for GameState {
    fn default() -> Self {
        // Новая игра начинается с пустой сумкой.
        let inventory: HashMap<String, u32> = HashMap::new();

        let mut state = Self {
            version: balance::SAVE_VERSION,
            energy: balance::START_ENERGY,
            energy_max: balance::START_ENERGY_MAX,
            wood: 0.0,
            stone: 0.0,
            iron: 0.0,
            coal: 0.0,
            gold: balance::START_GOLD,
            active_resources: {
                let mut map = HashMap::new();
                map.insert("wood".to_string(), 0);
                map.insert("stone".to_string(), 0);
                map.insert("iron".to_string(), 0);
                map.insert("coal".to_string(), 0);
                map
            },
            resource_fraction: HashMap::new(),
            rest_clicks: 0,
            house_level: 0,
            comfort_level: 0,
            storage_level: 0,
            barn_level: 0,
            worker_house_level: 0,
            research_house_level: 0,
            sawmill_level: 0,
            mine_level: 0,
            bank_level: 0,
            smith_level: 1,
            smith_xp: 0.0,
            coal_pit_unlocked: false,
            forge_unlocked: false,
            bank_unlocked: false,
            coal_pit_level: 0,
            forge_researched: false,
            forge_research: 0.0,
            forge_researchers: 0,
            forge_level: 0,
            current_fire: 0,
            forge_auto_feed: false,
            forge_auto_timer: 0.0,
            trader_researched: false,
            trader_research: 0.0,
            trader_researchers: 0,
            inventory,
            wardrobe: HashMap::new(),
            equipment: EquippedItems::default(),
            equipment_sets: std::array::from_fn(|_| EquippedItems::default()),
            active_set: 0,
            trade_bonus: 0.0,
            expedition_bonus: 0.0,
            gather_bonus: 0.0,
            smith_xp_bonus: 1.0,
            haggle_bonus: 0.0,
            ingredient_discount: 0.0,
            fire_discount: 0.0,
            expedition_energy_discount: 0.0,
            traders: trader::default_markets(),
            best_offer: None,
            quests: Vec::new(),
            quest_board_refresh_at: 0.0,
            debt: 0.0,
            debt_tick_at: 0.0,
            collector_tick_at: 0.0,
            quest_clock: 0.0,
            event_clock: 0.0,
            market_timer: 0.0,
            market_seed: 0x9e37_79b9_7f4a_7c15,
            expedition_timer: 0.0,
            expedition_candle_timer: 0,
            active_expedition: None,
            unlocked_locations: Vec::new(),
            pending_offer: None,
            steam_blueprint_studied: false,
            steam_prototype_built: false,
            steam_prototype_event: false,
            steam_era: false,
            compass_blueprint_studied: false,
            city: CityState::default(),
            letters: Vec::new(),
            salary_log_at: 0.0,
            salary_bill_stage: SalaryBillStage::None,
            bank_seen: false,
            pending_event: None,
            offline_events: Vec::new(),
            // Первое фоновое событие — через 5 минут игры. `total_play_seconds`
            // в новой игре равен 0, поэтому константа работает как задержка.
            next_baseline_at: balance::EVENTS_BASELINE_FIRST_SECS,
            action_queue: Vec::new(),
            buffs: Vec::new(),
            cycles: 0,
            earned_income: 1.0,
            max_smith_level_ever: 1,
            max_overgather_ever: 0.0,
            max_hours_ever: 0.0,
            cycle_started_at: 0.0,
            last_cycle_time: 0.0,
            cycle_wood: 0.0,
            cycle_stone: 0.0,
            time: 0.0,
            total_play_seconds: 0.0,
            last_seen: 0,
            stats: Stats::default(),
            sawmill_workers: 0,
            mine_workers: 0,
            coal_pit_workers: 0,
            coal_pit_researchers: 0,
            coal_pit_researched: false,
            coal_pit_research: 0.0,
            coal_pit_progress: 0.0,
            wood_seen: false,
            stone_seen: false,
            coal_seen: false,
            theme: Theme::Classic,
            dev: DevSettings::default(),
        };
        state.recalculate_stats();
        state
    }
}

impl GameState {
    /// Ключ вида ресурса в карте активных складов.
    fn resource_key(kind: ResourceKind) -> &'static str {
        match kind {
            ResourceKind::Wood => "wood",
            ResourceKind::Stone => "stone",
            ResourceKind::Coal => "coal",
            ResourceKind::Fire => "fire",
            ResourceKind::Gold => "gold",
        }
    }

    pub fn amount(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Wood => self.active_amount("wood") as f64,
            ResourceKind::Stone => self.active_amount("stone") as f64,
            ResourceKind::Coal => self.active_amount("coal") as f64,
            ResourceKind::Fire => self.current_fire as f64,
            ResourceKind::Gold => self.gold,
        }
    }

    /// Лимит ресурса с учётом склада. Для огня — запас кузницы, для золота — банк.
    pub fn cap(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Fire => self.fire_max() as f64,
            ResourceKind::Gold => self.gold_cap(),
            _ => {
                balance::BASE_STORAGE_CAP
                    + self.storage_level as f64 * balance::STORAGE_CAP_PER_LEVEL
                    + self.barn_level as f64 * balance::BARN_CAP_PER_LEVEL
            }
        }
    }

    /// Лимит золота с учётом банка.
    pub fn gold_cap(&self) -> f64 {
        balance::BASE_GOLD_CAP
            + self.bank_level as f64 * balance::GOLD_CAP_PER_LEVEL
            + self.city.bank_cells as f64 * balance::BANK_CELL_BONUS
    }

    /// Регенерация энергии в секунду. Без дома — ноль, если нет баффа.
    /// Бафф «Хорошее настроение» (`EnergyRegenFlat`) даёт плоскую прибавку
    /// и не умножается на X.
    pub fn energy_regen(&self) -> f64 {
        let base =
            self.house_level as f64 * balance::HOUSE_ENERGY_PER_LEVEL * self.income_multiplier()
                + self.dev.energy_regen;
        (base + events::active_buff_sum(self, events::BuffStat::EnergyRegenFlat)).max(0.0)
    }

    /// Множитель стоимости энергии от дебаффа «Кривые руки».
    ///
    /// Дефолт — 1.0 (без изменений). Применяется к клику добычи и к расходу
    /// в походе. К `rest_energy` не применяется: отдых — не трата.
    pub fn energy_cost_multiplier(&self) -> f64 {
        1.0 + events::active_buff_sum(self, events::BuffStat::EnergyCostMult) / 100.0
    }

    /// Сколько энергии даёт одно нажатие «Отдых». Растёт вместе с X.
    pub fn rest_energy(&self) -> f64 {
        (balance::REST_ENERGY
            + self.house_level as f64 * balance::REST_ENERGY_PER_HOUSE_LEVEL
            + self.dev.rest_bonus)
            * self.income_multiplier()
    }

    /// Сколько работников дают дома для работников.
    pub fn worker_slots(&self) -> u32 {
        self.worker_house_level * balance::WORKERS_PER_HOUSE_LEVEL
    }

    /// Сколько исследователей дают дома для изучения.
    pub fn researcher_slots(&self) -> u32 {
        self.research_house_level * balance::RESEARCHERS_PER_HOUSE_LEVEL
    }

    /// Сколько работников уже назначено.
    pub fn workers_assigned(&self) -> u32 {
        self.sawmill_workers + self.mine_workers + self.coal_pit_workers
    }

    /// Сколько работников свободно.
    pub fn workers_free(&self) -> u32 {
        self.worker_slots().saturating_sub(self.workers_assigned())
    }

    /// Сколько исследователей занято.
    pub fn researchers_assigned(&self) -> u32 {
        self.coal_pit_researchers + self.forge_researchers + self.trader_researchers
    }

    /// Сколько исследователей свободно.
    pub fn researchers_free(&self) -> u32 {
        self.researcher_slots()
            .saturating_sub(self.researchers_assigned())
    }

    /// Сколько рабочих мест даёт лесопилка.
    pub fn sawmill_slots(&self) -> u32 {
        self.sawmill_level
    }

    /// Сколько рабочих мест даёт шахта.
    pub fn mine_slots(&self) -> u32 {
        self.mine_level
    }

    /// Сколько рабочих мест даёт угольная яма.
    pub fn coal_pit_slots(&self) -> u32 {
        self.coal_pit_level
    }

    /// Дерево в секунду с лесопилки. Без множителя цикла: он применяется
    /// один раз в `actions::tick`, чтобы не было двойного умножения.
    /// Дебафф «Комары» (`SawmillMult`) режет выработку, но не ниже нуля.
    pub fn sawmill_wood_per_sec(&self) -> f64 {
        let base = self.sawmill_workers as f64
            * balance::SAWMILL_WOOD_PER_WORKER_PER_SEC
            * (1.0 + self.gather_bonus as f64);
        let buff = 1.0 + events::active_buff_sum(self, events::BuffStat::SawmillMult) / 100.0;
        (base * buff).max(0.0)
    }

    /// Камень в секунду с шахты (железо теперь только в походах и у торговца).
    /// Множитель цикла, как и у лесопилки, применяется в тике.
    pub fn mine_stone_per_sec(&self) -> f64 {
        let base = self.mine_workers as f64
            * balance::MINE_STONE_PER_WORKER_PER_SEC
            * (1.0 + self.gather_bonus as f64);
        let buff = 1.0 + events::active_buff_sum(self, events::BuffStat::MineMult) / 100.0;
        (base * buff).max(0.0)
    }

    /// Уголь в секунду с угольной ямы.
    pub fn coal_pit_coal_per_sec(&self) -> f64 {
        self.coal_pit_workers as f64 * balance::COAL_PIT_COAL_PER_BATCH
            / balance::COAL_PIT_BATCH_SECONDS
    }

    /// Дерево в секунду, которое тратит угольная яма.
    pub fn coal_pit_wood_per_sec(&self) -> f64 {
        self.coal_pit_workers as f64 * balance::COAL_PIT_WOOD_PER_BATCH
            / balance::COAL_PIT_BATCH_SECONDS
    }

    /// Дерево в секунду с наёмных на лесопилке (в разы больше общинных).
    pub fn hired_sawmill_wood_per_sec(&self) -> f64 {
        let base = self.city.hired_sawmill as f64
            * balance::HIRE_YIELD_MULT
            * balance::SAWMILL_WOOD_PER_WORKER_PER_SEC
            * (1.0 + self.gather_bonus as f64);
        let buff = 1.0 + events::active_buff_sum(self, events::BuffStat::SawmillMult) / 100.0;
        (base * buff).max(0.0)
    }

    /// Камень в секунду с наёмных на шахте.
    pub fn hired_mine_stone_per_sec(&self) -> f64 {
        let base = self.city.hired_mine as f64
            * balance::HIRE_YIELD_MULT
            * balance::MINE_STONE_PER_WORKER_PER_SEC
            * (1.0 + self.gather_bonus as f64);
        let buff = 1.0 + events::active_buff_sum(self, events::BuffStat::MineMult) / 100.0;
        (base * buff).max(0.0)
    }

    /// Сколько «общинных» рабочих заменяют наёмные на угольной яме.
    pub fn hired_coal_workers(&self) -> f64 {
        self.city.hired_coal as f64 * balance::HIRE_YIELD_MULT
    }

    /// Доступно ли направление изучения.
    pub fn research_available(&self, kind: ResearchKind) -> bool {
        match kind {
            ResearchKind::CoalPit => true,
            ResearchKind::Forge => self.coal_pit_researched,
            ResearchKind::Bank => self.forge_researched,
        }
    }

    /// Изучено ли направление.
    pub fn research_done(&self, kind: ResearchKind) -> bool {
        match kind {
            ResearchKind::CoalPit => self.coal_pit_researched,
            ResearchKind::Forge => self.forge_researched,
            ResearchKind::Bank => self.trader_researched,
        }
    }

    /// Сколько опыта нужно для изучения.
    pub fn research_cost(&self, kind: ResearchKind) -> f64 {
        match kind {
            ResearchKind::CoalPit => balance::COAL_PIT_RESEARCH_COST,
            ResearchKind::Forge => balance::FORGE_RESEARCH_COST,
            ResearchKind::Bank => balance::BANK_RESEARCH_COST,
        }
    }

    /// Накопленный опыт изучения.
    pub fn research_progress(&self, kind: ResearchKind) -> f64 {
        match kind {
            ResearchKind::CoalPit => self.coal_pit_research,
            ResearchKind::Forge => self.forge_research,
            ResearchKind::Bank => self.trader_research,
        }
    }

    /// Записывает накопленный опыт изучения.
    pub fn set_research_progress(&mut self, kind: ResearchKind, value: f64) {
        match kind {
            ResearchKind::CoalPit => self.coal_pit_research = value,
            ResearchKind::Forge => self.forge_research = value,
            ResearchKind::Bank => self.trader_research = value,
        }
    }

    /// Сколько исследователей назначено на направление.
    pub fn researchers_for(&self, kind: ResearchKind) -> u32 {
        match kind {
            ResearchKind::CoalPit => self.coal_pit_researchers,
            ResearchKind::Forge => self.forge_researchers,
            ResearchKind::Bank => self.trader_researchers,
        }
    }

    /// Назначает исследователей на направление.
    pub fn set_researchers(&mut self, kind: ResearchKind, value: u32) {
        match kind {
            ResearchKind::CoalPit => self.coal_pit_researchers = value,
            ResearchKind::Forge => self.forge_researchers = value,
            ResearchKind::Bank => self.trader_researchers = value,
        }
    }

    /// Опыт изучения в секунду с назначенных исследователей.
    pub fn research_speed(&self, kind: ResearchKind) -> f64 {
        self.researchers_for(kind) as f64 * balance::RESEARCH_PER_RESEARCHER_PER_SEC
    }

    /// Доля изучения от 0.0 до 1.0.
    /// Доля изучения (легаси, UI удалён).
    #[allow(dead_code)]
    pub fn research_ratio(&self, kind: ResearchKind) -> f64 {
        let cost = self.research_cost(kind);
        if cost <= 0.0 {
            1.0
        } else {
            (self.research_progress(kind) / cost).clamp(0.0, 1.0)
        }
    }

    /// Завершает изучение и открывает соответствующую систему.
    pub fn complete_research(&mut self, kind: ResearchKind) {
        match kind {
            ResearchKind::CoalPit => {
                self.coal_pit_researched = true;
                self.coal_pit_level = self.coal_pit_level.max(1);
                self.coal_seen = true;
                self.coal_pit_researchers = 0;
            }
            ResearchKind::Forge => {
                self.forge_researched = true;
                self.forge_researchers = 0;
            }
            ResearchKind::Bank => {
                self.trader_researched = true;
                self.trader_researchers = 0;
            }
        }
    }

    /// Максимум энергии с учётом уюта и экипировки.
    /// Множитель цикла сюда не входит: X действует на «Отдых» и «Труд»,
    /// но не на запас энергии.
    pub fn max_energy(&self) -> f64 {
        let equipment_bonus: u32 = EquipSlot::ALL
            .iter()
            .filter_map(|slot| self.equipped_item(*slot))
            .filter_map(|item| item.stats.max_energy)
            .sum();
        let base = self.energy_max + self.comfort_level as f64 * balance::ENERGY_CAP_PER_COMFORT;
        base + equipment_bonus as f64
    }

    /// Текущий уровень кузнеца (гарантированно не меньше 1).
    pub fn smith_level(&self) -> u32 {
        self.smith_level.max(1)
    }

    /// Сколько опыта нужно до следующего уровня.
    pub fn smith_xp_to_next(&self) -> f64 {
        balance::smith_xp_for_level(self.smith_level())
    }

    /// Прогресс до следующего уровня (от 0.0 до 1.0) — для полоски в UI.
    pub fn smith_xp_ratio(&self) -> f64 {
        let need = self.smith_xp_to_next();
        if need.is_finite() && need > 0.0 {
            (self.smith_xp / need).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Начисляет опыт кузнеца. Если уровень повысился, возвращает новый уровень.
    pub fn add_smith_xp(&mut self, xp: f64) -> Option<u32> {
        if xp <= 0.0 {
            return None;
        }
        self.smith_level = self.smith_level.max(1);
        self.smith_xp += xp;
        let mut new_level = None;

        // Потолка нет: качаемся, пока хватает XP. `smith_xp_for_level` растёт
        // с уровнем, поэтому цикл конечен.
        loop {
            let needed = balance::smith_xp_for_level(self.smith_level);
            if !needed.is_finite() || needed <= 0.0 {
                break;
            }
            if self.smith_xp + 1e-9 >= needed {
                self.smith_xp -= needed;
                self.smith_level += 1;
                new_level = Some(self.smith_level);
            } else {
                break;
            }
        }
        new_level
    }

    /// Максимальный запас огня кузницы с учётом уровня.
    pub fn fire_max(&self) -> u32 {
        let levels = self.forge_level.saturating_sub(1);
        balance::FORGE_FIRE_MAX_SECONDS + levels * balance::FORGE_FIRE_PER_LEVEL
    }

    /// Сколько секунд огня даёт один уголь: растёт с уровнем кузницы.
    pub fn fire_per_coal(&self) -> u32 {
        let levels = self.forge_level.saturating_sub(1);
        balance::FIRE_SECONDS_PER_COAL + levels * balance::FIRE_PER_COAL_PER_LEVEL
    }

    /// Активный ресурс «X» из склада.
    pub fn active_amount(&self, kind: &str) -> u32 {
        self.active_resources.get(kind).copied().unwrap_or(0)
    }

    /// Сколько активных квестов у игрока.
    pub fn active_quests_count(&self) -> u32 {
        self.quests
            .iter()
            .filter(|q| q.status == super::quests::QuestStatus::Active)
            .count() as u32
    }

    /// Множитель цены продажи: −25% пока есть долг.
    pub fn debt_sell_multiplier(&self) -> f64 {
        if self.debt > 0.0 {
            0.75
        } else {
            1.0
        }
    }

    /// Можно ли торговаться (долг блокирует торг).
    pub fn can_haggle(&self) -> bool {
        self.debt <= 0.0
    }

    /// Сколько раундов торга доступно: 3 базовых плюс по одному за каждое
    /// надетое кольцо, но не больше 6.
    pub fn haggle_rounds(&self) -> u32 {
        let rings = EquipSlot::ALL
            .iter()
            .filter_map(|slot| self.equipped_item(*slot))
            .filter(|item| item.id.ends_with("_ring"))
            .count() as u32;
        (3 + rings).min(6)
    }

    /// Сколько единиц предмета у игрока: активный склад (wood/stone/coal)
    /// или рюкзак (всё остальное, включая руду).
    /// Одна точка правды: события и торговые проверки не должны знать,
    /// где именно лежит предмет.
    pub fn amount_of(&self, item_id: &str) -> u32 {
        if is_active_resource(item_id) {
            self.active_amount(item_id)
        } else {
            self.item_count(item_id)
        }
    }

    /// Лимит активного ресурса.
    pub fn active_cap(&self, _kind: &str) -> u32 {
        (balance::BASE_STORAGE_CAP
            + self.storage_level as f64 * balance::STORAGE_CAP_PER_LEVEL
            + self.barn_level as f64 * balance::BARN_CAP_PER_LEVEL)
            .round() as u32
    }

    /// Открыт ли амбар.
    pub fn barn_unlocked(&self) -> bool {
        self.storage_level >= balance::BARN_UNLOCK_STORAGE_LEVEL || self.barn_level > 0
    }

    /// Есть ли непрочитанные письма.
    pub fn unread_letters(&self) -> u32 {
        self.letters.iter().filter(|letter| !letter.read).count() as u32
    }

    /// Кладёт письмо в колокольчик. Повторное письмо с тем же id не дублируется,
    /// но снова становится непрочитанным.
    pub fn add_letter(&mut self, id: &str, offer: Option<&str>) {
        if let Some(letter) = self.letters.iter_mut().find(|letter| letter.id == id) {
            // Текст-легаси из старого сохранения больше не нужен.
            letter.title.clear();
            letter.body.clear();
            letter.offer = offer.map(str::to_string);
            letter.quest = None;
            letter.read = false;
            return;
        }
        self.letters.push(Letter {
            id: id.to_string(),
            title: String::new(),
            body: String::new(),
            offer: offer.map(str::to_string),
            quest: None,
            read: false,
        });
    }

    /// Кладёт в колокольчик письмо о взятом заказе: в нём детали и кнопки
    /// «Сдать» / «Отклонить». У каждого заказа своё письмо.
    pub fn add_quest_letter(&mut self, quest_id: &str) {
        let id = format!("quest:{quest_id}");
        if let Some(letter) = self.letters.iter_mut().find(|letter| letter.id == id) {
            letter.quest = Some(quest_id.to_string());
            letter.read = false;
            return;
        }
        self.letters.push(Letter {
            id,
            title: String::new(),
            body: String::new(),
            offer: None,
            quest: Some(quest_id.to_string()),
            read: false,
        });
    }

    /// Убирает письмо о заказе, когда он сдан, отклонён или просрочен.
    pub fn remove_quest_letter(&mut self, quest_id: &str) {
        self.letters
            .retain(|letter| letter.quest.as_deref() != Some(quest_id));
    }

    /// Открывает письмо: помечает прочитанным, снова показывает предложение
    /// и убирает записку из колокольчика.
    ///
    /// В почте остаются только письма, которые ещё чего-то ждут от игрока:
    /// счёт по налогу, счёт за работников, долг и карточки заказов. Остальные
    /// (`город открыт`, `скоро налог`, `налог уплачен`, возвращения
    /// путешественников и т.п.) — разовые вести: прочитал и убрал.
    pub fn open_letter(&mut self, letter_id: &str) {
        let mut offer = None;
        let mut keeps = false;
        if let Some(letter) = self.letters.iter_mut().find(|l| l.id == letter_id) {
            letter.read = true;
            offer = letter.offer.clone();
            keeps = letter.waits_for_the_player();
        }
        if offer.is_some() {
            self.pending_offer = offer;
        }
        if !keeps {
            self.letters.retain(|letter| letter.id != letter_id);
        }
    }

    /// Убирает письмо из колокольчика по id.
    pub fn dismiss_letter(&mut self, letter_id: &str) {
        self.letters.retain(|letter| letter.id != letter_id);
    }

    /// Добавляет активный ресурс с проверкой лимита. Возвращает добавленное.
    pub fn add_active(&mut self, kind: &str, amount: u32) -> u32 {
        let room = self
            .active_cap(kind)
            .saturating_sub(self.active_amount(kind));
        let added = room.min(amount);
        *self.active_resources.entry(kind.to_string()).or_insert(0) += added;
        added
    }

    /// Списывает активный ресурс. `false`, если не хватает.
    pub fn spend_active(&mut self, kind: &str, amount: u32) -> bool {
        if self.active_amount(kind) < amount {
            return false;
        }
        let current = self.active_resources.get_mut(kind).unwrap();
        *current -= amount;
        if *current == 0 {
            self.active_resources.remove(kind);
        }
        true
    }

    /// Множитель дохода: накопленная прибавка от перерождений плюс бонус
    /// Эпохи Пара. Между перерождениями не меняется.
    pub fn income_multiplier(&self) -> f64 {
        let era = if self.steam_era {
            balance::STEAM_ERA_INCOME_BONUS
        } else {
            0.0
        };
        self.earned_income + era
    }

    /// Составляющие прибавки к X от текущего цикла: база, кузнец,
    /// добыча сверх порога и время в цикле.
    pub fn cycle_delta_parts(&self) -> (f64, f64, f64, f64) {
        // Уровень 1 — базовый, поэтому бонус считается с уровня выше первого.
        let smith = 0.03 * self.smith_level().saturating_sub(1) as f64;
        let overgather = 0.15 * self.current_overgather().max(0.0).sqrt();
        let hours = 0.03 * self.current_cycle_hours();
        (0.08, smith, overgather, hours)
    }

    /// Прибавка к X, которую даст текущий цикл при перерождении.
    pub fn cycle_delta(&self) -> f64 {
        let (base, smith, overgather, hours) = self.cycle_delta_parts();
        base + smith + overgather + hours
    }

    /// Сколько будет X, если переродиться прямо сейчас.
    pub fn preview_income_multiplier(&self) -> f64 {
        self.income_multiplier() + self.cycle_delta()
    }

    /// Текущее превышение порога: (добыто - порог) / порог. Может быть
    /// отрицательным (не добрал); X берёт корень только из положительной части.
    pub fn current_overgather(&self) -> f64 {
        let threshold = self.cycle_threshold();
        if threshold <= 0.0 {
            0.0
        } else {
            (self.cycle_gathered() - threshold) / threshold
        }
    }

    /// Сколько часов длится текущий цикл.
    pub fn current_cycle_hours(&self) -> f64 {
        (self.total_play_seconds - self.cycle_started_at).max(0.0) / 3600.0
    }

    /// Порог перерождения: база × (1 + 0.5 × √cycles).
    /// Сначала крутой рост (первые циклы тяжёлые), потом пологий —
    /// X успевает обогнать порог к ~40-му циклу.
    pub fn cycle_threshold(&self) -> f64 {
        balance::CYCLE_THRESHOLD_BASE * (1.0 + 0.5 * (self.cycles as f64).sqrt())
    }

    /// Сколько дерева и камня добыто за текущий цикл.
    pub fn cycle_gathered(&self) -> f64 {
        self.cycle_wood + self.cycle_stone
    }

    /// Прогресс порога перерождения от 0.0 до 1.0.
    pub fn cycle_progress(&self) -> f64 {
        let threshold = self.cycle_threshold();
        if threshold <= 0.0 {
            1.0
        } else {
            (self.cycle_gathered() / threshold).clamp(0.0, 1.0)
        }
    }

    /// Стена R = 5: шахта должна быть построена до нужного уровня.
    pub fn cycle_wall_mine_met(&self) -> bool {
        self.cycles < balance::CYCLE_WALL_LEVEL || self.mine_level >= balance::CYCLE_WALL_MINE_LEVEL
    }

    /// Стена R = 10: за цикл нужно добыть камень сверх порога.
    pub fn cycle_wall_stone_met(&self) -> bool {
        self.cycles < balance::CYCLE_WALL_STONE_LEVEL
            || self.cycle_stone + 1e-9 >= balance::CYCLE_WALL_EXTRA_STONE
    }

    /// Сколько предметов данного вида в сумке.
    pub fn item_count(&self, item_id: &str) -> u32 {
        self.inventory.get(item_id).copied().unwrap_or(0)
    }

    /// Стоимость ингредиента: активный ресурс склада, огонь кузницы или предмет.
    pub fn ingredient_amount(&self, key: &str) -> u32 {
        if key == "fire" {
            self.current_fire
        } else if is_active_resource(key) {
            self.active_amount(key)
        } else {
            self.item_count(key)
        }
    }

    /// Списывает ингредиент: активный ресурс склада, огонь кузницы или предмет.
    pub fn spend_ingredient(&mut self, key: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        if key == "fire" {
            if self.current_fire < quantity {
                return false;
            }
            self.current_fire -= quantity;
            return true;
        }
        if is_active_resource(key) {
            return self.spend_active(key, quantity);
        }
        self.remove_item(key, quantity)
    }

    /// Сколько ячеек занято. Каждый вид предмета — одна ячейка, сколько бы
    /// штук в ней ни лежало.
    pub fn inventory_used_slots(&self) -> u32 {
        self.inventory
            .iter()
            .filter(|(_, quantity)| **quantity > 0)
            .count() as u32
    }

    /// Всего ячеек с учётом экипировки.
    pub fn inventory_slots(&self) -> u32 {
        let bonus: u32 = EquipSlot::ALL
            .iter()
            .filter_map(|slot| self.equipped_item(*slot))
            .filter_map(|item| item.stats.inventory_slots)
            .sum();
        items::BASE_INVENTORY_SLOTS + bonus
    }

    /// Свободные ячейки.
    pub fn inventory_free_slots(&self) -> u32 {
        self.inventory_slots()
            .saturating_sub(self.inventory_used_slots())
    }

    /// Свободные ячейки с учётом того, что ингредиенты рецепта
    /// освободят свои ячейки после списания. Нужно для проверки «хватит ли
    /// места» в `affordable_crafts`: игрок, у которого рюкзак забит только
    /// ингредиентами, должен всё равно иметь возможность крафтить.
    pub fn inventory_free_slots_with_freed(&self, recipe: &items::Recipe) -> u32 {
        let free = self.inventory_free_slots();
        // Огонь и активные ресурсы склада (wood/stone/coal) в инвентаре
        // не лежат — списание их не освобождает ячейку.
        let mut freed = 0;
        for ingredient in &recipe.ingredients {
            if ingredient.item_id == "fire" || is_active_resource(ingredient.item_id) {
                continue;
            }
            // Предмет не должен совпадать с результатом: если это тот же id,
            // что и результат, ячейка не исчезнет — количество в ней просто
            // пересчитается.
            if ingredient.item_id == recipe.result_id {
                continue;
            }
            let have = self.item_count(ingredient.item_id);
            if have > 0 && have <= ingredient.quantity {
                freed += 1;
            }
        }
        free + freed
    }

    /// Кладёт предмет экипировки в гардероб. Только экипировку — ресурсы
    /// и расходники туда не попадают.
    pub fn add_to_wardrobe(&mut self, item_id: &str, quantity: u32) {
        if quantity == 0 {
            return;
        }
        if items::item(item_id).and_then(|i| i.slot).is_none() {
            return;
        }
        *self.wardrobe.entry(item_id.to_string()).or_insert(0) += quantity;
    }

    /// Забирает предмет из гардероба.
    pub fn remove_from_wardrobe(&mut self, item_id: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        let remaining = {
            let Some(current) = self.wardrobe.get_mut(item_id) else {
                return false;
            };
            if *current < quantity {
                return false;
            }
            *current -= quantity;
            *current
        };
        if remaining == 0 {
            self.wardrobe.remove(item_id);
        }
        true
    }

    /// Сколько предметов лежит в гардеробе.
    pub fn wardrobe_count(&self, item_id: &str) -> u32 {
        self.wardrobe.get(item_id).copied().unwrap_or(0)
    }

    /// Поместится ли предмет в сумку: не больше лимита на ячейку и есть куда класть.
    /// Если такой предмет уже лежит, свободная ячейка не нужна — он докладывается
    /// в существующий стак.
    pub fn can_add_item(&self, item_id: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        if items::item(item_id).is_none() {
            return false;
        }
        self.item_capacity_left(item_id) >= quantity
            && (self.item_count(item_id) > 0 || self.inventory_free_slots() > 0)
    }

    /// Поместится ли результат крафта. Отличается от `can_add_item` тем, что
    /// ингредиенты при списании освобождают свои ячейки.
    pub fn can_add_crafted(&self, recipe: &items::Recipe) -> bool {
        let quantity = recipe.result_quantity;
        if quantity == 0 {
            return true;
        }
        if items::item(recipe.result_id).is_none() {
            return false;
        }
        if self.item_capacity_left(recipe.result_id) < quantity {
            return false;
        }
        self.item_count(recipe.result_id) > 0 || self.inventory_free_slots_with_freed(recipe) > 0
    }

    /// Сколько ещё штук предмета влезет в его ячейку.
    pub fn item_capacity_left(&self, item_id: &str) -> u32 {
        items::ITEM_STACK_LIMIT.saturating_sub(self.item_count(item_id))
    }

    /// Добавляет предмет в сумку. Возвращает `true`, если добавлено всё;
    /// остаток сверх лимита ячейки отбрасывается (как излишки склада).
    pub fn add_item(&mut self, item_id: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        if items::item(item_id).is_none() {
            return false;
        }
        let left = self.item_capacity_left(item_id);
        if left == 0 {
            return false;
        }
        let added = quantity.min(left);
        *self.inventory.entry(item_id.to_string()).or_insert(0) += added;
        added == quantity
    }

    /// Убирает предмет из сумки. Снимает экипировку, если предмет закончился.
    pub fn remove_item(&mut self, item_id: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        let remaining = {
            let Some(current) = self.inventory.get_mut(item_id) else {
                return false;
            };
            if *current < quantity {
                return false;
            }
            *current -= quantity;
            *current
        };
        if remaining == 0 {
            self.inventory.remove(item_id);
        }
        self.ensure_equipment_owned();
        true
    }

    /// Предмет в слоте экипировки.
    pub fn equipped(&self, slot: EquipSlot) -> Option<&str> {
        self.equipment.get(slot)
    }

    /// Предмет каталога в слоте экипировки.
    pub fn equipped_item(&self, slot: EquipSlot) -> Option<&'static items::Item> {
        self.equipped(slot).and_then(items::item)
    }

    /// Надевает предмет из сумки. Один предмет на слот.
    ///
    /// Надетый предмет больше не занимает ячейку: при надевании он списывается
    /// из инвентаря, при снятии возвращается.
    pub fn equip_item(&mut self, item_id: &str) -> Result<(), EquipError> {
        let item = items::item(item_id).ok_or(EquipError::UnknownItem)?;
        let from_inventory = self.item_count(item_id) > 0;
        let from_wardrobe = self.wardrobe_count(item_id) > 0;
        if !from_inventory && !from_wardrobe {
            return Err(EquipError::NotOwned);
        }
        // Проверка «уже надет» — по обоим слотам аксессуаров.
        if self.equipment.get(EquipSlot::Accessory1) == Some(item_id)
            || self.equipment.get(EquipSlot::Accessory2) == Some(item_id)
        {
            return Err(EquipError::AlreadyEquipped);
        }
        let slot = match item.slot {
            Some(EquipSlot::Accessory1) => {
                if self.equipment.get(EquipSlot::Accessory1).is_none() {
                    EquipSlot::Accessory1
                } else if self.equipment.get(EquipSlot::Accessory2).is_none() {
                    EquipSlot::Accessory2
                } else {
                    return Err(EquipError::SlotOccupied);
                }
            }
            Some(slot) => {
                if self.equipment.get(slot).is_some() {
                    return Err(EquipError::SlotOccupied);
                }
                slot
            }
            None => return Err(EquipError::NotEquippable),
        };
        if from_inventory {
            self.remove_one_item(item_id);
        } else {
            self.remove_from_wardrobe(item_id, 1);
        }
        self.equipment.set(slot, Some(item_id.to_string()));
        self.sync_active_set();
        self.recalculate_stats();
        Ok(())
    }

    /// Снимает предмет из слота и возвращает его в сумку.
    pub fn unequip_slot(&mut self, slot: EquipSlot) {
        if let Some(item_id) = self.equipment.get(slot).map(str::to_string) {
            // Если в сумке нет места, предмет остаётся надетым, а не пропадает.
            if self.add_item(&item_id, 1) {
                self.equipment.clear(slot);
            }
        }
        self.sync_active_set();
        self.recalculate_stats();
    }

    /// Переключает активный набор экипировки.
    ///
    /// Снятые вещи уходят в гардероб (не в рюкзак), вещи нового набора
    /// достаются из гардероба или из рюкзака. Если чего-то нет — слот
    /// остаётся пустым, ничего не откатывается: игрок сам разберётся.
    pub fn switch_set(&mut self, index: usize) {
        if index >= self.equipment_sets.len() || index == self.active_set {
            return;
        }
        // Снимаем текущий набор в гардероб и запоминаем его в слоте набора:
        // иначе вернуться к нему будет нечем — предметы останутся в гардеробе,
        // а набор окажется пустым.
        let mut previous = EquippedItems::default();
        for slot in EquipSlot::ALL {
            if let Some(item_id) = self.equipment.get(slot).map(str::to_string) {
                self.add_to_wardrobe(&item_id, 1);
                previous.set(slot, Some(item_id));
                self.equipment.clear(slot);
            }
        }
        self.equipment_sets[self.active_set] = previous;
        self.active_set = index;
        // Надеваем новый набор: сначала пробуем гардероб, потом рюкзак.
        let target = self.equipment_sets[index].clone();
        for slot in EquipSlot::ALL {
            if let Some(item_id) = target.get(slot).map(str::to_string) {
                if self.remove_from_wardrobe(&item_id, 1) {
                    self.equipment.set(slot, Some(item_id));
                } else if self.item_count(&item_id) > 0 {
                    self.remove_one_item(&item_id);
                    self.equipment.set(slot, Some(item_id));
                }
            }
        }
        self.recalculate_stats();
    }

    /// Убирает одну штуку предмета без побочных эффектов экипировки.
    fn remove_one_item(&mut self, item_id: &str) {
        let remaining = {
            let Some(current) = self.inventory.get_mut(item_id) else {
                return;
            };
            if *current == 0 {
                return;
            }
            *current -= 1;
            *current
        };
        if remaining == 0 {
            self.inventory.remove(item_id);
        }
    }

    /// Запоминает текущую экипировку в активном наборе.
    fn sync_active_set(&mut self) {
        if let Some(set) = self.equipment_sets.get_mut(self.active_set) {
            *set = self.equipment.clone();
        }
    }

    /// Пересчитывает бонусы от экипировки.
    pub fn recalculate_stats(&mut self) {
        let mut trade_bonus = 0.0;
        let mut expedition_bonus = 0.0;
        let mut gather_bonus = 0.0;
        let mut fire_discount = 0.0;
        let mut expedition_energy_discount = 0.0;
        let mut smith_xp_bonus = 0.0;
        let mut haggle_bonus = 0.0;
        let mut ingredient_discount = 0.0;
        let mut beggar_pieces = 0;
        for slot in EquipSlot::ALL {
            if let Some(item) = self.equipped_item(slot) {
                let stats = &item.stats;
                if stats.set_id.as_deref() == Some("beggar") {
                    beggar_pieces += 1;
                }
                trade_bonus += stats.trade_bonus.unwrap_or(0.0) / 100.0;
                trade_bonus += stats.gold_bonus.unwrap_or(0.0) / 100.0;
                expedition_bonus += stats.expedition_bonus.unwrap_or(0.0) / 100.0;
                gather_bonus += stats.gather_bonus.unwrap_or(0.0) / 100.0;
                fire_discount += stats.fire_discount.unwrap_or(0.0) / 100.0;
                expedition_energy_discount +=
                    stats.expedition_energy_discount.unwrap_or(0.0) / 100.0;
                smith_xp_bonus += stats.smith_xp_bonus.unwrap_or(0.0) / 100.0;
                haggle_bonus += stats.haggle_bonus.unwrap_or(0.0) / 100.0;
                ingredient_discount += stats.ingredient_discount.unwrap_or(0.0) / 100.0;
            }
        }
        self.trade_bonus = trade_bonus.clamp(0.0, 5.0);
        self.expedition_bonus = expedition_bonus.clamp(0.0, 5.0);
        // Сет «Бродяга»: 4 и более предметов дают +100% к ручной добыче.
        if beggar_pieces >= 4 {
            gather_bonus += 1.0;
        }
        self.gather_bonus = gather_bonus.clamp(0.0, 5.0);
        self.fire_discount = fire_discount.clamp(0.0, 0.9);
        self.expedition_energy_discount = expedition_energy_discount.clamp(0.0, 0.9);
        self.smith_xp_bonus = 1.0 + smith_xp_bonus.clamp(0.0, 2.0);
        self.haggle_bonus = haggle_bonus.clamp(0.0, 0.5);
        self.ingredient_discount = ingredient_discount.clamp(0.0, 0.5);
        // Проверка сета Кузнеца: если надеты все 3 предмета — бонус +15%
        // к опыту крафта. Считаем по `set_id` у надетых предметов.
        let mut smith_pieces = 0;
        for slot in EquipSlot::ALL {
            if let Some(item) = self.equipped_item(slot) {
                if item.stats.set_id.as_deref() == Some("smith") {
                    smith_pieces += 1;
                }
            }
        }
        if smith_pieces >= 3 {
            self.smith_xp_bonus *= 1.15;
        }
    }

    /// Эффективный расход энергии в походе: базовая цена минус скидка
    /// от экипировки, умноженная на дебафф «Кривые руки» (если активен).
    pub fn effective_expedition_energy(&self, base_cost: f64) -> f64 {
        let discount = self.expedition_energy_discount.clamp(0.0, 0.9) as f64;
        base_cost * (1.0 - discount) * self.energy_cost_multiplier()
    }

    /// Множитель скорости/количества лута в походе.
    /// 1.0 = без бонусов, 1.35 = +35% к количеству находок.
    pub fn effective_expedition_bonus(&self) -> f64 {
        let from_equipment = self.expedition_bonus as f64;
        let from_buffs = events::active_buff_sum(self, events::BuffStat::ExpeditionSpeed) / 100.0;
        1.0 + from_equipment + from_buffs
    }

    /// Убирает торговца из чисел изучения: банк открывается после кузницы.
    /// Изученным он считается задним числом, чтобы старые сохранения не теряли банк.
    pub fn drop_trader_research(&mut self) {
        if self.forge_researched {
            self.trader_researched = true;
        }
        self.trader_researchers = 0;
        self.trader_research = 0.0;
    }

    /// Синхронизирует активный набор и пересчитывает бонусы.
    ///
    /// Раньше метод снимал экипировку, если предмета не было в сумке. Теперь
    /// надетые предметы намеренно не лежат в инвентаре, поэтому проверка убрана.
    fn ensure_equipment_owned(&mut self) {
        self.sync_active_set();
        self.recalculate_stats();
    }

    /// Достижения с прогрессом.
    pub fn achievements(&self) -> Vec<AchievementStatus> {
        fn make(id: &'static str, value: f64, target: f64) -> AchievementStatus {
            let ratio = if target <= 0.0 {
                1.0
            } else {
                (value / target).clamp(0.0, 1.0)
            };
            AchievementStatus {
                id,
                progress: ratio,
                done: value + 1e-9 >= target,
                value_text: format!(
                    "{}/{} {}",
                    value.floor(),
                    target.floor(),
                    i18n::t_or(&format!("ach.{id}.unit"), "")
                ),
            }
        }

        vec![
            make("first_steps", self.stats.rests as f64, 10.0),
            make("woodcutter", self.stats.wood_gathered, 500.0),
            make("mason", self.stats.stone_gathered, 500.0),
            make("coal_master", self.stats.coal_produced, 500.0),
            make("miner", self.stats.iron_mined, 250.0),
            make("rich", self.stats.gold_earned, 1000.0),
            make("first_rebirth", self.cycles as f64, 1.0),
            make("master", self.worker_slots() as f64, 5.0),
            make("time_traveler", self.time, 60.0),
        ]
    }

    /// Сбрасывает прогресс цикла, сохраняя циклы, время и статистику.
    ///
    /// Глобальные баффы (`BuffInstance::global`) переживают перерождение,
    /// обычные — нет. Таймеры событий (`next_baseline_at`, `fire_at`,
    /// `expires_at`) живут в базе `total_play_seconds`, поэтому его нельзя
    /// сбрасывать: иначе все отложенные события окажутся «в прошлом».
    pub fn reset_for_cycle(&mut self) {
        // 1. Считаем прибавку к X по текущему циклу (до сброса!) и обновляем рекорды.
        let delta = self.cycle_delta();
        let earned_income = self.earned_income + delta;
        self.max_smith_level_ever = self.max_smith_level_ever.max(self.smith_level());
        let overgather = self.current_overgather().max(0.0);
        self.max_overgather_ever = self.max_overgather_ever.max(overgather);
        let hours = self.current_cycle_hours();
        self.max_hours_ever = self.max_hours_ever.max(hours);
        self.last_cycle_time = (self.total_play_seconds - self.cycle_started_at).max(0.0);

        // 2. Сохраняем то, что не сбрасывается.
        let theme = self.theme;
        let stats = self.stats.clone();
        let cycles = self.cycles + 1;
        let max_smith = self.max_smith_level_ever;
        let max_overgather = self.max_overgather_ever;
        let max_hours = self.max_hours_ever;
        let last_cycle_time = self.last_cycle_time;
        let time = self.time;
        let last_seen = self.last_seen;
        let total_play_seconds = self.total_play_seconds;
        let quest_clock = self.quest_clock;
        let event_clock = self.event_clock;
        let rest_clicks = self.rest_clicks;
        let wood_seen = self.wood_seen;
        let stone_seen = self.stone_seen;
        let dev = std::mem::take(&mut self.dev);
        let now = self.event_clock;
        let global_buffs: Vec<BuffInstance> = self
            .buffs
            .iter()
            .filter(|buff| buff.global && buff.expires_at > now)
            .cloned()
            .collect();

        // 3. Полный сброс.
        *self = GameState::default();
        self.gold = 0.0;
        self.inventory.clear();
        self.equipment = EquippedItems::default();
        self.recalculate_stats();

        // 4. Возвращаем сохранённое.
        self.theme = theme;
        self.stats = stats;
        self.cycles = cycles;
        self.earned_income = earned_income;
        self.max_smith_level_ever = max_smith;
        self.max_overgather_ever = max_overgather;
        self.max_hours_ever = max_hours;
        self.last_cycle_time = last_cycle_time;
        self.cycle_started_at = total_play_seconds;
        self.time = time;
        self.last_seen = last_seen;
        self.total_play_seconds = total_play_seconds;
        self.quest_clock = quest_clock;
        self.event_clock = event_clock;
        self.rest_clicks = rest_clicks;
        self.wood_seen = wood_seen;
        self.stone_seen = stone_seen;
        // Город сбрасывается целиком: лавка, наёмные, кузнецы, приказчик,
        // путешественники и все покупки в городе делаются заново, и вкладка
        // города заново появляется после сюжетной цепочки Мартина.
        self.dev = dev;
        self.buffs = global_buffs;

        // 5. Кузнец: сбрасывается в 1 + бонус за цикл.
        self.smith_level = 1;
        self.smith_xp = 0.0;
        let bonus = balance::SMITH_XP_PER_CYCLE * (cycles.saturating_sub(1)) as f64;
        self.add_smith_xp(bonus);

        // 6. Таймеры событий/квестов.
        self.next_baseline_at = self.event_clock + balance::EVENTS_BASELINE_FIRST_SECS;
        self.quest_board_refresh_at = self.quest_clock + 10.0;
        // 7. Сюжет города начинается заново: квест Мартина на доске.
        city::ensure_intro_quest(self);
    }

    /// Открыта ли добыча ресурсов.
    pub fn gathering_unlocked(&self) -> bool {
        self.rest_clicks >= balance::REST_CLICKS_TO_UNLOCK_GATHER
    }

    /// Открыты ли камень, дом и склад.
    pub fn stone_unlocked(&self) -> bool {
        self.stone_seen
    }

    /// Добавляет ресурс с учётом лимита. Возвращает, сколько реально поместилось.
    ///
    /// Для дерева, камня и угля дробные доли копятся в `resource_fraction`:
    /// работник с 0.5/с даёт единицу ресурса раз в две секунды, а не ноль.
    pub fn add_resource(&mut self, kind: ResourceKind, amount: f64) -> f64 {
        match kind {
            ResourceKind::Gold => {
                let room = (self.gold_cap() - self.gold).max(0.0);
                let added = room.min(amount.max(0.0));
                self.gold = (self.gold + added).min(self.gold_cap());
                added
            }
            ResourceKind::Fire => {
                let room = (self.cap(kind) - self.amount(kind)).max(0.0);
                let added = room.min(amount.max(0.0));
                self.current_fire = (self.current_fire as f64 + added).min(self.cap(kind)) as u32;
                added
            }
            ResourceKind::Wood | ResourceKind::Stone | ResourceKind::Coal => {
                let key = Self::resource_key(kind);
                let mut fraction =
                    self.resource_fraction.get(key).copied().unwrap_or(0.0) + amount.max(0.0);
                let whole = fraction.floor();
                fraction -= whole;
                let room = self.active_cap(key).saturating_sub(self.active_amount(key)) as f64;
                let added = whole.min(room);
                if added > 0.0 {
                    self.add_active(key, added as u32);
                }
                // Если склад полон, целая часть сгорает, как и раньше.
                self.resource_fraction.insert(key.to_string(), fraction);
                added
            }
        }
    }

    /// Начисляет добытый ресурс: склад, статистика и счётчик текущего цикла.
    ///
    /// Единственная точка учёта добычи. Через неё идут клик по ресурсу и
    /// «Труд» из тика; торговля и походы вызывают `add_resource` напрямую,
    /// поэтому в порог перерождения не попадают.
    pub fn add_produced(&mut self, kind: ResourceKind, amount: f64) -> f64 {
        let added = self.add_resource(kind, amount);
        match kind {
            ResourceKind::Wood => {
                self.cycle_wood += added;
                self.stats.wood_gathered += added;
            }
            ResourceKind::Stone => {
                self.cycle_stone += added;
                self.stats.stone_gathered += added;
            }
            _ => {}
        }
        added
    }

    /// Тратит энергию, если её хватает.
    pub fn spend_energy(&mut self, amount: f64) -> bool {
        if self.energy + 1e-9 >= amount {
            self.energy = (self.energy - amount).max(0.0);
            true
        } else {
            false
        }
    }

    /// Добавляет энергию с учётом максимума. Возвращает прибавку.
    pub fn add_energy(&mut self, amount: f64) -> f64 {
        let before = self.energy;
        self.energy = (self.energy + amount.max(0.0)).min(self.max_energy());
        self.energy - before
    }

    /// Доля заполнения полосы энергии от 0.0 до 1.0.
    pub fn energy_ratio(&self) -> f64 {
        let max = self.max_energy();
        if max <= 0.0 {
            0.0
        } else {
            (self.energy / max).clamp(0.0, 1.0)
        }
    }

    /// Обновляет сохранение старого формата: демо-капитал, версия и рынки.
    pub fn migrate(&mut self) {
        let previous_version = self.version;
        if self.version < balance::SAVE_VERSION {
            // Снос условий выживания: чужие сохранения переводятся на новую
            // экономику. С v7 демо-капитал больше не выдаётся, поэтому старая
            // ветка «обнулить золото и сумку» больше не нужна.
            self.version = balance::SAVE_VERSION;
        }
        if self.active_resources.is_empty() {
            // Сохранения старого формата хранили ресурсы в плоских полях.
            self.active_resources
                .insert("wood".to_string(), self.wood as u32);
            self.active_resources
                .insert("stone".to_string(), self.stone as u32);
            self.active_resources
                .insert("iron".to_string(), self.iron as u32);
            self.active_resources
                .insert("coal".to_string(), self.coal as u32);
        }
        // Руда больше не живёт на складе: переносим её в рюкзак.
        if let Some(iron) = self.active_resources.remove("iron") {
            if iron > 0 && !self.add_item("iron", iron) {
                // Часть не поместилась — теряем остаток, как излишки склада.
            }
        }
        // Глина переехала со склада в рюкзак, а зелье силы и легаси-руда удалены.
        if let Some(clay) = self.active_resources.remove("clay") {
            if clay > 0 && self.add_item("clay", clay) {
                // Перенесено без потерь.
            }
        }
        self.inventory.remove("strength_potion");
        self.inventory.remove("iron_ore");
        // Предметы, которых больше нет в новой экономике.
        self.inventory.remove("coal_amulet");
        // Одна ячейка — не больше лимита штук.
        for quantity in self.inventory.values_mut() {
            *quantity = (*quantity).min(items::ITEM_STACK_LIMIT);
        }

        // Слот Back появился в v11: старые сохранения держали плащи и рюкзак
        // Гардероб появился в v11. У старых сохранений его нет — serde
        // уже подставил пустую карту через `#[serde(default)]`, но проверим
        // на случай ручной правки JSON.
        if !self.wardrobe.is_empty() {
            // Отфильтровываем не-экипировку (на случай, если кто-то вручную
            // вписал туда ресурсы).
            self.wardrobe
                .retain(|id, _| items::item(id).and_then(|i| i.slot).is_some());
        }
        // Доска объявлений появилась в v12. У старых сохранений quest_clock
        // = 0, а таймеры квестов живут в total_play_seconds — переносим.
        if self.quest_clock <= 0.0 {
            self.quest_clock = self.total_play_seconds;
        }
        if self.event_clock <= 0.0 {
            self.event_clock = self.total_play_seconds;
        }
        if self.quest_board_refresh_at <= 0.0 {
            self.quest_board_refresh_at = self.quest_clock + 10.0;
        }
        // Коллекторы раньше приходили раз в 10 минут. Если в сохранении
        // остался длинный таймер, срезаем его до нового интервала, иначе
        // первый визит после обновления пришлось бы ждать вдвое дольше.
        let max_collector_wait = super::quests::COLLECTOR_INTERVAL;
        if self.collector_tick_at > self.quest_clock + max_collector_wait {
            self.collector_tick_at = self.quest_clock + max_collector_wait;
        }
        // Слот Back появился в v11: старые сохранения держали плащи и рюкзак
        // в Body. Переносим и в активном наборе, и в сохранённых.
        move_body_item_to_back(&mut self.equipment);
        for set in &mut self.equipment_sets {
            move_body_item_to_back(set);
        }

        // Старые сохранения: если постройка уже построена или исследована,
        // считаем её открытой, чтобы кнопка «Построить 0→1» не появлялась
        // повторно и не требовала оплаты открытия заново.
        if !self.coal_pit_unlocked && (self.coal_pit_researched || self.coal_pit_level > 0) {
            self.coal_pit_unlocked = true;
        }
        if !self.forge_unlocked && (self.forge_researched || self.forge_level > 0) {
            self.forge_unlocked = true;
        }
        if !self.bank_unlocked && (self.bank_level > 0 || self.trader_researched) {
            self.bank_unlocked = true;
        }
        // Собранный прототип (или уже начатая эра) означает изученный чертёж.
        if self.steam_prototype_built || self.steam_era {
            self.steam_blueprint_studied = true;
        }
        if self.steam_era {
            self.steam_prototype_built = true;
        }
        self.inventory.retain(|_, quantity| *quantity > 0);
        // Если загрузили старое сохранение, где нет уровня кузнеца — ставим 1.
        if self.smith_level == 0 {
            self.smith_level = 1;
        }
        self.ensure_markets();
        if self.active_set >= self.equipment_sets.len() {
            self.active_set = 0;
        }
        // До v9 надетый предмет оставался в сумке. Убираем дубликат, чтобы
        // после обновления он не занимал ячейку.
        if previous_version < 9 {
            for slot in EquipSlot::ALL {
                if let Some(item_id) = self.equipment.get(slot).map(str::to_string) {
                    self.remove_one_item(&item_id);
                }
            }
        }
        // Старое поле `accessory` → новый слот `accessory1`.
        self.equipment.migrate_legacy_accessory();
        for set in &mut self.equipment_sets {
            set.migrate_legacy_accessory();
        }
        self.ensure_equipment_owned();
        self.drop_trader_research();
        // Игры до v0.7.16 считали общее время только в статистике; переносим
        // его, чтобы после обновления счётчик не начинался с нуля.
        if self.total_play_seconds <= 0.0 {
            self.total_play_seconds = self.stats.play_seconds;
        }
        self.recalculate_stats();
        // Поход, застрявший после сворачивания игры, закрываем при малой энергии.
        if self.active_expedition.is_some() && self.energy <= balance::EXPEDITION_MIN_ENERGY {
            self.active_expedition = None;
            self.expedition_timer = 0.0;
        }
        // События: чистим протухшие баффы (например, если игрок не заходил
        // неделю), очередь оффлайн-событий ограничиваем одним — чтобы не
        // встретить игрока стеной окон.
        let now = self.event_clock;
        self.buffs.retain(|buff| buff.expires_at > now);
        self.offline_events.truncate(1);
        // Старое сохранение без поля: ставим таймер от «сейчас».
        // Сохранение с полем в прошлом (игрок не заходил) — тоже от «сейчас»,
        // иначе первое же событие выпадет мгновенно при заходе.
        if self.next_baseline_at <= now {
            self.next_baseline_at = now + balance::EVENTS_BASELINE_FIRST_SECS;
        }
        if self.city.unlocked {
            if self.city.next_tax_at <= 0.0 {
                self.city.next_tax_at = self.total_play_seconds + balance::CITY_TAX_INTERVAL;
            }
            if self.city.next_event_at <= 0.0 {
                self.city.next_event_at = self.total_play_seconds + balance::CITY_TAX_INTERVAL;
            }
            if self.city.shop.is_none() {
                self.city.shop = Some(city::Shop::default());
            }
        }
        // Дом кузнеца из гильдии стал первым кузнецом в «Труде».
        city::migrate_smiths(self);
        // Награды банкира больше не удорожают покупные ячейки.
        city::migrate_bank_cells(self);
        // Старые сохранения могли держать на полках распроданные позиции.
        city::drop_empty_slots(self);
        // Спрос города переехал на торговую модель (спрос/предложение/тренд).
        city::migrate_demand(self);
        self.recalculate_stats();

        // Рекорды появились в новой версии. У старых сохранений их нет —
        // инициализируем разумными значениями. Это надо сделать ДО мягкой
        // миграции: она считает новый X и должна видеть настоящий уровень.
        if self.max_smith_level_ever == 0 {
            self.max_smith_level_ever = self.smith_level();
        }
        if self.cycle_started_at <= 0.0 {
            self.cycle_started_at = self.total_play_seconds;
        }
        // max_overgather_ever и max_hours_ever — serde default = 0.0, это ок.

        // Старые сохранения: X раньше считался из рекордов, теперь копится
        // в `earned_income`. Если поле пустое (serde default = 1.0), а
        // перерождения уже были — пересчитываем старую формулу один раз.
        if self.earned_income <= 1.0 && self.cycles > 0 {
            self.earned_income = 1.0
                + 0.05 * self.cycles as f64
                + 0.03 * self.max_smith_level_ever.saturating_sub(1) as f64
                + 0.10 * self.max_overgather_ever.min(3.0)
                + 0.05 * self.max_hours_ever.min(3.0);
        }
    }

    /// Приводит рынки в соответствие с каталогом торговцев.
    fn ensure_markets(&mut self) {
        if self.traders.len() != trader::TRADERS.len() {
            self.traders = trader::default_markets();
            return;
        }
        for (index, market) in self.traders.iter_mut().enumerate() {
            // Витрина — это подмножество ассортимента, а не весь список,
            // поэтому сверяем не длины, а сами товары. Резервные карточки
            // (вещи игрока) законны и переживают загрузку.
            let roster = trader::TRADERS[index].goods;
            let has_foreign = market
                .goods
                .iter()
                .any(|good| !good.is_reserve() && !roster.contains(&good.item_id.as_str()));
            if has_foreign {
                *market = trader::default_market(index);
            }
        }
    }

    /// Обновляет флаги открытия по текущим ресурсам. Флаги не откатываются.
    pub fn refresh_unlocks(&mut self) {
        if self.amount(ResourceKind::Wood) >= balance::WOOD_VISIBLE_FROM {
            self.wood_seen = true;
        }
        if self.amount(ResourceKind::Wood) >= balance::STONE_UNLOCK_WOOD {
            self.stone_seen = true;
        }
        if self.active_amount("coal") > 0 {
            self.coal_seen = true;
        }
    }

    /// Открыта ли локация похода.
    pub fn location_unlocked(&self, location_id: &str) -> bool {
        super::expedition::STARTING_LOCATIONS.contains(&location_id)
            || self.unlocked_locations.iter().any(|id| id == location_id)
    }

    /// Открывает локацию. Возвращает `true`, если она была закрыта.
    pub fn unlock_location(&mut self, location_id: &str) -> bool {
        if self.location_unlocked(location_id) {
            return false;
        }
        if super::expedition::location(location_id).is_none() {
            return false;
        }
        self.unlocked_locations.push(location_id.to_string());
        true
    }

    /// Последняя открытая картой локация — только в ней падают карты.
    ///
    /// Стартовые локации не считаются «открытыми картой»: цепочка идёт
    /// со шахт — в них выпадает карта Забытой каменоломни.
    pub fn current_location(&self) -> &'static str {
        for location_id in self.unlocked_locations.iter().rev() {
            let Some(location) = super::expedition::location(location_id) else {
                continue;
            };
            return location.id;
        }
        "field"
    }

    /// Есть ли у игрока требуемый локации предмет (кирка, свеча).
    pub fn meets_location_requirement(&self, requirement: Option<&'static str>) -> bool {
        match requirement {
            None => true,
            Some(item_id) => self.item_count(item_id) > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::actions;

    #[test]
    fn legacy_accessory_moves_to_first_slot() {
        let mut state = GameState {
            version: 9,
            ..GameState::default()
        };
        state.equipment = EquippedItems {
            accessory: Some("trader_ring".to_string()),
            ..EquippedItems::default()
        };

        state.migrate();

        assert_eq!(
            state.equipped(items::EquipSlot::Accessory1),
            Some("trader_ring")
        );
        assert!(state.equipment.accessory.is_none());
        // Бонус старого кольца снова работает.
        assert!((state.trade_bonus - 0.15).abs() < 1e-6);
    }

    #[test]
    fn seven_slots_accept_all_items() {
        let mut state = GameState::default();
        for item_id in [
            "wretched_hat",
            "iron_armor",
            "backpack",
            "iron_shovel",
            "leather_boots",
            "silver_ring",
            "miner_amulet",
        ] {
            state.add_item(item_id, 1);
            assert_eq!(state.equip_item(item_id), Ok(()), "не надеть {item_id}");
        }
        // Шапка даёт +5 к запасу энергии, броня — +30.
        assert_eq!(state.max_energy(), 100.0 + 5.0 + 30.0);

        for slot in EquipSlot::ALL {
            assert!(
                state.equipped(slot).is_some(),
                "пустой слот {}",
                slot.label()
            );
        }
    }

    #[test]
    fn smith_set_fits_three_pieces_and_boosts_craft_xp() {
        let mut state = GameState::default();
        for item_id in ["smith_ring", "smith_amulet", "smith_belt"] {
            state.add_item(item_id, 1);
            assert_eq!(state.equip_item(item_id), Ok(()), "не надеть {item_id}");
        }

        // Кольцо +10%, амулет +10%, сет +15% → 1.2 × 1.15 = 1.38.
        assert!((state.smith_xp_bonus - 1.38).abs() < 1e-6);
    }

    #[test]
    fn beggar_set_boosts_gather_from_four_pieces() {
        let mut state = GameState::default();
        for item_id in ["robe", "beggars_cloak", "wretched_hat"] {
            state.add_item(item_id, 1);
            assert_eq!(state.equip_item(item_id), Ok(()), "не надеть {item_id}");
        }
        // Три предмета — бонус ещё не активен.
        assert!((state.gather_bonus).abs() < 1e-6);

        state.add_item("poor_boots", 1);
        assert_eq!(state.equip_item("poor_boots"), Ok(()));
        // Четвёртый предмет включает сет: +100% к ручной добыче.
        assert!((state.gather_bonus - 1.0).abs() < 1e-6);

        // Пятый предмет добавляет и свой личный бонус (+8%).
        state.add_item("beggars_gloves", 1);
        assert_eq!(state.equip_item("beggars_gloves"), Ok(()));
        assert!((state.gather_bonus - 1.08).abs() < 1e-6);
    }

    #[test]
    fn new_game_starts_empty() {
        let state = GameState::default();
        assert_eq!(state.gold, 0.0);
        assert!(state.inventory.is_empty());
        assert_eq!(state.active_amount("wood"), 0);
        assert_eq!(state.active_amount("stone"), 0);
        assert_eq!(state.item_count("iron"), 0);
        assert_eq!(state.active_amount("coal"), 0);
        assert_eq!(state.energy, balance::START_ENERGY);
    }

    #[test]
    fn play_time_is_counted_and_saved() {
        let mut state = GameState::default();
        actions::tick(&mut state, 3600.0);
        assert!((state.total_play_seconds - 3600.0).abs() < 1e-9);
        actions::tick(&mut state, 30.0);
        assert!((state.total_play_seconds - 3630.0).abs() < 1e-9);

        // Счётчик попадает в сохранение.
        let json = serde_json::to_string(&state).expect("сериализация");
        let restored: GameState = serde_json::from_str(&json).expect("десериализация");
        assert!((restored.total_play_seconds - 3630.0).abs() < 1e-9);
    }

    #[test]
    fn old_save_keeps_play_time_from_stats() {
        // Сохранение до появления счётчика: время лежит только в статистике.
        let mut state = GameState {
            total_play_seconds: 0.0,
            ..GameState::default()
        };
        state.stats.play_seconds = 5000.0;
        state.migrate();
        assert!((state.total_play_seconds - 5000.0).abs() < 1e-9);
    }

    #[test]
    fn one_cell_per_item_and_stack_limit() {
        let mut state = GameState::default();
        assert!(state.add_item("iron_ingot", 10));
        assert_eq!(state.inventory_used_slots(), 1);
        // Переполнение лимита срезается, но остальное добавляется.
        assert!(state.add_item("iron_ingot", items::ITEM_STACK_LIMIT - 10));
        assert_eq!(state.item_count("iron_ingot"), items::ITEM_STACK_LIMIT);
        assert!(!state.can_add_item("iron_ingot", 1));
        assert!(!state.add_item("iron_ingot", 5));
        assert_eq!(state.item_count("iron_ingot"), items::ITEM_STACK_LIMIT);
        assert_eq!(state.inventory_used_slots(), 1);
    }

    #[test]
    fn gold_cap_is_thousand_per_bank_level() {
        let mut state = GameState::default();
        assert_eq!(state.gold_cap(), 1_000.0);
        state.bank_level = 2;
        assert_eq!(state.gold_cap(), 3_000.0);
    }

    #[test]
    fn migration_keeps_player_reserve_cards() {
        // Витрина — подмножество ассортимента, поэтому миграция не должна
        // сбрасывать её только из-за несовпадения длины. Резервные карточки
        // (вещи игрока) переживают загрузку.
        let mut state = GameState::default();
        let mut reserve = trader::GoodState::default();
        reserve.item_id = "archers_jacket".to_string();
        reserve.stock = 1;
        reserve.buy_limit = 0;
        reserve.avg_cost = Some(100.0);
        state.traders[0].goods.push(reserve);

        state.migrate();

        assert!(
            state.traders[0]
                .goods
                .iter()
                .any(|good| good.item_id == "archers_jacket" && good.is_reserve()),
            "резервная карточка пропала при миграции"
        );
    }

    #[test]
    fn gold_stops_at_cap_and_does_not_reset() {
        let mut state = GameState::default();
        let cap = state.gold_cap();
        state.add_resource(ResourceKind::Gold, cap);
        assert_eq!(state.gold, cap);

        // Попытка добавить сверх лимита ничего не отнимает.
        let added = state.add_resource(ResourceKind::Gold, 500.0);
        assert_eq!(added, 0.0);
        assert_eq!(state.gold, cap);

        // Сверх лимита золото тоже не растёт и не обнуляется.
        state.add_resource(ResourceKind::Gold, 1_000_000.0);
        assert_eq!(state.gold, cap);

        // Банк поднимает лимит — золото на месте.
        state.bank_level = 1;
        assert_eq!(state.gold, cap);
        state.add_resource(ResourceKind::Gold, 500.0);
        assert_eq!(state.gold, cap + 500.0);
    }

    #[test]
    fn gold_at_cap_survives_saving_and_bank_upgrade() {
        let mut state = GameState::default();
        state.add_resource(ResourceKind::Gold, 5_000.0);
        assert_eq!(state.gold, state.gold_cap());

        // Полный бар золота переживает круговую сериализацию.
        let text = serde_json::to_string(&state).expect("сериализация");
        let mut restored: GameState = serde_json::from_str(&text).expect("десериализация");
        assert_eq!(restored.gold, state.gold_cap());
        restored.migrate();
        assert_eq!(restored.gold, state.gold_cap());

        // Апгрейд банка поднимает лимит и не сбрасывает золото.
        restored.bank_level = 1;
        restored.refresh_unlocks();
        assert_eq!(restored.gold, state.gold_cap());
    }

    #[test]
    fn migration_moves_ore_from_storage_to_backpack() {
        let mut state = GameState {
            version: 7,
            gold: 1_240.0,
            ..GameState::default()
        };
        state.inventory.insert("leather".to_string(), 2);
        state.active_resources.insert("iron".to_string(), 40);
        state.migrate();
        // Золото и сумку больше не обнуляем: демо-капитал не выдаётся с v7.
        assert_eq!(state.gold, 1_240.0);
        assert_eq!(state.item_count("leather"), 2);
        assert_eq!(state.item_count("iron"), 40);
        assert!(!state.active_resources.contains_key("iron"));
    }

    #[test]
    fn soft_migration_applies_only_to_old_saves() {
        // Новая игра без перерождений миграцию не запускает.
        let mut fresh = GameState::default();
        fresh.migrate();
        assert!((fresh.earned_income - 1.0).abs() < 1e-9);
        assert!((fresh.income_multiplier() - 1.0).abs() < 1e-9);

        // Если X уже накоплен, миграция его не пересчитывает.
        let mut progressed = GameState {
            cycles: 10,
            earned_income: 1.5,
            ..GameState::default()
        };
        progressed.migrate();
        assert!((progressed.earned_income - 1.5).abs() < 1e-9);

        // Старое сохранение (без поля earned_income): пересчёт старой формулы.
        let mut old = GameState {
            cycles: 10,
            max_smith_level_ever: 5,
            ..GameState::default()
        };
        old.migrate();
        let expected = 1.0 + 0.05 * 10.0 + 0.03 * 4.0;
        assert!((old.earned_income - expected).abs() < 1e-9);
        assert!((old.income_multiplier() - expected).abs() < 1e-9);
    }

    #[test]
    fn new_game_starts_with_earned_income_one() {
        let state = GameState::default();
        assert!((state.earned_income - 1.0).abs() < 1e-9);
        assert!((state.income_multiplier() - 1.0).abs() < 1e-9);
        assert!((state.preview_income_multiplier() - (1.0 + state.cycle_delta())).abs() < 1e-9);
    }

    #[test]
    fn cycle_delta_grows_with_overgather() {
        let base = GameState::default();
        let mut over = GameState::default();
        over.cycle_wood = over.cycle_threshold() * 1.5; // overgather = 0.5

        assert!(over.cycle_delta() > base.cycle_delta());
        let (_, _, overgather_part, _) = over.cycle_delta_parts();
        assert!((overgather_part - 0.15 * 0.5_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn cycle_delta_uses_sqrt() {
        let mut one = GameState::default();
        let mut four = GameState::default();
        let threshold = one.cycle_threshold();
        one.cycle_wood = threshold * 2.0; // overgather = 1
        four.cycle_wood = threshold * 5.0; // overgather = 4

        // √4 − √1 = 1, значит прибавка 0.15, а не 0.45 при линейном росте.
        assert!((four.cycle_delta() - one.cycle_delta() - 0.15).abs() < 1e-9);
    }

    #[test]
    fn earned_income_accumulates_across_cycles() {
        let mut state = GameState::default();
        state.cycle_wood = state.cycle_threshold();
        let first = state.cycle_delta();
        state.reset_for_cycle();
        assert!((state.earned_income - (1.0 + first)).abs() < 1e-9);

        state.cycle_wood = state.cycle_threshold();
        let second = state.cycle_delta();
        state.reset_for_cycle();
        assert!((state.earned_income - (1.0 + first + second)).abs() < 1e-9);
        assert!(state.earned_income > 1.0 + first);
    }

    #[test]
    fn old_save_migration_preserves_income() {
        // Старое сохранение: поля earned_income нет, serde подставил бы 1.0.
        let mut old = GameState {
            cycles: 3,
            max_smith_level_ever: 7,
            max_overgather_ever: 2.0,
            max_hours_ever: 1.5,
            ..GameState::default()
        };
        old.migrate();

        let expected = 1.0 + 0.05 * 3.0 + 0.03 * 6.0 + 0.10 * 2.0 + 0.05 * 1.5;
        assert!((old.earned_income - expected).abs() < 1e-9);
        assert!((old.income_multiplier() - expected).abs() < 1e-9);
    }

    #[test]
    fn location_chain_unlocks_by_map() {
        let state = GameState::default();
        assert!(state.location_unlocked("field"));
        assert!(state.location_unlocked("ravine"));
        assert!(!state.location_unlocked("deep_forest"));
        assert!(!state.location_unlocked("deep_mines"));
        assert!(!state.location_unlocked("forgotten_quarry"));
        assert!(!state.location_unlocked("ancient_ruins"));
        assert_eq!(state.current_location(), "field");
    }

    #[test]
    fn fire_goes_green_with_forge_level() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        let base_max = state.fire_max();
        let base_coal = state.fire_per_coal();
        // Первый уровень уже равен норме: fire_max() = 67 × level.
        assert_eq!(base_max, balance::FORGE_FIRE_MAX_SECONDS);
        state.forge_level = 3;
        assert_eq!(
            state.fire_max(),
            base_max + 2 * balance::FORGE_FIRE_PER_LEVEL
        );
        assert_eq!(state.fire_max(), 3 * balance::FORGE_FIRE_MAX_SECONDS);
        assert_eq!(
            state.fire_per_coal(),
            base_coal + 2 * balance::FIRE_PER_COAL_PER_LEVEL
        );

        state.forge_level = 10;
        assert_eq!(state.fire_max(), 670);
    }

    #[test]
    fn smith_xp_can_jump_several_levels() {
        let mut state = GameState::default();
        // Новый порог: 100 × level^1.6. 1 → 2 стоит 100, 2 → 3 — около 303.
        let first = balance::smith_xp_for_level(1);
        let second = balance::smith_xp_for_level(2);
        state.add_smith_xp(first + second + 0.5);
        assert_eq!(state.smith_level, 3);
        assert!((state.smith_xp - 0.5).abs() < 1e-6);
    }

    #[test]
    fn smith_level_is_not_capped() {
        let mut state = GameState::default();
        state.add_smith_xp(1_000_000.0);
        assert!(
            state.smith_level > balance::SMITH_MAX_LEVEL,
            "уровень больше не упирается в потолок"
        );
        // Полоска остаётся в допустимых пределах, а не NaN.
        let ratio = state.smith_xp_ratio();
        assert!((0.0..=1.0).contains(&ratio));
    }

    #[test]
    fn switch_set_moves_items_to_wardrobe() {
        let mut state = GameState::default();
        state.add_item("wretched_hat", 1);
        state.add_item("trader_ring", 1);
        state.add_item("iron_boots", 1);

        // Собираем Set 1.
        assert_eq!(state.equip_item("wretched_hat"), Ok(()));
        assert_eq!(state.equip_item("trader_ring"), Ok(()));
        assert_eq!(state.equip_item("iron_boots"), Ok(()));

        // Переключаемся на пустой Set 2.
        state.switch_set(1);

        // Все предметы теперь в гардеробе, а не в рюкзаке.
        assert_eq!(state.wardrobe_count("wretched_hat"), 1);
        assert_eq!(state.wardrobe_count("trader_ring"), 1);
        assert_eq!(state.wardrobe_count("iron_boots"), 1);
        assert_eq!(state.item_count("wretched_hat"), 0);
        for slot in EquipSlot::ALL {
            assert!(
                state.equipped(slot).is_none(),
                "слот {} не пуст",
                slot.label()
            );
        }

        // Возвращаемся к Set 1 — предметы должны надеться из гардероба.
        state.switch_set(0);
        assert!(state.wardrobe.is_empty());
        assert_eq!(state.equipped(EquipSlot::Head), Some("wretched_hat"));
        assert_eq!(state.equipped(EquipSlot::Accessory1), Some("trader_ring"));
        assert_eq!(state.equipped(EquipSlot::Feet), Some("iron_boots"));
    }

    #[test]
    fn wardrobe_items_can_be_moved_back_to_bag() {
        let mut state = GameState::default();
        state.add_to_wardrobe("wretched_hat", 1);
        assert_eq!(state.wardrobe_count("wretched_hat"), 1);
        assert_eq!(state.item_count("wretched_hat"), 0);

        // Нельзя положить не-экипировку.
        state.add_to_wardrobe("wood", 5);
        assert_eq!(state.wardrobe_count("wood"), 0);

        let moved = actions::take_from_wardrobe(&mut state, "wretched_hat");
        assert!(moved);
        assert_eq!(state.wardrobe_count("wretched_hat"), 0);
        assert_eq!(state.item_count("wretched_hat"), 1);
    }

    #[test]
    fn new_game_has_empty_quest_board_and_no_debt() {
        let state = GameState::default();
        assert!(state.quests.is_empty());
        assert_eq!(state.debt, 0.0);
        // Таймер обновления — 0.0 (поставится в migrate).
        assert_eq!(state.quest_board_refresh_at, 0.0);
    }
}
