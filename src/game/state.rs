use serde::{Deserialize, Serialize};

use super::balance;

/// Виды ресурсов, которые можно добывать кликом.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Wood,
    Stone,
    Iron,
}

impl ResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wood => "дерево",
            Self::Stone => "камень",
            Self::Iron => "железо",
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

    /// Значок дерева уже показывался в баре.
    pub wood_seen: bool,
    /// Открыты ли камень, дом и склад.
    pub stone_seen: bool,

    pub theme: Theme,
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
            wood_seen: false,
            stone_seen: false,
            theme: Theme::Classic,
        }
    }
}

impl GameState {
    pub fn amount(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Wood => self.wood,
            ResourceKind::Stone => self.stone,
            ResourceKind::Iron => self.iron,
        }
    }

    /// Лимит ресурса с учётом склада.
    pub fn cap(&self, _kind: ResourceKind) -> f64 {
        balance::BASE_STORAGE_CAP + self.storage_level as f64 * balance::STORAGE_CAP_PER_LEVEL
    }

    /// Регенерация энергии в секунду. Без дома — ноль.
    pub fn energy_regen(&self) -> f64 {
        self.house_level as f64 * balance::HOUSE_ENERGY_PER_LEVEL
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
    }
}
