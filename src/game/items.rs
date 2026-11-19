//! Каталог предметов и рецептов.
//!
//! Каталог статический и не зависит от Dioxus. Состояние игрока хранит только
//! ID предметов и количества, поэтому новые товары добавляются здесь.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::balance;
use super::i18n;
use super::i18n::catalog;

/// Множитель пола цены по тиру товара: чем сложнее вещь, тем выше наценка
/// над себестоимостью. Слитки считаются материалом, а не снаряжением.
/// Нужен только тесту `every_recipe_price_covers_floor`.
#[cfg(test)]
pub fn recipe_price_multiplier(item_id: &str) -> f64 {
    use super::trader::{tier_of, Tier};
    if matches!(item_id, "iron_ingot" | "silver_ingot" | "gold_ingot") {
        return balance::PRICE_FLOOR_MATERIALS;
    }
    match tier_of(item_id) {
        Tier::Bulk => balance::PRICE_FLOOR_MATERIALS,
        Tier::CheapGear => balance::PRICE_FLOOR_CHEAP,
        Tier::MidGear => balance::PRICE_FLOOR_MID,
        Tier::HeavyGear => balance::PRICE_FLOOR_HEAVY,
        Tier::Epic => balance::PRICE_FLOOR_EPIC,
    }
}

/// Базовое число ячеек инвентаря без экипировки.
pub const BASE_INVENTORY_SLOTS: u32 = 10;

/// Сколько штук одного предмета помещается в одну ячейку.
pub const ITEM_STACK_LIMIT: u32 = 9_999;

/// Чертёж паровой машины — особая находка Древних руин.
pub const STEAM_BLUEPRINT_ID: &str = "steam_blueprint";
/// Прототип паровой машины: результат крафта и ключ к Эпохе Пара.
pub const STEAM_PROTOTYPE_ID: &str = "steam_prototype";
/// ID рецепта прототипа (совпадает с ID предмета).
pub const STEAM_PROTOTYPE_RECIPE: &str = "steam_prototype";

/// Чертёж компаса — награда Мартина за вступление в город.
pub const COMPASS_BLUEPRINT_ID: &str = "compass_blueprint";
/// Компас — ключевой предмет сюжетной цепочки города.
pub const COMPASS_ID: &str = "compass";
/// ID рецепта компаса (совпадает с ID предмета).
pub const COMPASS_RECIPE: &str = "compass";

/// Категория предмета.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Material,
    Gear,
    Jewelry,
    Goods,
    Consumable,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Material => i18n::t("cat.material"),
            Self::Gear => i18n::t("cat.gear"),
            Self::Jewelry => i18n::t("cat.jewelry"),
            Self::Goods => i18n::t("cat.goods"),
            Self::Consumable => i18n::t("cat.consumable"),
        }
    }
}

/// Слот экипировки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    Hand,
    Body,
    Back,
    Head,
    Feet,
    Accessory1,
    Accessory2,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 7] = [
        Self::Head,
        Self::Body,
        Self::Back,
        Self::Hand,
        Self::Feet,
        Self::Accessory1,
        Self::Accessory2,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hand => i18n::t("slot.hand"),
            Self::Body => i18n::t("slot.body"),
            Self::Back => i18n::t("slot.back"),
            Self::Head => i18n::t("slot.head"),
            Self::Feet => i18n::t("slot.feet"),
            Self::Accessory1 => i18n::t("slot.accessory1"),
            Self::Accessory2 => i18n::t("slot.accessory2"),
        }
    }
}

/// Бонусы предмета. `None` — бонуса нет.
///
/// Хранятся сырыми процентами (20 = +20%), а тянет их в игру `state.rs`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemStats {
    pub inventory_slots: Option<u32>,
    pub trade_bonus: Option<f32>,
    pub max_energy: Option<u32>,
    pub expedition_bonus: Option<f32>,
    /// +% к добыче ресурсов (общий для дерева, камня, руды, волокна).
    pub gather_bonus: Option<f32>,
    /// +% к золоту за продажу.
    pub gold_bonus: Option<f32>,
    /// +% к шансу редкой находки в походах.
    pub rare_loot_bonus: Option<f32>,
    /// −% к расходу огня кузницы.
    pub fire_discount: Option<f32>,
    /// −% к расходу энергии в походах.
    pub expedition_energy_discount: Option<f32>,
    /// +% к опыту кузнеца.
    pub smith_xp_bonus: Option<f32>,
    /// +% к шансу успешного торга.
    pub haggle_bonus: Option<f32>,
    /// % шанс не потратить ингредиент при крафте.
    pub ingredient_discount: Option<f32>,
    /// ID сета, к которому принадлежит предмет (`None` — вне сета).
    pub set_id: Option<String>,
    /// Особый эффект текстом (например, открывает шахту).
    pub unlock: Option<String>,
}

impl ItemStats {
    /// Строка бонусов для карточки предмета.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        let mut add = |key: &str, value: String| parts.push(i18n::tf(key, &[&value]));
        if let Some(slots) = self.inventory_slots {
            add("stats.inventory_slots", slots.to_string());
        }
        if let Some(trade) = self.trade_bonus {
            add("stats.trade_bonus", format!("{trade:.0}"));
        }
        if let Some(energy) = self.max_energy {
            add("stats.max_energy", energy.to_string());
        }
        if let Some(expedition) = self.expedition_bonus {
            add("stats.expedition_bonus", format!("{expedition:.0}"));
        }
        if let Some(gather) = self.gather_bonus {
            add("stats.gather_bonus", format!("{gather:.0}"));
        }
        if let Some(gold) = self.gold_bonus {
            add("stats.gold_bonus", format!("{gold:.0}"));
        }
        if let Some(rare) = self.rare_loot_bonus {
            add("stats.rare_loot_bonus", format!("{rare:.0}"));
        }
        if let Some(fire) = self.fire_discount {
            add("stats.fire_discount", format!("{fire:.0}"));
        }
        if let Some(energy) = self.expedition_energy_discount {
            add("stats.expedition_energy_discount", format!("{energy:.0}"));
        }
        if let Some(smith) = self.smith_xp_bonus {
            add("stats.smith_xp_bonus", format!("{smith:.0}"));
        }
        if let Some(haggle) = self.haggle_bonus {
            add("stats.haggle_bonus", format!("{haggle:.0}"));
        }
        if let Some(discount) = self.ingredient_discount {
            add("stats.ingredient_discount", format!("{discount:.0}"));
        }
        if let Some(unlock) = &self.unlock {
            parts.push(unlock.clone());
        }
        if parts.is_empty() {
            i18n::t("stats.none").to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Предмет каталога.
///
/// Название и описание в данных не хранятся: они берутся из словаря
/// (`game::i18n`) по `id`, поэтому меняются вместе с языком интерфейса.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub category: Category,
    pub base_price: u32,
    pub stats: ItemStats,
    /// Слот экипировки. `None` — предмет нельзя надеть.
    pub slot: Option<EquipSlot>,
    /// Стоимость огня кузницы, если предмет требует огонь при крафте.
    pub fire_cost: u32,
}

impl Item {
    /// Название на текущем языке.
    pub fn name(&self) -> &'static str {
        catalog::name(self.id.as_str())
    }

    /// Описание на текущем языке.
    pub fn description(&self) -> &'static str {
        catalog::description(self.id.as_str())
    }

    /// Условная редкость для карточки товара.
    pub fn rarity(&self) -> &'static str {
        match self.base_price {
            0..=29 => i18n::t("rarity.common"),
            30..=99 => i18n::t("rarity.uncommon"),
            100..=179 => i18n::t("rarity.rare"),
            _ => i18n::t("rarity.valuable"),
        }
    }
}

/// Ингредиент рецепта.
#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    pub item_id: &'static str,
    pub quantity: u32,
}

/// Рецепт крафта.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: &'static str,
    pub ingredients: Vec<Ingredient>,
    pub result_id: &'static str,
    pub result_quantity: u32,
    /// Стоимость огня кузницы для крафта.
    pub fire_cost: u32,
}

impl Recipe {
    /// Название рецепта: имя его результата.
    pub fn name(&self) -> &'static str {
        item(self.result_id)
            .map(Item::name)
            .unwrap_or(self.result_id)
    }
}

/// Каталог всех предметов.
pub fn catalog() -> &'static [Item] {
    static CATALOG: OnceLock<Vec<Item>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Все рецепты крафта.
pub fn recipes() -> &'static [Recipe] {
    static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();
    RECIPES.get_or_init(|| {
        let mut list = build_recipes();
        // Сортируем: сначала по группам (материалы, потом инструменты, потом перчатки),
        // а внутри группы — по возрастанию требуемого уровня кузнеца.
        list.sort_by(|a, b| {
            let group_a = recipe_group_order(a.id);
            let group_b = recipe_group_order(b.id);
            if group_a != group_b {
                return group_a.cmp(&group_b);
            }
            let level_a = recipe_min_smith_level(a.id);
            let level_b = recipe_min_smith_level(b.id);
            level_a.cmp(&level_b)
        });
        list
    })
}

/// Предмет по ID.
pub fn item(id: &str) -> Option<&'static Item> {
    catalog().iter().find(|item| item.id == id)
}

/// Рецепт по ID.
pub fn recipe(id: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|recipe| recipe.id == id)
}

fn build_catalog() -> Vec<Item> {
    vec![
        // ---------- Складские ресурсы ----------
        item_def("wood", Category::Material, 2, None),
        item_def("stone", Category::Material, 2, None),
        item_def("iron", Category::Material, 5, None),
        item_def("coal", Category::Material, 3, None),
        // ---------- Инвентарные ресурсы ----------
        item_def("clay", Category::Material, 3, None),
        item_def("leather", Category::Material, 8, None),
        item_def("fiber", Category::Material, 2, None),
        item_def("quartz", Category::Material, 10, None),
        item_def("silver", Category::Material, 30, None),
        item_def("gold_ore", Category::Material, 150, None),
        item_def("gems", Category::Material, 100, None),
        // ---------- Карты локаций ----------
        item_def("map_forest", Category::Material, 500, None),
        item_def("map_mines", Category::Material, 1500, None),
        item_def("map_quarry", Category::Material, 5000, None),
        item_def("map_ruins", Category::Material, 10000, None),
        item_def(STEAM_BLUEPRINT_ID, Category::Material, 5000, None),
        item_def(STEAM_PROTOTYPE_ID, Category::Material, 0, None),
        item_def(COMPASS_BLUEPRINT_ID, Category::Material, 5000, None),
        item_def(COMPASS_ID, Category::Material, 1500, None),
        // ---------- Промежуточные материалы ----------
        item_def("board", Category::Material, 8, None),
        item_def("cloth", Category::Material, 11, None),
        item_def("whetstone", Category::Material, 26, None),
        item_def("brick", Category::Goods, 16, None).with_fire_cost(10),
        item_def("stone_block", Category::Goods, 40, None),
        item_def("iron_ingot", Category::Material, 24, None).with_fire_cost(10),
        item_def("gold_ingot", Category::Material, 528, None).with_fire_cost(30),
        item_def("silver_ingot", Category::Material, 96, None).with_fire_cost(20),
        // ---------- Экипировка и инструменты ----------
        item_def("backpack", Category::Gear, 51, Some(EquipSlot::Back)).with_stats(ItemStats {
            inventory_slots: Some(10),
            ..ItemStats::default()
        }),
        item_def("robe", Category::Gear, 88, Some(EquipSlot::Body)).with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            set_id: Some("beggar".to_string()),
            ..ItemStats::default()
        }),
        item_def("leather_gloves", Category::Gear, 43, Some(EquipSlot::Hand)).with_stats(
            ItemStats {
                gather_bonus: Some(10.0),
                ..ItemStats::default()
            },
        ),
        item_def("iron_bracers", Category::Gear, 239, Some(EquipSlot::Hand))
            .with_stats(ItemStats {
                fire_discount: Some(5.0),
                ..ItemStats::default()
            })
            .with_fire_cost(40),
        item_def("winter_gloves", Category::Gear, 91, Some(EquipSlot::Hand)).with_stats(
            ItemStats {
                expedition_energy_discount: Some(10.0),
                ..ItemStats::default()
            },
        ),
        item_def("jacket", Category::Gear, 187, Some(EquipSlot::Body)).with_stats(ItemStats {
            expedition_energy_discount: Some(15.0),
            ..ItemStats::default()
        }),
        // ---------- Инструменты для фермы ----------
        item_def("wooden_bucket", Category::Goods, 75, None),
        item_def("iron_bucket", Category::Goods, 95, None).with_fire_cost(15),
        item_def("sickle", Category::Gear, 161, Some(EquipSlot::Hand))
            .with_stats(ItemStats {
                gather_bonus: Some(20.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            })
            .with_fire_cost(30),
        item_def("scythe", Category::Gear, 177, Some(EquipSlot::Hand))
            .with_stats(ItemStats {
                gather_bonus: Some(40.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            })
            .with_fire_cost(35),
        // ---------- Товары на продажу ----------
        item_def("jug", Category::Goods, 11, None),
        item_def("candle", Category::Goods, 10, None),
        // ---------- Головные уборы и шлемы ----------
        item_def("wretched_hat", Category::Gear, 70, Some(EquipSlot::Head)).with_stats(ItemStats {
            max_energy: Some(5),
            set_id: Some("beggar".to_string()),
            ..ItemStats::default()
        }),
        item_def("leather_hat", Category::Gear, 45, Some(EquipSlot::Head)).with_stats(ItemStats {
            gather_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def("war_hat", Category::Gear, 91, Some(EquipSlot::Head)).with_stats(ItemStats {
            gather_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def("iron_helmet", Category::Gear, 315, Some(EquipSlot::Head))
            .with_stats(ItemStats {
                max_energy: Some(15),
                fire_discount: Some(5.0),
                ..ItemStats::default()
            })
            .with_fire_cost(20),
        item_def("coif", Category::Gear, 450, Some(EquipSlot::Head))
            .with_stats(ItemStats {
                expedition_energy_discount: Some(8.0),
                ..ItemStats::default()
            })
            .with_fire_cost(50),
        item_def("wanderers_hood", Category::Gear, 88, Some(EquipSlot::Head)).with_stats(
            ItemStats {
                expedition_bonus: Some(8.0),
                ..ItemStats::default()
            },
        ),
        item_def("armet", Category::Gear, 1581, Some(EquipSlot::Head))
            .with_stats(ItemStats {
                max_energy: Some(20),
                trade_bonus: Some(8.0),
                gold_bonus: Some(8.0),
                ..ItemStats::default()
            })
            .with_fire_cost(150),
        // ---------- Броня ----------
        item_def("iron_armor", Category::Gear, 1600, Some(EquipSlot::Body))
            .with_stats(ItemStats {
                max_energy: Some(30),
                fire_discount: Some(10.0),
                ..ItemStats::default()
            })
            .with_fire_cost(200),
        item_def("archers_jacket", Category::Gear, 126, Some(EquipSlot::Body)).with_stats(
            ItemStats {
                expedition_bonus: Some(10.0),
                gather_bonus: Some(5.0),
                ..ItemStats::default()
            },
        ),
        // ---------- Пояса ----------
        item_def(
            "wanderers_belt",
            Category::Gear,
            84,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            inventory_slots: Some(3),
            trade_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        item_def(
            "belt_with_bag",
            Category::Gear,
            169,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            inventory_slots: Some(5),
            expedition_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        // ---------- Щиты ----------
        item_def("wooden_shield", Category::Gear, 112, None),
        item_def("norman_shield", Category::Gear, 275, None).with_fire_cost(50),
        item_def("iron_shield", Category::Gear, 607, None),
        // ---------- Плащи ----------
        item_def("beggars_cloak", Category::Gear, 88, Some(EquipSlot::Back)).with_stats(
            ItemStats {
                inventory_slots: Some(2),
                trade_bonus: Some(2.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            },
        ),
        item_def("leather_coat", Category::Gear, 117, Some(EquipSlot::Back)).with_stats(
            ItemStats {
                expedition_energy_discount: Some(5.0),
                gather_bonus: Some(5.0),
                ..ItemStats::default()
            },
        ),
        item_def(
            "researchers_cloak",
            Category::Gear,
            161,
            Some(EquipSlot::Back),
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            inventory_slots: Some(4),
            ..ItemStats::default()
        }),
        item_def(
            "travelers_cloak",
            Category::Gear,
            989,
            Some(EquipSlot::Back),
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(15.0),
            expedition_energy_discount: Some(8.0),
            trade_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        // ---------- Перчатки ----------
        item_def("beggars_gloves", Category::Gear, 70, Some(EquipSlot::Hand)).with_stats(
            ItemStats {
                gather_bonus: Some(8.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            },
        ),
        item_def(
            "chainmail_gloves",
            Category::Gear,
            386,
            Some(EquipSlot::Hand),
        )
        .with_stats(ItemStats {
            fire_discount: Some(10.0),
            gather_bonus: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(100),
        // ---------- Обувь ----------
        item_def("poor_boots", Category::Gear, 100, Some(EquipSlot::Feet)).with_stats(ItemStats {
            expedition_energy_discount: Some(3.0),
            set_id: Some("beggar".to_string()),
            ..ItemStats::default()
        }),
        item_def("leather_boots", Category::Gear, 100, Some(EquipSlot::Feet)).with_stats(
            ItemStats {
                expedition_energy_discount: Some(5.0),
                expedition_bonus: Some(3.0),
                ..ItemStats::default()
            },
        ),
        item_def("hiking_boots", Category::Gear, 160, Some(EquipSlot::Feet)).with_stats(
            ItemStats {
                expedition_energy_discount: Some(8.0),
                expedition_bonus: Some(5.0),
                ..ItemStats::default()
            },
        ),
        item_def("iron_boots", Category::Gear, 730, Some(EquipSlot::Feet))
            .with_stats(ItemStats {
                expedition_energy_discount: Some(10.0),
                max_energy: Some(10),
                ..ItemStats::default()
            })
            .with_fire_cost(150),
        item_def("winter_boots", Category::Gear, 152, Some(EquipSlot::Feet)).with_stats(
            ItemStats {
                expedition_energy_discount: Some(10.0),
                gather_bonus: Some(3.0),
                ..ItemStats::default()
            },
        ),
        // ---------- Инструменты ----------
        item_def("wooden_shovel", Category::Gear, 56, Some(EquipSlot::Hand)).with_stats(
            ItemStats {
                gather_bonus: Some(15.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            },
        ),
        item_def("iron_shovel", Category::Gear, 188, Some(EquipSlot::Hand))
            .with_stats(ItemStats {
                gather_bonus: Some(25.0),
                set_id: Some("beggar".to_string()),
                ..ItemStats::default()
            })
            .with_fire_cost(20),
        item_def("hammer", Category::Gear, 80, None).with_fire_cost(15),
        item_def("chainmail", Category::Goods, 787, Some(EquipSlot::Body))
            .with_stats(ItemStats {
                fire_discount: Some(10.0),
                expedition_energy_discount: Some(8.0),
                ..ItemStats::default()
            })
            .with_fire_cost(200),
        // ---------- Бижутерия ----------
        item_def(
            "silver_ring",
            Category::Jewelry,
            187,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            trade_bonus: Some(3.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "trader_ring",
            Category::Jewelry,
            206,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            trade_bonus: Some(10.0),
            gold_bonus: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "miner_amulet",
            Category::Jewelry,
            206,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            gather_bonus: Some(15.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "wanderer_pendant",
            Category::Jewelry,
            377,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            expedition_energy_discount: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "gold_ring",
            Category::Jewelry,
            1275,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            gold_bonus: Some(20.0),
            trade_bonus: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(60),
        item_def(
            "gold_pendant",
            Category::Jewelry,
            1480,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            trade_bonus: Some(10.0),
            gold_bonus: Some(10.0),
            ..ItemStats::default()
        })
        .with_fire_cost(80),
        // ---------- Сет Кузнеца ----------
        item_def(
            "smith_ring",
            Category::Jewelry,
            281,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            smith_xp_bonus: Some(10.0),
            haggle_bonus: Some(5.0),
            set_id: Some("smith".to_string()),
            ..ItemStats::default()
        })
        .with_fire_cost(25),
        item_def(
            "smith_amulet",
            Category::Jewelry,
            228,
            Some(EquipSlot::Accessory1),
        )
        .with_stats(ItemStats {
            smith_xp_bonus: Some(10.0),
            fire_discount: Some(5.0),
            set_id: Some("smith".to_string()),
            ..ItemStats::default()
        })
        .with_fire_cost(25),
        item_def("smith_belt", Category::Gear, 98, Some(EquipSlot::Body))
            .with_stats(ItemStats {
                ingredient_discount: Some(10.0),
                set_id: Some("smith".to_string()),
                ..ItemStats::default()
            })
            .with_fire_cost(25),
    ]
}

fn item_def(id: &str, category: Category, base_price: u32, slot: Option<EquipSlot>) -> Item {
    Item {
        id: id.to_string(),
        category,
        base_price,
        stats: ItemStats::default(),
        slot,
        fire_cost: 0,
    }
}

trait WithStats {
    fn with_stats(self, stats: ItemStats) -> Item;
}

impl WithStats for Item {
    fn with_stats(mut self, stats: ItemStats) -> Item {
        self.stats = stats;
        self
    }
}

impl Item {
    fn with_fire_cost(mut self, fire_cost: u32) -> Item {
        self.fire_cost = fire_cost;
        self
    }
}

/// Ингредиент одной позиции.
fn need(item_id: &'static str, quantity: u32) -> Ingredient {
    Ingredient { item_id, quantity }
}

/// Рецепт крафта в одну строку.
fn make_recipe(
    id: &'static str,
    result_id: &'static str,
    result_quantity: u32,
    fire_cost: u32,
    ingredients: Vec<Ingredient>,
) -> Recipe {
    let mut ingredients = ingredients;
    // Крафт с огнём прямо указывает, сколько огня он тратит: в ресурсах карточки
    // горит тот же огонь, что и в реальной стоимости. Огонь ставим после
    // угля, чтобы список читался «уголь, огонь».
    if fire_cost > 0
        && !ingredients
            .iter()
            .any(|ingredient| ingredient.item_id == "fire")
    {
        ingredients.push(need("fire", fire_cost));
    }
    Recipe {
        id,
        ingredients,
        result_id,
        result_quantity,
        fire_cost,
    }
}

fn build_recipes() -> Vec<Recipe> {
    vec![
        // Промежуточные материалы.
        make_recipe("board", "board", 1, 0, vec![need("wood", 2)]),
        make_recipe("cloth", "cloth", 1, 0, vec![need("fiber", 3)]),
        make_recipe(
            "whetstone",
            "whetstone",
            1,
            0,
            vec![need("stone", 2), need("quartz", 1)],
        ),
        make_recipe("brick", "brick", 1, 0, vec![need("clay", 2)]),
        make_recipe("stone_block", "stone_block", 1, 0, vec![need("stone", 50)]),
        make_recipe("iron_ingot", "iron_ingot", 1, 10, vec![need("iron", 2)]),
        make_recipe("gold_ingot", "gold_ingot", 1, 30, vec![need("gold_ore", 2)]),
        make_recipe(
            "silver_ingot",
            "silver_ingot",
            1,
            20,
            vec![need("silver", 2)],
        ),
        // Компас для Мартина: открывается только изученным чертежом.
        make_recipe(
            COMPASS_RECIPE,
            COMPASS_ID,
            1,
            100,
            vec![
                need("gold_ingot", 1),
                need("quartz", 3),
                need("silver_ingot", 2),
            ],
        ),
        // Экипировка и инструменты.
        make_recipe(
            "backpack",
            "backpack",
            1,
            0,
            vec![need("leather", 3), need("board", 1)],
        ),
        make_recipe("robe", "robe", 1, 0, vec![need("cloth", 5)]),
        make_recipe(
            "leather_gloves",
            "leather_gloves",
            1,
            0,
            vec![need("leather", 3)],
        ),
        make_recipe(
            "iron_bracers",
            "iron_bracers",
            1,
            40,
            vec![need("iron_ingot", 5)],
        ),
        make_recipe(
            "winter_gloves",
            "winter_gloves",
            1,
            0,
            vec![need("leather", 3), need("cloth", 3)],
        ),
        make_recipe(
            "jacket",
            "jacket",
            1,
            0,
            vec![need("leather", 5), need("cloth", 7)],
        ),
        // Инструменты для фермы.
        make_recipe(
            "wooden_bucket",
            "wooden_bucket",
            1,
            0,
            vec![need("board", 5)],
        ),
        make_recipe(
            "iron_bucket",
            "iron_bucket",
            1,
            15,
            vec![need("iron_ingot", 2)],
        ),
        make_recipe(
            "sickle",
            "sickle",
            1,
            30,
            vec![need("iron_ingot", 3), need("board", 1)],
        ),
        make_recipe(
            "scythe",
            "scythe",
            1,
            35,
            vec![need("iron_ingot", 3), need("board", 2)],
        ),
        // Товары на продажу.
        make_recipe("jug", "jug", 1, 0, vec![need("clay", 2)]),
        make_recipe(
            "candle",
            "candle",
            1,
            0,
            vec![need("fiber", 1), need("coal", 1)],
        ),
        // Головные уборы и шлемы.
        make_recipe("wretched_hat", "wretched_hat", 1, 0, vec![need("cloth", 4)]),
        make_recipe("leather_hat", "leather_hat", 1, 0, vec![need("leather", 3)]),
        make_recipe(
            "war_hat",
            "war_hat",
            1,
            0,
            vec![need("leather", 3), need("iron_ingot", 1)],
        ),
        make_recipe(
            "iron_helmet",
            "iron_helmet",
            1,
            20,
            vec![need("leather", 2), need("cloth", 2), need("iron_ingot", 4)],
        ),
        make_recipe(
            "coif",
            "coif",
            1,
            50,
            vec![need("cloth", 4), need("iron_ingot", 6)],
        ),
        make_recipe(
            "wanderers_hood",
            "wanderers_hood",
            1,
            0,
            vec![need("cloth", 5)],
        ),
        make_recipe(
            "armet",
            "armet",
            1,
            150,
            vec![need("iron_ingot", 10), need("gold_ingot", 1)],
        ),
        // Броня.
        make_recipe(
            "iron_armor",
            "iron_armor",
            1,
            200,
            vec![need("iron_ingot", 25), need("leather", 5)],
        ),
        make_recipe(
            "archers_jacket",
            "archers_jacket",
            1,
            0,
            vec![need("cloth", 5), need("leather", 3)],
        ),
        // Пояса.
        make_recipe(
            "wanderers_belt",
            "wanderers_belt",
            1,
            0,
            vec![need("leather", 3), need("quartz", 2)],
        ),
        make_recipe(
            "belt_with_bag",
            "belt_with_bag",
            1,
            0,
            vec![need("leather", 7), need("cloth", 3)],
        ),
        // Щиты.
        make_recipe(
            "wooden_shield",
            "wooden_shield",
            1,
            0,
            vec![need("board", 10)],
        ),
        make_recipe(
            "norman_shield",
            "norman_shield",
            1,
            50,
            vec![need("board", 8), need("iron_ingot", 2)],
        ),
        make_recipe(
            "iron_shield",
            "iron_shield",
            1,
            0,
            vec![need("iron_ingot", 10), need("leather", 3)],
        ),
        // Плащи.
        make_recipe(
            "beggars_cloak",
            "beggars_cloak",
            1,
            0,
            vec![need("cloth", 5)],
        ),
        make_recipe(
            "leather_coat",
            "leather_coat",
            1,
            0,
            vec![need("cloth", 3), need("leather", 5)],
        ),
        make_recipe(
            "researchers_cloak",
            "researchers_cloak",
            1,
            0,
            vec![need("leather", 5), need("quartz", 3)],
        ),
        make_recipe(
            "travelers_cloak",
            "travelers_cloak",
            1,
            0,
            vec![need("leather", 15), need("cloth", 10), need("gems", 2)],
        ),
        // Перчатки.
        make_recipe(
            "beggars_gloves",
            "beggars_gloves",
            1,
            0,
            vec![need("cloth", 4)],
        ),
        make_recipe(
            "chainmail_gloves",
            "chainmail_gloves",
            1,
            100,
            vec![need("iron_ingot", 5), need("cloth", 3)],
        ),
        // Обувь.
        make_recipe(
            "poor_boots",
            "poor_boots",
            1,
            0,
            vec![need("cloth", 5), need("leather", 1)],
        ),
        make_recipe(
            "leather_boots",
            "leather_boots",
            1,
            0,
            vec![need("leather", 5), need("cloth", 2)],
        ),
        make_recipe(
            "hiking_boots",
            "hiking_boots",
            1,
            0,
            vec![need("leather", 7), need("cloth", 4)],
        ),
        make_recipe(
            "iron_boots",
            "iron_boots",
            1,
            150,
            vec![need("iron_ingot", 10), need("cloth", 5)],
        ),
        make_recipe(
            "winter_boots",
            "winter_boots",
            1,
            0,
            vec![need("leather", 5), need("cloth", 5)],
        ),
        // Инструменты.
        make_recipe(
            "wooden_shovel",
            "wooden_shovel",
            1,
            0,
            vec![need("board", 5)],
        ),
        make_recipe(
            "iron_shovel",
            "iron_shovel",
            1,
            20,
            vec![need("board", 3), need("iron_ingot", 3)],
        ),
        make_recipe(
            "hammer",
            "hammer",
            1,
            15,
            vec![need("board", 2), need("iron_ingot", 1)],
        ),
        make_recipe(
            "chainmail",
            "chainmail",
            1,
            200,
            vec![need("iron_ingot", 12), need("leather", 3)],
        ),
        // Бижутерия.
        make_recipe(
            "silver_ring",
            "silver_ring",
            1,
            15,
            vec![need("silver_ingot", 1)],
        ),
        make_recipe(
            "trader_ring",
            "trader_ring",
            1,
            15,
            vec![need("silver_ingot", 1), need("quartz", 1)],
        ),
        make_recipe(
            "miner_amulet",
            "miner_amulet",
            1,
            15,
            vec![need("silver_ingot", 1), need("quartz", 1)],
        ),
        make_recipe(
            "wanderer_pendant",
            "wanderer_pendant",
            1,
            15,
            vec![need("silver_ingot", 1), need("gems", 1)],
        ),
        make_recipe(
            "gold_ring",
            "gold_ring",
            1,
            60,
            vec![need("gold_ingot", 1), need("gems", 1)],
        ),
        make_recipe(
            "gold_pendant",
            "gold_pendant",
            1,
            80,
            vec![need("gold_ingot", 1), need("gems", 2)],
        ),
        make_recipe(
            "smith_ring",
            "smith_ring",
            1,
            25,
            vec![need("silver_ingot", 1), need("iron_ingot", 2)],
        ),
        make_recipe(
            "smith_amulet",
            "smith_amulet",
            1,
            25,
            vec![need("silver_ingot", 1), need("quartz", 2)],
        ),
        make_recipe(
            "smith_belt",
            "smith_belt",
            1,
            25,
            vec![need("leather", 3), need("iron_ingot", 1)],
        ),
        // Финал Средневековья: прототип паровой машины.
        make_recipe(
            STEAM_PROTOTYPE_RECIPE,
            STEAM_PROTOTYPE_ID,
            1,
            balance::STEAM_PROTOTYPE_FIRE,
            vec![
                need("iron_ingot", 100),
                need("silver", 50),
                need("gems", 10),
                need("quartz", 50),
            ],
        ),
    ]
}

/// Определяет группу рецепта для сортировки (Вариант А: сплошная сетка).
/// Чем меньше число, тем выше в сетке будет группа.
fn recipe_group_order(recipe_id: &str) -> u32 {
    match recipe_id {
        // 1. Материалы (сырье)
        "board" | "cloth" | "whetstone" | "brick" | "stone_block" | "iron_ingot"
        | "silver_ingot" | "gold_ingot" => 1,
        // 2. Товары и расходники
        "jug" | "candle" | "wooden_bucket" | "iron_bucket" => 2,
        // 3. Рюкзаки
        "backpack" => 3,
        // 4. Инструменты
        "wooden_shovel" | "sickle" | "scythe" | "hammer" | "iron_shovel" => 4,
        // 5. Корпус (броня, плащи, куртки)
        "robe" | "jacket" | "beggars_cloak" | "archers_jacket" | "leather_coat"
        | "researchers_cloak" | "travelers_cloak" | "chainmail" | "iron_armor" => 5,
        // 6. Перчатки
        "leather_gloves" | "winter_gloves" | "beggars_gloves" | "chainmail_gloves"
        | "iron_bracers" => 6,
        // 7. Шлемы
        "leather_hat" | "wretched_hat" | "war_hat" | "coif" | "iron_helmet" | "wanderers_hood"
        | "armet" => 7,
        // 8. Обувь
        "poor_boots" | "leather_boots" | "hiking_boots" | "winter_boots" | "iron_boots" => 8,
        // 9. Щиты
        "wooden_shield" | "norman_shield" | "iron_shield" => 9,
        // 10. Пояса
        "wanderers_belt" | "belt_with_bag" => 10,
        // 11. Кольца и амулеты
        "silver_ring" | "trader_ring" | "miner_amulet" | "wanderer_pendant" | "gold_ring"
        | "gold_pendant" => 19,
        "smith_ring" | "smith_amulet" | "smith_belt" => 14,
        // 12. Всё остальное (например, паровая машина)
        _ => 12,
    }
}

/// Минимальный уровень кузнеца для рецепта.
pub fn recipe_min_smith_level(recipe_id: &str) -> u32 {
    match recipe_id {
        // Без уровня (гейт — только кузница/огонь или чертёж)
        "iron_ingot" | STEAM_PROTOTYPE_RECIPE | COMPASS_RECIPE => 0,

        // Уровень 1: доступно сразу
        "board" | "cloth" | "whetstone" | "stone_block" | "jug" | "candle" | "brick" => 1,

        // Уровни 2-8: первая экипировка
        "wooden_bucket" => 2,
        "wooden_shovel" => 3,
        "wooden_shield" => 4,
        "backpack" => 5,
        "robe" | "leather_gloves" | "beggars_gloves" | "wretched_hat" | "beggars_cloak"
        | "poor_boots" => 6,
        "winter_gloves" => 7,
        "leather_hat" | "jacket" => 8,

        // Сет Кузнеца — с 4-го уровня: его можно собрать уже в начале игры.
        "smith_ring" | "smith_amulet" | "smith_belt" => 4,

        // Уровни 9-14: металл и кожа
        "war_hat" | "coif" | "norman_shield" | "chainmail_gloves" => 9,
        "silver_ingot" | "silver_ring" | "iron_bracers" | "wanderers_belt" => 10,
        "archers_jacket" | "iron_helmet" | "leather_boots" => 11,
        "iron_shovel" | "iron_bucket" | "hammer" | "sickle" | "scythe" | "leather_coat"
        | "researchers_cloak" | "belt_with_bag" => 12,
        "wanderers_hood" | "hiking_boots" | "winter_boots" | "iron_boots" | "travelers_cloak" => 13,
        "iron_shield" | "iron_armor" | "chainmail" => 14,

        // Уровни 15-20: эндгейм
        "trader_ring" | "miner_amulet" | "wanderer_pendant" => 16,
        "armet" => 17,
        "gold_ingot" | "gold_ring" => 18,
        "gold_pendant" => 19,

        _ => 1, // По умолчанию 1 уровень
    }
}

/// Уровень, с которого предмет можно купить у торговца по нормальной цене.
pub fn item_unlock_level(item_id: &str) -> u32 {
    recipe_min_smith_level(item_id)
}

/// Можно ли продать предмет из сумки через «Предложить товар».
pub fn is_sellable_from_bag(state: &crate::game::GameState, item_id: &str) -> bool {
    // 1. Исключаем базовые ресурсы
    if matches!(
        item_id,
        "wood"
            | "stone"
            | "iron"
            | "coal"
            | "clay"
            | "leather"
            | "fiber"
            | "quartz"
            | "silver"
            | "gold_ore"
            | "gems"
    ) {
        return false;
    }

    // 2. Исключаем доски и блоки (по просьбе)
    if item_id == "board" || item_id == "stone_block" {
        return false;
    }

    // 3. Карты: можно продавать, только если локация уже открыта
    if let Some(location_id) = crate::game::expedition::location_for_map(item_id) {
        return state.location_unlocked(location_id);
    }

    // 4. Чертёж: можно продавать, только если он изучен
    if item_id == STEAM_BLUEPRINT_ID {
        return state.steam_blueprint_studied;
    }
    if item_id == COMPASS_BLUEPRINT_ID {
        return state.compass_blueprint_studied;
    }

    // 5. Остальное (экипировка, инструменты, слитки) продавать можно
    true
}

/// Можно ли разобрать предмет обратно в ресурсы.
///
/// Разбирается любой скрафченный предмет (экипировка, инструменты, посуда),
/// кроме промежуточных материалов, карт и чертежей.
pub fn is_dismantlable(item_id: &str) -> bool {
    // Карты и чертежи — особые предметы, их разбирать нельзя.
    if item_id.starts_with("map_")
        || item_id == STEAM_BLUEPRINT_ID
        || item_id == STEAM_PROTOTYPE_ID
        || item_id == COMPASS_BLUEPRINT_ID
        || item_id == COMPASS_ID
    {
        return false;
    }
    // Промежуточные материалы — это и есть результат переработки сырья.
    if matches!(
        item_id,
        "board"
            | "cloth"
            | "whetstone"
            | "brick"
            | "stone_block"
            | "iron_ingot"
            | "silver_ingot"
            | "gold_ingot"
    ) {
        return false;
    }
    // Разбираем только то, у чего есть рецепт.
    item(item_id).is_some() && recipes().iter().any(|recipe| recipe.result_id == item_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_ids() {
        let mut ids: Vec<&str> = catalog().iter().map(|item| item.id.as_str()).collect();
        ids.sort_unstable();
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(ids, unique);
    }

    #[test]
    fn all_recipe_ingredients_exist() {
        for recipe in recipes() {
            assert!(
                item(recipe.result_id).is_some(),
                "нет результата {}",
                recipe.id
            );
            for ingredient in &recipe.ingredients {
                assert!(
                    item(ingredient.item_id).is_some() || ingredient.item_id == "fire",
                    "нет ингредиента {}",
                    ingredient.item_id
                );
            }
        }
    }

    #[test]
    fn gear_items_have_slots() {
        assert_eq!(
            item("backpack").and_then(|item| item.slot),
            Some(EquipSlot::Back)
        );
        assert_eq!(
            item("trader_ring").and_then(|item| item.slot),
            Some(EquipSlot::Accessory1)
        );
        assert_eq!(item("wood").and_then(|item| item.slot), None);
    }

    #[test]
    fn new_gear_uses_all_seven_slots() {
        let expected = [
            ("wretched_hat", EquipSlot::Head),
            ("leather_hat", EquipSlot::Head),
            ("war_hat", EquipSlot::Head),
            ("iron_helmet", EquipSlot::Head),
            ("coif", EquipSlot::Head),
            ("wanderers_hood", EquipSlot::Head),
            ("armet", EquipSlot::Head),
            ("iron_armor", EquipSlot::Body),
            ("archers_jacket", EquipSlot::Body),
            ("chainmail", EquipSlot::Body),
            // Плащи и рюкзак переехали на слот Back.
            ("backpack", EquipSlot::Back),
            ("beggars_cloak", EquipSlot::Back),
            ("leather_coat", EquipSlot::Back),
            ("researchers_cloak", EquipSlot::Back),
            ("travelers_cloak", EquipSlot::Back),
            ("wooden_shovel", EquipSlot::Hand),
            ("iron_shovel", EquipSlot::Hand),
            ("sickle", EquipSlot::Hand),
            ("scythe", EquipSlot::Hand),
            ("beggars_gloves", EquipSlot::Hand),
            ("chainmail_gloves", EquipSlot::Hand),
            ("poor_boots", EquipSlot::Feet),
            ("leather_boots", EquipSlot::Feet),
            ("hiking_boots", EquipSlot::Feet),
            ("iron_boots", EquipSlot::Feet),
            ("winter_boots", EquipSlot::Feet),
            ("silver_ring", EquipSlot::Accessory1),
            ("trader_ring", EquipSlot::Accessory1),
            ("miner_amulet", EquipSlot::Accessory1),
            ("wanderer_pendant", EquipSlot::Accessory1),
            ("gold_ring", EquipSlot::Accessory1),
            ("gold_pendant", EquipSlot::Accessory1),
            ("wanderers_belt", EquipSlot::Accessory1),
            ("belt_with_bag", EquipSlot::Accessory1),
        ];
        for (id, slot) in expected {
            assert_eq!(
                item(id).and_then(|item| item.slot),
                Some(slot),
                "неверный слот у {id}"
            );
        }
    }

    #[test]
    fn fire_costs_match_the_balance_table() {
        let expected = [
            ("brick", 10),
            ("iron_ingot", 10),
            ("gold_ingot", 30),
            ("robe", 0),
            ("iron_helmet", 20),
            ("iron_bracers", 40),
            ("iron_bucket", 15),
            ("sickle", 30),
            ("scythe", 35),
            ("chainmail", 200),
            ("coif", 50),
            ("armet", 150),
            ("iron_armor", 200),
            ("norman_shield", 50),
            ("chainmail_gloves", 100),
            ("iron_boots", 150),
            ("iron_shovel", 20),
            ("hammer", 15),
            ("silver_ring", 15),
            ("trader_ring", 15),
            ("miner_amulet", 15),
            ("wanderer_pendant", 15),
            ("gold_ring", 60),
            ("gold_pendant", 80),
        ];
        for (id, fire) in expected {
            assert_eq!(item(id).map(|item| item.fire_cost), Some(fire), "{id}");
        }
    }

    #[test]
    fn base_prices_match_the_balance_table() {
        let expected = [
            ("wood", 2),
            ("stone", 2),
            ("iron", 5),
            ("coal", 3),
            ("clay", 3),
            ("leather", 8),
            ("fiber", 2),
            ("quartz", 10),
            ("silver", 30),
            ("gold_ore", 150),
            ("gems", 100),
            ("board", 8),
            ("cloth", 11),
            ("whetstone", 26),
            ("brick", 16),
            ("stone_block", 40),
            ("iron_ingot", 24),
            ("gold_ingot", 528),
            ("backpack", 51),
            ("robe", 88),
            ("leather_gloves", 43),
            ("iron_bracers", 239),
            ("winter_gloves", 91),
            ("jacket", 187),
            ("wooden_bucket", 75),
            ("iron_bucket", 95),
            ("sickle", 161),
            ("scythe", 177),
            ("jug", 11),
            ("candle", 10),
            ("chainmail", 787),
            ("wretched_hat", 70),
            ("leather_hat", 45),
            ("war_hat", 91),
            ("iron_helmet", 315),
            ("coif", 450),
            ("wanderers_hood", 88),
            ("armet", 1581),
            ("iron_armor", 1600),
            ("archers_jacket", 126),
            ("wanderers_belt", 84),
            ("belt_with_bag", 169),
            ("wooden_shield", 112),
            ("norman_shield", 275),
            ("iron_shield", 607),
            ("beggars_cloak", 88),
            ("leather_coat", 117),
            ("researchers_cloak", 161),
            ("travelers_cloak", 989),
            ("beggars_gloves", 70),
            ("chainmail_gloves", 386),
            ("poor_boots", 100),
            ("leather_boots", 100),
            ("hiking_boots", 160),
            ("iron_boots", 730),
            ("winter_boots", 152),
            ("wooden_shovel", 56),
            ("iron_shovel", 188),
            ("hammer", 80),
            ("silver_ring", 187),
            ("trader_ring", 206),
            ("miner_amulet", 206),
            ("wanderer_pendant", 377),
            ("gold_ring", 1275),
            ("gold_pendant", 1480),
            ("silver_ingot", 96),
            ("smith_ring", 281),
            ("smith_amulet", 228),
            ("smith_belt", 98),
            ("steam_prototype", 0),
        ];
        for (id, price) in expected {
            assert_eq!(item(id).map(|item| item.base_price), Some(price), "{id}");
        }
    }

    #[test]
    fn every_recipe_price_covers_floor() {
        for recipe in recipes() {
            // Прототип паровой машины не продаётся, у него нулевая цена.
            if recipe.result_id == STEAM_PROTOTYPE_ID {
                continue;
            }
            let mut cost = 0.0;
            for ingredient in &recipe.ingredients {
                // Добываемые ресурсы считаются по себестоимости добычи,
                // остальные — по рыночной цене.
                let extraction = balance::production_cost(ingredient.item_id);
                let unit = if extraction > 0.0 {
                    extraction
                } else {
                    item(ingredient.item_id)
                        .map(|item| item.base_price as f64)
                        .unwrap_or(0.0)
                };
                cost += unit * ingredient.quantity as f64;
            }
            cost += recipe.fire_cost as f64 * balance::FIRE_PRICE;
            let floor = (cost * recipe_price_multiplier(recipe.result_id)).floor();
            let price = item(recipe.result_id)
                .map(|item| item.base_price as f64)
                .unwrap_or(0.0);
            assert!(
                price + 1e-9 >= floor,
                "{}: цена {price}, а пол себестоимости {floor}",
                recipe.result_id
            );
        }
    }

    #[test]
    fn ingots_do_not_require_coal() {
        for id in ["iron_ingot", "silver_ingot"] {
            let recipe = recipe(id).expect("рецепт есть");
            assert!(
                !recipe.ingredients.iter().any(|i| i.item_id == "coal"),
                "{id} не должен требовать уголь"
            );
        }
        // А кирпич — только из глины.
        let brick = recipe("brick").expect("рецепт есть");
        assert_eq!(brick.ingredients.len(), 1);
        assert_eq!(brick.ingredients[0].item_id, "clay");
        assert_eq!(brick.ingredients[0].quantity, 2);
    }
}
