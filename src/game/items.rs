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
            "iron_ore",
            "Железная руда",
            Category::Material,
            10,
            true,
            None,
            "Кусок руды. Основа для слитков и снаряжения.",
        ),
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
            "iron_ingot",
            "Железный слиток",
            Category::Material,
            50,
            true,
            None,
            "Сплав руды и дерева. Заготовка для снаряжения.",
        ),
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
            "strength_potion",
            "Зелье силы",
            Category::Consumable,
            45,
            true,
            None,
            "Даёт силы в походе. Расходник.",
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
        }),
        item_def(
            "iron_armor",
            "Железная броня",
            Category::Gear,
            150,
            false,
            Some(EquipSlot::Body),
            "Тяжёлая броня. Позволяет нести больше энергии.",
        )
        .with_stats(ItemStats {
            max_energy: Some(25),
            ..ItemStats::default()
        }),
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

fn build_recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            id: "iron_ingot",
            name: "Железный слиток",
            ingredients: vec![
                Ingredient {
                    item_id: "iron_ore",
                    quantity: 2,
                },
                Ingredient {
                    item_id: "wood",
                    quantity: 1,
                },
            ],
            result_id: "iron_ingot",
            result_quantity: 1,
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
                    item_id: "wood",
                    quantity: 1,
                },
            ],
            result_id: "iron_sword",
            result_quantity: 1,
        },
        Recipe {
            id: "iron_armor",
            name: "Железная броня",
            ingredients: vec![
                Ingredient {
                    item_id: "iron_ingot",
                    quantity: 3,
                },
                Ingredient {
                    item_id: "leather",
                    quantity: 1,
                },
            ],
            result_id: "iron_armor",
            result_quantity: 1,
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
        },
        Recipe {
            id: "strength_potion",
            name: "Зелье силы",
            ingredients: vec![
                Ingredient {
                    item_id: "quartz",
                    quantity: 1,
                },
                Ingredient {
                    item_id: "leather",
                    quantity: 1,
                },
            ],
            result_id: "strength_potion",
            result_quantity: 1,
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
                assert!(
                    item(ingredient.item_id).is_some(),
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
            Some(EquipSlot::Body)
        );
        assert_eq!(
            item("trader_ring").and_then(|item| item.slot),
            Some(EquipSlot::Accessory)
        );
        assert_eq!(item("iron_ore").and_then(|item| item.slot), None);
    }
}
