use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::balance;
use super::items::{self, EquipSlot};
use super::trader::{self, TraderMarket};

/// Виды ресурсов в баре: дерево, камень, огонь, золото. Руда и прочие
/// находки живут в рюкзаке.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Wood,
    Stone,
    Fire,
    Gold,
}

impl ResourceKind {
    /// Название ресурса для текстов (задел на будущие экраны).
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::Wood => "дерево",
            Self::Stone => "камень",
            Self::Fire => "огонь",
            Self::Gold => "золото",
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

impl ResearchKind {
    pub const ALL: [ResearchKind; 3] = [Self::CoalPit, Self::Forge, Self::Bank];

    pub fn name(self) -> &'static str {
        match self {
            Self::CoalPit => "Угольная яма",
            Self::Forge => "Кузница",
            Self::Bank => "Банк",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::CoalPit => "Открывает угольную яму: работники тратят дерево и добывают уголь.",
            Self::Forge => "Открывает кузницу: огонь из угля плавит камень в железо.",
            Self::Bank => "Открывает банк: повышает лимит золота.",
        }
    }

    /// Что нужно изучить перед этим направлением.
    pub fn requirement(self) -> Option<&'static str> {
        match self {
            Self::CoalPit => None,
            Self::Forge => Some("угольную яму"),
            Self::Bank => Some("кузницу"),
        }
    }
}

/// Надетая экипировка. Один предмет на слот.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EquippedItems {
    pub hand: Option<String>,
    pub body: Option<String>,
    pub accessory: Option<String>,
}

impl EquippedItems {
    pub fn get(&self, slot: EquipSlot) -> Option<&str> {
        match slot {
            EquipSlot::Hand => self.hand.as_deref(),
            EquipSlot::Body => self.body.as_deref(),
            EquipSlot::Accessory => self.accessory.as_deref(),
        }
    }

    pub fn set(&mut self, slot: EquipSlot, item_id: Option<String>) {
        match slot {
            EquipSlot::Hand => self.hand = item_id,
            EquipSlot::Body => self.body = item_id,
            EquipSlot::Accessory => self.accessory = item_id,
        }
    }

    pub fn clear(&mut self, slot: EquipSlot) {
        self.set(slot, None);
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
            Self::UnknownItem => "Такого предмета нет.",
            Self::NotOwned => "Этого предмета нет в сумке.",
            Self::NotEquippable => "Этот предмет нельзя надеть.",
            Self::AlreadyEquipped => "Предмет уже надет.",
            Self::SlotOccupied => "Слот занят. Сначала сними текущий предмет.",
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
    pub name: &'static str,
    pub description: &'static str,
    pub progress: f64,
    pub done: bool,
    pub value_text: String,
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
}

impl ActiveExpedition {
    pub fn new(location_id: &str) -> Self {
        Self {
            location_id: location_id.to_string(),
            elapsed: 0,
            found: Vec::new(),
            notice: None,
            done: false,
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

    /// Сколько раз игрок нажимал «Отдых».
    pub rest_clicks: u32,
    /// Уровень своего дома (0 — ещё не построен).
    pub house_level: u32,
    /// Уровень «Уюта»: повышает максимум энергии.
    pub comfort_level: u32,
    /// Уровень склада.
    pub storage_level: u32,
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

    /// Изучен ли торговец.
    pub trader_researched: bool,
    /// Накопленный опыт изучения торговца.
    pub trader_research: f64,
    /// Сколько исследователей занято торговцем.
    pub trader_researchers: u32,

    /// Инвентарь: ID предмета -> количество.
    pub inventory: HashMap<String, u32>,
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
    /// −% расход огня кузницы от экипировки.
    pub fire_discount: f32,
    /// −% расход энергии в походах от экипировки.
    pub expedition_energy_discount: f32,
    /// Рынки всех торговцев.
    pub traders: Vec<TraderMarket>,
    /// Накопленное время до следующего шага рынка.
    pub market_timer: f64,
    /// Детерминированное зерно рынка.
    pub market_seed: u64,
    /// Накопленная дробная секунда пути в походе.
    pub expedition_timer: f64,
    /// Активный поход: `None` — игрок не в пути. Хранится в состоянии, чтобы
    /// поход шёл и при закрытой вкладке.
    pub active_expedition: Option<ActiveExpedition>,
    /// Открытые картами локации походов (кроме стартовых поля и оврага).
    pub unlocked_locations: Vec<String>,

    /// Сколько циклов (перерождений) пройдено.
    pub cycles: u32,
    /// Ресурс «время» за оффлайн-прогресс.
    pub time: f64,
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
            rest_clicks: 0,
            house_level: 0,
            comfort_level: 0,
            storage_level: 0,
            worker_house_level: 0,
            research_house_level: 0,
            sawmill_level: 0,
            mine_level: 0,
            bank_level: 0,
            coal_pit_level: 0,
            forge_researched: false,
            forge_research: 0.0,
            forge_researchers: 0,
            forge_level: 0,
            current_fire: 0,
            forge_auto_feed: false,
            trader_researched: false,
            trader_research: 0.0,
            trader_researchers: 0,
            inventory,
            equipment: EquippedItems::default(),
            equipment_sets: std::array::from_fn(|_| EquippedItems::default()),
            active_set: 0,
            trade_bonus: 0.0,
            expedition_bonus: 0.0,
            gather_bonus: 0.0,
            fire_discount: 0.0,
            expedition_energy_discount: 0.0,
            traders: trader::default_markets(),
            market_timer: 0.0,
            market_seed: 0x9e37_79b9_7f4a_7c15,
            expedition_timer: 0.0,
            active_expedition: None,
            unlocked_locations: Vec::new(),
            cycles: 0,
            time: 0.0,
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
            ResourceKind::Fire => "fire",
            ResourceKind::Gold => "gold",
        }
    }

    pub fn amount(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Wood => self.active_amount("wood") as f64,
            ResourceKind::Stone => self.active_amount("stone") as f64,
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
            }
        }
    }

    /// Лимит золота с учётом банка.
    pub fn gold_cap(&self) -> f64 {
        balance::BASE_GOLD_CAP + self.bank_level as f64 * balance::GOLD_CAP_PER_LEVEL
    }

    /// Регенерация энергии в секунду. Без дома — ноль.
    pub fn energy_regen(&self) -> f64 {
        self.house_level as f64 * balance::HOUSE_ENERGY_PER_LEVEL + self.dev.energy_regen
    }

    /// Сколько энергии даёт одно нажатие «Отдых».
    pub fn rest_energy(&self) -> f64 {
        balance::REST_ENERGY
            + self.house_level as f64 * balance::REST_ENERGY_PER_HOUSE_LEVEL
            + self.dev.rest_bonus
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

    /// Дерево в секунду с лесопилки.
    pub fn sawmill_wood_per_sec(&self) -> f64 {
        self.sawmill_workers as f64
            * balance::SAWMILL_WOOD_PER_WORKER_PER_SEC
            * self.production_multiplier()
            * (1.0 + self.gather_bonus as f64)
    }

    /// Камень в секунду с шахты (железо теперь только в походах и у торговца).
    pub fn mine_stone_per_sec(&self) -> f64 {
        self.mine_workers as f64
            * balance::MINE_STONE_PER_WORKER_PER_SEC
            * self.production_multiplier()
            * (1.0 + self.gather_bonus as f64)
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
    pub fn max_energy(&self) -> f64 {
        let equipment_bonus: u32 = EquipSlot::ALL
            .iter()
            .filter_map(|slot| self.equipped_item(*slot))
            .filter_map(|item| item.stats.max_energy)
            .sum();
        self.energy_max
            + self.comfort_level as f64 * balance::ENERGY_CAP_PER_COMFORT
            + equipment_bonus as f64
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

    /// Лимит активного ресурса.
    pub fn active_cap(&self, _kind: &str) -> u32 {
        balance::BASE_STORAGE_CAP as u32
            + self.storage_level * balance::STORAGE_CAP_PER_LEVEL as u32
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

    /// Постоянный множитель производства за пройденные циклы.
    pub fn production_multiplier(&self) -> f64 {
        1.0 + self.cycles as f64 * balance::CYCLE_BONUS_PER_CYCLE
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

    /// Поместится ли предмет в сумку: не больше лимита на ячейку и есть куда класть.
    pub fn can_add_item(&self, item_id: &str, quantity: u32) -> bool {
        if quantity == 0 {
            return true;
        }
        if items::item(item_id).is_none() {
            return false;
        }
        self.item_capacity_left(item_id) >= quantity && self.inventory_free_slots() > 0
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
    pub fn equip_item(&mut self, item_id: &str) -> Result<(), EquipError> {
        let item = items::item(item_id).ok_or(EquipError::UnknownItem)?;
        let slot = item.slot.ok_or(EquipError::NotEquippable)?;
        if self.item_count(item_id) == 0 {
            return Err(EquipError::NotOwned);
        }
        if self.equipment.get(slot) == Some(item_id) {
            return Err(EquipError::AlreadyEquipped);
        }
        if self.equipment.get(slot).is_some() {
            return Err(EquipError::SlotOccupied);
        }
        self.equipment.set(slot, Some(item_id.to_string()));
        self.recalculate_stats();
        Ok(())
    }

    /// Снимает предмет из слота.
    pub fn unequip_slot(&mut self, slot: EquipSlot) {
        self.equipment.clear(slot);
        self.recalculate_stats();
    }

    /// Переключает активный набор экипировки, сохраняя текущий.
    pub fn switch_set(&mut self, index: usize) {
        if index >= self.equipment_sets.len() || index == self.active_set {
            return;
        }
        self.equipment_sets[self.active_set] = self.equipment.clone();
        self.active_set = index;
        self.equipment = self.equipment_sets[index].clone();
        self.ensure_equipment_owned();
    }

    /// Пересчитывает бонусы от экипировки.
    pub fn recalculate_stats(&mut self) {
        let mut trade_bonus = 0.0;
        let mut expedition_bonus = 0.0;
        let mut gather_bonus = 0.0;
        let mut fire_discount = 0.0;
        let mut expedition_energy_discount = 0.0;
        for slot in EquipSlot::ALL {
            if let Some(item) = self.equipped_item(slot) {
                let stats = &item.stats;
                trade_bonus += stats.trade_bonus.unwrap_or(0.0) / 100.0;
                trade_bonus += stats.gold_bonus.unwrap_or(0.0) / 100.0;
                expedition_bonus += stats.expedition_bonus.unwrap_or(0.0) / 100.0;
                gather_bonus += stats.wood_bonus.unwrap_or(0.0) / 100.0;
                gather_bonus += stats.stone_bonus.unwrap_or(0.0) / 100.0;
                gather_bonus += stats.ore_bonus.unwrap_or(0.0) / 100.0;
                gather_bonus += stats.fiber_bonus.unwrap_or(0.0) / 100.0;
                trade_bonus += stats
                    .trade_bonus
                    .map(|_| stats.gold_bonus.unwrap_or(0.0) / 100.0)
                    .unwrap_or(0.0);
                fire_discount += stats.fire_discount.unwrap_or(0.0) / 100.0;
                expedition_energy_discount +=
                    stats.expedition_energy_discount.unwrap_or(0.0) / 100.0;
            }
        }
        self.trade_bonus = trade_bonus.clamp(0.0, 5.0);
        self.expedition_bonus = expedition_bonus.clamp(0.0, 5.0);
        self.gather_bonus = gather_bonus.clamp(0.0, 5.0);
        self.fire_discount = fire_discount.clamp(0.0, 0.9);
        self.expedition_energy_discount = expedition_energy_discount.clamp(0.0, 0.9);
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

    /// Снимает экипировку, которой больше нет в сумке.
    fn ensure_equipment_owned(&mut self) {
        for slot in EquipSlot::ALL {
            let missing = self
                .equipment
                .get(slot)
                .map(|item_id| self.item_count(item_id) == 0)
                .unwrap_or(false);
            if missing {
                self.equipment.clear(slot);
            }
        }
        if let Some(set) = self.equipment_sets.get_mut(self.active_set) {
            *set = self.equipment.clone();
        }
        self.recalculate_stats();
    }

    /// Достижения с прогрессом.
    pub fn achievements(&self) -> Vec<AchievementStatus> {
        fn make(
            name: &'static str,
            description: &'static str,
            value: f64,
            target: f64,
            unit: &str,
        ) -> AchievementStatus {
            let ratio = if target <= 0.0 {
                1.0
            } else {
                (value / target).clamp(0.0, 1.0)
            };
            AchievementStatus {
                name,
                description,
                progress: ratio,
                done: value + 1e-9 >= target,
                value_text: format!("{}/{} {unit}", value.floor(), target.floor()),
            }
        }

        vec![
            make(
                "Первые шаги",
                "Отдохни 10 раз и открой добычу.",
                self.stats.rests as f64,
                10.0,
                "отдыхов",
            ),
            make(
                "Дровосек",
                "Добудь 100 дерева за всё время.",
                self.stats.wood_gathered,
                100.0,
                "дерева",
            ),
            make(
                "Каменотёс",
                "Добудь 100 камня за всё время.",
                self.stats.stone_gathered,
                100.0,
                "камня",
            ),
            make(
                "Угольный мастер",
                "Получи 100 угля за всё время.",
                self.stats.coal_produced,
                100.0,
                "угля",
            ),
            make(
                "Шахтёр",
                "Добудь 50 железа за всё время.",
                self.stats.iron_mined,
                50.0,
                "железа",
            ),
            make(
                "Богач",
                "Заработай 100 золота на торговле.",
                self.stats.gold_earned,
                100.0,
                "золота",
            ),
            make(
                "Первый цикл",
                "Начни новый цикл после кузницы.",
                self.cycles as f64,
                1.0,
                "циклов",
            ),
            make(
                "Хозяин",
                "Получи 5 рабочих мест.",
                self.worker_slots() as f64,
                5.0,
                "мест",
            ),
            make(
                "Путешественник во времени",
                "Накопи 60 времени.",
                self.time,
                60.0,
                "времени",
            ),
        ]
    }

    /// Сбрасывает прогресс цикла, сохраняя циклы, время и статистику.
    pub fn reset_for_cycle(&mut self) {
        let theme = self.theme;
        let stats = self.stats.clone();
        let cycles = self.cycles + 1;
        let time = self.time;
        let last_seen = self.last_seen;
        let rest_clicks = self.rest_clicks;
        let wood_seen = self.wood_seen;
        let stone_seen = self.stone_seen;
        let dev = std::mem::take(&mut self.dev);

        *self = GameState::default();
        self.gold = 0.0;
        self.inventory.clear();
        self.equipment = EquippedItems::default();
        self.recalculate_stats();
        self.theme = theme;
        self.stats = stats;
        self.cycles = cycles;
        self.time = time;
        self.last_seen = last_seen;
        self.rest_clicks = rest_clicks;
        self.wood_seen = wood_seen;
        self.stone_seen = stone_seen;
        self.dev = dev;
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
    pub fn add_resource(&mut self, kind: ResourceKind, amount: f64) -> f64 {
        let room = (self.cap(kind) - self.amount(kind)).max(0.0);
        let added = room.min(amount.max(0.0));

        match kind {
            ResourceKind::Gold => {
                self.gold = (self.gold + added).min(self.gold_cap());
            }
            ResourceKind::Fire => {
                self.current_fire = (self.current_fire as f64 + added).min(self.cap(kind)) as u32;
            }
            _ => {
                let added_u32 = added.max(0.0) as u32;
                if added_u32 > 0 {
                    self.add_active(Self::resource_key(kind), added_u32);
                }
            }
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
        for removed in ["coal_amulet", "iron_armor"] {
            self.inventory.remove(removed);
        }
        // Одна ячейка — не больше лимита штук.
        for quantity in self.inventory.values_mut() {
            *quantity = (*quantity).min(items::ITEM_STACK_LIMIT);
        }
        self.inventory.retain(|_, quantity| *quantity > 0);
        self.ensure_markets();
        if self.active_set >= self.equipment_sets.len() {
            self.active_set = 0;
        }
        self.ensure_equipment_owned();
        self.drop_trader_research();
        self.recalculate_stats();
    }

    /// Приводит рынки в соответствие с каталогом торговцев.
    fn ensure_markets(&mut self) {
        if self.traders.len() != trader::TRADERS.len() {
            self.traders = trader::default_markets();
            return;
        }
        for (index, market) in self.traders.iter_mut().enumerate() {
            if market.goods.len() != trader::TRADERS[index].goods.len() {
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
        "deep_mines"
    }

    /// `true`, если это последняя открытая локация.
    pub fn is_current_location(&self, location_id: &str) -> bool {
        self.current_location() == location_id
    }

    /// Есть ли у игрока требуемый локации предмет (кирка, свеча).
    pub fn meets_location_requirement(
        &self,
        requirement: Option<(&'static str, &'static str)>,
    ) -> bool {
        match requirement {
            None => true,
            Some((item_id, _)) => self.item_count(item_id) > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn location_chain_unlocks_by_map() {
        let state = GameState::default();
        assert!(state.location_unlocked("field"));
        assert!(state.location_unlocked("ravine"));
        assert!(state.location_unlocked("deep_forest"));
        assert!(state.location_unlocked("deep_mines"));
        assert!(!state.location_unlocked("forgotten_quarry"));
        assert!(!state.location_unlocked("ancient_ruins"));
        assert_eq!(state.current_location(), "deep_mines");
    }

    #[test]
    fn fire_goes_green_with_forge_level() {
        let mut state = GameState {
            forge_level: 1,
            ..GameState::default()
        };
        let base_max = state.fire_max();
        let base_coal = state.fire_per_coal();
        state.forge_level = 3;
        assert_eq!(
            state.fire_max(),
            base_max + 2 * balance::FORGE_FIRE_PER_LEVEL
        );
        assert_eq!(
            state.fire_per_coal(),
            base_coal + 2 * balance::FIRE_PER_COAL_PER_LEVEL
        );
    }
}
