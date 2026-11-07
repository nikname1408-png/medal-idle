use serde::{Deserialize, Serialize};

use super::balance;

/// Виды ресурсов.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Wood,
    Stone,
    Iron,
    Coal,
}

impl ResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wood => "дерево",
            Self::Stone => "камень",
            Self::Iron => "железо",
            Self::Coal => "уголь",
        }
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
    pub gold: f64,

    /// Сколько раз игрок нажимал «Отдых».
    pub rest_clicks: u32,
    /// Уровень своего дома (0 — ещё не построен).
    pub house_level: u32,
    /// Уровень склада.
    pub storage_level: u32,
    /// Уровень дома для работников (каждый уровень — один работник).
    pub worker_house_level: u32,
    /// Уровень дома для изучения (каждый уровень — один исследователь).
    pub research_house_level: u32,
    /// Уровень лесопилки (каждый уровень — одно рабочее место).
    pub sawmill_level: u32,
    /// Уровень угольной ямы (открывается исследованием).
    pub coal_pit_level: u32,

    /// Сколько работников назначено на лесопилку.
    pub sawmill_workers: u32,
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
        Self {
            version: balance::SAVE_VERSION,
            energy: balance::START_ENERGY,
            energy_max: balance::START_ENERGY_MAX,
            wood: 0.0,
            stone: 0.0,
            iron: 0.0,
            coal: 0.0,
            gold: 0.0,
            rest_clicks: 0,
            house_level: 0,
            storage_level: 0,
            worker_house_level: 0,
            research_house_level: 0,
            sawmill_level: 0,
            coal_pit_level: 0,
            sawmill_workers: 0,
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
        }
    }
}

impl GameState {
    pub fn amount(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Wood => self.wood,
            ResourceKind::Stone => self.stone,
            ResourceKind::Iron => self.iron,
            ResourceKind::Coal => self.coal,
        }
    }

    /// Лимит ресурса с учётом склада.
    pub fn cap(&self, _kind: ResourceKind) -> f64 {
        balance::BASE_STORAGE_CAP + self.storage_level as f64 * balance::STORAGE_CAP_PER_LEVEL
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
        self.sawmill_workers + self.coal_pit_workers
    }

    /// Сколько работников свободно.
    pub fn workers_free(&self) -> u32 {
        self.worker_slots().saturating_sub(self.workers_assigned())
    }

    /// Сколько исследователей занято.
    pub fn researchers_assigned(&self) -> u32 {
        self.coal_pit_researchers
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

    /// Сколько рабочих мест даёт угольная яма.
    pub fn coal_pit_slots(&self) -> u32 {
        self.coal_pit_level
    }

    /// Дерево в секунду с лесопилки.
    pub fn sawmill_wood_per_sec(&self) -> f64 {
        self.sawmill_workers as f64 * balance::SAWMILL_WOOD_PER_WORKER_PER_SEC
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

    /// Опыт исследования угольной ямы в секунду.
    pub fn coal_pit_research_speed(&self) -> f64 {
        self.coal_pit_researchers as f64 * balance::RESEARCH_PER_RESEARCHER_PER_SEC
    }

    /// Доля изучения угольной ямы от 0.0 до 1.0.
    pub fn coal_pit_research_ratio(&self) -> f64 {
        if balance::COAL_PIT_RESEARCH_COST <= 0.0 {
            1.0
        } else {
            (self.coal_pit_research / balance::COAL_PIT_RESEARCH_COST).clamp(0.0, 1.0)
        }
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
            ResourceKind::Wood => self.wood += added,
            ResourceKind::Stone => self.stone += added,
            ResourceKind::Iron => self.iron += added,
            ResourceKind::Coal => self.coal += added,
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
        self.energy = (self.energy + amount.max(0.0)).min(self.energy_max);
        self.energy - before
    }

    /// Доля заполнения полосы энергии от 0.0 до 1.0.
    pub fn energy_ratio(&self) -> f64 {
        if self.energy_max <= 0.0 {
            0.0
        } else {
            (self.energy / self.energy_max).clamp(0.0, 1.0)
        }
    }

    /// Обновляет флаги открытия по текущим ресурсам. Флаги не откатываются.
    pub fn refresh_unlocks(&mut self) {
        if self.wood >= balance::WOOD_VISIBLE_FROM {
            self.wood_seen = true;
        }
        if self.wood >= balance::STONE_UNLOCK_WOOD {
            self.stone_seen = true;
        }
        if self.coal > 0.0 {
            self.coal_seen = true;
        }
    }
}
