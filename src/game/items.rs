//! Каталог предметов и рецептов.
//!
//! Каталог статический и не зависит от Dioxus. Состояние игрока хранит только
//! ID предметов и количества, поэтому новые товары добавляются здесь.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Базовое число ячеек инвентаря без экипировки.
pub const BASE_INVENTORY_SLOTS: u32 = 10;

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
            Self::Material => "материал",
            Self::Gear => "снаряжение",
            Self::Jewelry => "украшение",
            Self::Goods => "товар",
            Self::Consumable => "расходник",
        }
    }
}

/// Слот экипировки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    Hand,
    Body,
    Accessory,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 3] = [Self::Hand, Self::Body, Self::Accessory];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hand => "Рука",
            Self::Body => "Тело",
            Self::Accessory => "Аксессуар",
        }
    }
}

/// Бонусы предмета. `None` — бонуса нет.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemStats {
    pub inventory_slots: Option<u32>,
    pub trade_bonus: Option<f32>,
    pub max_energy: Option<u32>,
    pub expedition_bonus: Option<f32>,
}

impl ItemStats {
    /// Короткая строка бонусов для карточки предмета.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(slots) = self.inventory_slots {
            parts.push(format!("+{slots} ячеек"));
        }
        if let Some(trade) = self.trade_bonus {
            parts.push(format!("+{:.0}% торговли", trade * 100.0));
        }
        if let Some(energy) = self.max_energy {
            parts.push(format!("+{energy} энергии"));
        }
        if let Some(expedition) = self.expedition_bonus {
            parts.push(format!("+{:.0}% к походам", expedition * 100.0));
        }
        if parts.is_empty() {
            "без бонусов".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Предмет каталога.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub base_price: u32,
    pub stackable: bool,
    pub stats: ItemStats,
    /// Слот экипировки. `None` — предмет нельзя надеть.
    pub slot: Option<EquipSlot>,
    pub description: String,
    /// Стоимость огня кузницы, если предмет требует огонь при крафте.
    pub fire_cost: u32,
}

impl Item {
    /// Условная редкость для карточки товара.
    pub fn rarity(&self) -> &'static str {
        match self.base_price {
            0..=29 => "обычный",
            30..=99 => "необычный",
            100..=179 => "редкий",
            _ => "ценный",
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
    pub name: &'static str,
    pub ingredients: Vec<Ingredient>,
    pub result_id: &'static str,
    pub result_quantity: u32,
    /// Стоимость огня кузницы для крафта.
    pub fire_cost: u32,
}

/// Каталог всех предметов.
pub fn catalog() -> &'static [Item] {
    static CATALOG: OnceLock<Vec<Item>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Все рецепты крафта.
pub fn recipes() -> &'static [Recipe] {
    static RECIPES: OnceLock<Vec<Recipe>> = OnceLock::new();
    RECIPES.get_or_init(build_recipes)
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
        item_def(
            "wood",
            "Дерево",
            Category::Material,
            5,
            true,
            None,
            "Простая древесина. Нужна почти во всех рецептах.",
        ),
        item_def(
            "leather",
            "Кожа",
            Category::Material,
            15,
            true,
            None,
            "Выделанная кожа. Идёт на рюкзаки и одежду.",
        ),
        item_def(
            "silver",
            "Серебро",
            Category::Material,
            80,
            true,
            None,
            "Драгоценный металл. Ценится торговцами.",
        ),
        item_def(
            "quartz",
            "Кварц",
            Category::Material,
            50,
            true,
            None,
            "Прозрачный камень для украшений и зелий.",
        ),
        item_def(
            "fiber",
            "Волокно",
            Category::Material,
            8,
            true,
            None,
            "Растительное волокно. Идёт на ткань и снаряжение.",
        ),
        item_def(
            "iron_ingot",
            "Железный слиток",
            Category::Material,
            50,
            true,
            None,
            "Сплав руды и дерева. Заготовка для снаряжения.",
        )
        .with_fire_cost(5),
        item_def(
            "cloth",
            "Ткань",
            Category::Goods,
            18,
            true,
            None,
            "Обычная ткань. Ходовой товар на рынке.",
        ),
        item_def(
            "clay",
            "Глина",
            Category::Material,
            6,
            true,
            None,
            "Мягкая глина из каменоломни. Идёт на кирпич.",
        ),
        item_def(
            "backpack",
            "Рюкзак",
            Category::Gear,
            120,
            false,
            Some(EquipSlot::Body),
            "Крепкий рюкзак с дополнительными карманами.",
        )
        .with_stats(ItemStats {
            inventory_slots: Some(10),
            ..ItemStats::default()
        }),
        item_def(
            "trader_ring",
            "Кольцо торговца",
            Category::Jewelry,
            200,
            false,
            Some(EquipSlot::Accessory),
            "Серебряное кольцо. Помогает торговаться.",
        )
        .with_stats(ItemStats {
            trade_bonus: Some(0.10),
            ..ItemStats::default()
        }),
        item_def(
            "iron_sword",
            "Железный меч",
            Category::Gear,
            90,
            false,
            Some(EquipSlot::Hand),
            "Надёжный клинок для дальних походов.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(0.10),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "iron_armor",
            "Кольчуга",
            Category::Gear,
            150,
            false,
            Some(EquipSlot::Body),
            "Плотная кольчуга из слитков. Позволяет нести больше энергии.",
        )
        .with_stats(ItemStats {
            max_energy: Some(25),
            ..ItemStats::default()
        })
        .with_fire_cost(50),
        item_def(
            "coal_amulet",
            "Угольный амулет",
            Category::Jewelry,
            110,
            false,
            Some(EquipSlot::Accessory),
            "Тёмный амулет. Даёт небольшую скидку у торговцев.",
        )
        .with_stats(ItemStats {
            trade_bonus: Some(0.05),
            ..ItemStats::default()
        }),
        item_def(
            "brick",
            "Кирпич",
            Category::Goods,
            25,
            false,
            None,
            "Кирпич из глины и угля. Основной строительный материал.",
        )
        .with_fire_cost(3),
        item_def(
            "board",
            "Доска",
            Category::Goods,
            12,
            false,
            None,
            "Деревянная доска. Материал для мечей.",
        ),
    ]
}

fn item_def(
    id: &str,
    name: &str,
    category: Category,
    base_price: u32,
    stackable: bool,
    slot: Option<EquipSlot>,
    description: &str,
) -> Item {
    Item {
        id: id.to_string(),
        name: name.to_string(),
        category,
        base_price,
        stackable,
        stats: ItemStats::default(),
        slot,
        description: description.to_string(),
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

fn build_recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            id: "iron_ingot",
            name: "Железный слиток",
            ingredients: vec![
                Ingredient {
                    item_id: "iron",
                    quantity: 2,
                },
                Ingredient {
                    item_id: "coal",
                    quantity: 1,
                },
            ],
            result_id: "iron_ingot",
            result_quantity: 1,
            fire_cost: 5,
        },
        Recipe {
            id: "brick",
            name: "Кирпич",
            ingredients: vec![
                Ingredient {
                    item_id: "clay",
                    quantity: 2,
                },
                Ingredient {
                    item_id: "coal",
                    quantity: 1,
                },
            ],
            result_id: "brick",
            result_quantity: 1,
            fire_cost: 3,
        },
        Recipe {
            id: "board",
            name: "Доска",
            ingredients: vec![Ingredient {
                item_id: "wood",
                quantity: 2,
            }],
            result_id: "board",
            result_quantity: 1,
            fire_cost: 0,
        },
        Recipe {
            id: "backpack",
            name: "Рюкзак",
            ingredients: vec![
                Ingredient {
                    item_id: "leather",
                    quantity: 2,
                },
                Ingredient {
                    item_id: "iron_ingot",
                    quantity: 1,
                },
            ],
            result_id: "backpack",
            result_quantity: 1,
            fire_cost: 0,
        },
        Recipe {
            id: "trader_ring",
            name: "Кольцо торговца",
            ingredients: vec![
                Ingredient {
                    item_id: "silver",
                    quantity: 1,
                },
                Ingredient {
                    item_id: "quartz",
                    quantity: 1,
                },
            ],
            result_id: "trader_ring",
            result_quantity: 1,
            fire_cost: 0,
        },
        Recipe {
            id: "iron_sword",
            name: "Железный меч",
            ingredients: vec![
                Ingredient {
                    item_id: "iron_ingot",
                    quantity: 2,
                },
                Ingredient {
                    item_id: "board",
                    quantity: 1,
                },
                Ingredient {
                    item_id: "leather",
                    quantity: 1,
                },
            ],
            result_id: "iron_sword",
            result_quantity: 1,
            fire_cost: 15,
        },
        Recipe {
            id: "iron_armor",
            name: "Кольчуга",
            ingredients: vec![
                Ingredient {
                    item_id: "iron_ingot",
                    quantity: 25,
                },
                Ingredient {
                    item_id: "leather",
                    quantity: 2,
                },
            ],
            result_id: "iron_armor",
            result_quantity: 1,
            fire_cost: 50,
        },
        Recipe {
            id: "coal_amulet",
            name: "Угольный амулет",
            ingredients: vec![
                Ingredient {
                    item_id: "iron_ingot",
                    quantity: 1,
                },
                Ingredient {
                    item_id: "quartz",
                    quantity: 1,
                },
            ],
            result_id: "coal_amulet",
            result_quantity: 1,
            fire_cost: 0,
        },
    ]
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
                let exists = item(ingredient.item_id).is_some()
                    || matches!(ingredient.item_id, "wood" | "stone" | "iron" | "coal");
                assert!(exists, "нет ингредиента {}", ingredient.item_id);
            }
        }
    }

    #[test]
    fn gear_items_have_slots() {
        assert_eq!(
            item("backpack").and_then(|item| item.slot),
            Some(EquipSlot::Body)
        );
        assert_eq!(
            item("trader_ring").and_then(|item| item.slot),
            Some(EquipSlot::Accessory)
        );
        assert_eq!(item("iron_ore").and_then(|item| item.slot), None);
    }
}
