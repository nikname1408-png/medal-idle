use super::balance;

/// Виды ресурсов, которые можно добывать кликом.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Всё состояние игры. Логика не зависит от Dioxus.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub energy: f64,
    pub energy_max: f64,
    pub energy_regen: f64,

    pub wood: f64,
    pub stone: f64,
    pub iron: f64,
    pub coal: f64,
    pub gold: f64,

    pub wood_cap: f64,
    pub stone_cap: f64,
    pub iron_cap: f64,
    pub coal_cap: f64,
    pub gold_cap: f64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            energy: balance::START_ENERGY,
            energy_max: balance::START_ENERGY_MAX,
            energy_regen: balance::START_ENERGY_REGEN,
            wood: 0.0,
            stone: 0.0,
            iron: 0.0,
            coal: 0.0,
            gold: 0.0,
            wood_cap: balance::BASE_STORAGE_CAP,
            stone_cap: balance::BASE_STORAGE_CAP,
            iron_cap: balance::BASE_STORAGE_CAP,
            coal_cap: balance::BASE_STORAGE_CAP,
            gold_cap: balance::BASE_STORAGE_CAP,
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

    pub fn cap(&self, kind: ResourceKind) -> f64 {
        match kind {
            ResourceKind::Wood => self.wood_cap,
            ResourceKind::Stone => self.stone_cap,
            ResourceKind::Iron => self.iron_cap,
        }
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
}
