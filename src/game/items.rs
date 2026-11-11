//! Каталог предметов и рецептов.
//!
//! Каталог статический и не зависит от Dioxus. Состояние игрока хранит только
//! ID предметов и количества, поэтому новые товары добавляются здесь.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Базовое число ячеек инвентаря без экипировки.
pub const BASE_INVENTORY_SLOTS: u32 = 10;

/// Сколько штук одного предмета помещается в одну ячейку.
pub const ITEM_STACK_LIMIT: u32 = 9_999;

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
    Head,
    Feet,
    Accessory1,
    Accessory2,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 6] = [
        Self::Head,
        Self::Body,
        Self::Hand,
        Self::Feet,
        Self::Accessory1,
        Self::Accessory2,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hand => "Рука",
            Self::Body => "Тело",
            Self::Head => "Голова",
            Self::Feet => "Ноги",
            Self::Accessory1 => "Аксессуар I",
            Self::Accessory2 => "Аксессуар II",
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
    /// +% к добыче дерева.
    pub wood_bonus: Option<f32>,
    /// +% к добыче камня и глины.
    pub stone_bonus: Option<f32>,
    /// +% к добыче руды.
    pub ore_bonus: Option<f32>,
    /// +% к добыче волокна.
    pub fiber_bonus: Option<f32>,
    /// +% к золоту за продажу.
    pub gold_bonus: Option<f32>,
    /// +% к шансу редкой находки в походах.
    pub rare_loot_bonus: Option<f32>,
    /// +% к шансу найти драгоценные камни.
    pub gems_bonus: Option<f32>,
    /// −% к расходу огня кузницы.
    pub fire_discount: Option<f32>,
    /// −% к расходу энергии в походах.
    pub expedition_energy_discount: Option<f32>,
    /// Особый эффект текстом (например, открывает шахту).
    pub unlock: Option<String>,
}

impl ItemStats {
    /// Строка бонусов для карточки предмета.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(slots) = self.inventory_slots {
            parts.push(format!("+{slots} ячеек в инвентаре"));
        }
        if let Some(trade) = self.trade_bonus {
            parts.push(format!("+{trade:.0}% к цене продажи"));
        }
        if let Some(energy) = self.max_energy {
            parts.push(format!("+{energy} к запасу энергии"));
        }
        if let Some(expedition) = self.expedition_bonus {
            parts.push(format!("+{expedition:.0}% к скорости походов"));
        }
        if let Some(wood) = self.wood_bonus {
            parts.push(format!("+{wood:.0}% к добыче дерева"));
        }
        if let Some(stone) = self.stone_bonus {
            parts.push(format!("+{stone:.0}% к добыче камня и глины"));
        }
        if let Some(ore) = self.ore_bonus {
            parts.push(format!("+{ore:.0}% к добыче руды"));
        }
        if let Some(fiber) = self.fiber_bonus {
            parts.push(format!("+{fiber:.0}% к добыче волокна"));
        }
        if let Some(gold) = self.gold_bonus {
            parts.push(format!("+{gold:.0}% к золоту за продажу"));
        }
        if let Some(rare) = self.rare_loot_bonus {
            parts.push(format!("+{rare:.0}% к шансу редкого лута"));
        }
        if let Some(gems) = self.gems_bonus {
            parts.push(format!("+{gems:.0}% к шансу найти камни"));
        }
        if let Some(fire) = self.fire_discount {
            parts.push(format!("−{fire:.0}% расход огня в кузнице"));
        }
        if let Some(energy) = self.expedition_energy_discount {
            parts.push(format!("−{energy:.0}% расход энергии в походах"));
        }
        if let Some(unlock) = &self.unlock {
            parts.push(unlock.clone());
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
        // ---------- Складские ресурсы ----------
        item_def(
            "wood",
            "Дерево",
            Category::Material,
            2,
            None,
            "Простая древесина. Нужна почти во всех рецептах.",
        ),
        item_def(
            "stone",
            "Камень",
            Category::Material,
            2,
            None,
            "Обычный камень. Основа строительства и точил.",
        ),
        item_def(
            "iron",
            "Железная руда",
            Category::Material,
            5,
            None,
            "Кусок руды. Плавится в железные слитки.",
        ),
        item_def(
            "coal",
            "Уголь",
            Category::Material,
            3,
            None,
            "Уголь для кузницы. Горит и даёт огонь.",
        ),
        // ---------- Инвентарные ресурсы ----------
        item_def(
            "clay",
            "Глина",
            Category::Material,
            3,
            None,
            "Мягкая глина из каменоломни. Идёт на кирпич и кувшины.",
        ),
        item_def(
            "leather",
            "Кожа",
            Category::Material,
            8,
            None,
            "Выделанная кожа. Идёт на рюкзаки и одежду.",
        ),
        item_def(
            "fiber",
            "Волокно",
            Category::Material,
            2,
            None,
            "Растительное волокно. Идёт на ткань и свечи.",
        ),
        item_def(
            "quartz",
            "Кварц",
            Category::Material,
            10,
            None,
            "Прозрачный камень для точил и украшений.",
        ),
        item_def(
            "silver",
            "Серебро",
            Category::Material,
            30,
            None,
            "Драгоценный металл для украшений.",
        ),
        item_def(
            "gold_ore",
            "Золотая руда",
            Category::Material,
            150,
            None,
            "Редкая золотая руда. Плавится в золотые слитки.",
        ),
        item_def(
            "gems",
            "Драгоценные камни",
            Category::Material,
            100,
            None,
            "Огранённые камни для дорогих украшений.",
        ),
        // ---------- Карты локаций ----------
        item_def(
            "map_forest",
            "Карта Дремучего леса",
            Category::Material,
            500,
            None,
            "Открывает поход в Дремучий лес. Покупается у торговца или падает в поле и овраге.",
        ),
        item_def(
            "map_mines",
            "Карта Глубоких шахт",
            Category::Material,
            1500,
            None,
            "Открывает поход в Глубокие шахты. Нужна свеча.",
        ),
        item_def(
            "map_quarry",
            "Карта Забытой каменоломни",
            Category::Material,
            5000,
            None,
            "Открывает поход в Забытую каменоломню. Нужна свеча.",
        ),
        item_def(
            "map_ruins",
            "Карта Древних руин",
            Category::Material,
            10000,
            None,
            "Открывает поход в Древние руины. Нужен факел или свеча.",
        ),
        item_def(
            "steam_blueprint",
            "Чертёж паровой машины",
            Category::Material,
            5000,
            None,
            "Особая находка Древних руин. Пригодится в будущих эпохах.",
        ),
        // ---------- Промежуточные материалы ----------
        item_def(
            "board",
            "Доска",
            Category::Material,
            5,
            None,
            "Деревянная доска. Материал для инструментов.",
        ),
        item_def(
            "cloth",
            "Ткань",
            Category::Material,
            7,
            None,
            "Обычная ткань. Идёт на робы и куртки.",
        ),
        item_def(
            "whetstone",
            "Точильный камень",
            Category::Material,
            16,
            None,
            "Камень с кварцем. Чтобы затачивать инструменты.",
        ),
        item_def(
            "brick",
            "Кирпич",
            Category::Goods,
            10,
            None,
            "Кирпич из глины и угля. Строительный материал.",
        )
        .with_fire_cost(10),
        item_def(
            "iron_ingot",
            "Железный слиток",
            Category::Material,
            15,
            None,
            "Выплавленная руда. Заготовка для снаряжения.",
        )
        .with_fire_cost(15),
        item_def(
            "gold_ingot",
            "Золотой слиток",
            Category::Material,
            330,
            None,
            "Чистое золото для дорогих украшений.",
        )
        .with_fire_cost(30),
        // ---------- Экипировка и инструменты ----------
        item_def(
            "backpack",
            "Кожаный рюкзак",
            Category::Gear,
            32,
            Some(EquipSlot::Body),
            "Крепкий рюкзак с карманами для находок.",
        )
        .with_stats(ItemStats {
            inventory_slots: Some(10),
            ..ItemStats::default()
        }),
        item_def(
            "robe",
            "Роба",
            Category::Gear,
            39,
            Some(EquipSlot::Body),
            "Лёгкая роба странника. Не сковывает в пути.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            ..ItemStats::default()
        }),
        item_def(
            "leather_gloves",
            "Кожаные перчатки",
            Category::Gear,
            27,
            Some(EquipSlot::Hand),
            "Удобные перчатки для работы в поле и в лесу.",
        )
        .with_stats(ItemStats {
            wood_bonus: Some(10.0),
            stone_bonus: Some(10.0),
            ..ItemStats::default()
        }),
        item_def(
            "iron_bracers",
            "Железные наручи",
            Category::Gear,
            83,
            Some(EquipSlot::Hand),
            "Наручи кузнеца: часть жара уходит в металл.",
        )
        .with_stats(ItemStats {
            fire_discount: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(40),
        item_def(
            "winter_gloves",
            "Зимние перчатки",
            Category::Gear,
            50,
            Some(EquipSlot::Hand),
            "Тёплые перчатки. Меньше усталости в пути.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(10.0),
            ..ItemStats::default()
        }),
        item_def(
            "jacket",
            "Куртка",
            Category::Gear,
            98,
            Some(EquipSlot::Body),
            "Плотная куртка из кожи и ткани.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(15.0),
            ..ItemStats::default()
        }),
        // ---------- Инструменты для фермы ----------
        item_def(
            "wooden_bucket",
            "Деревянное ведро",
            Category::Goods,
            47,
            None,
            "Ведро из досок.",
        ),
        item_def(
            "iron_bucket",
            "Железное ведро",
            Category::Goods,
            42,
            None,
            "Прочное ведро, сколоченное на кузнице.",
        )
        .with_fire_cost(15),
        item_def(
            "sickle",
            "Серп",
            Category::Goods,
            60,
            None,
            "Серп для жатвы волокна.",
        )
        .with_stats(ItemStats {
            fiber_bonus: Some(15.0),
            ..ItemStats::default()
        })
        .with_fire_cost(30),
        item_def(
            "scythe",
            "Коса",
            Category::Goods,
            72,
            None,
            "Длинная коса: косит волокно рядками.",
        )
        .with_stats(ItemStats {
            fiber_bonus: Some(25.0),
            ..ItemStats::default()
        })
        .with_fire_cost(35),
        // ---------- Товары на продажу ----------
        item_def(
            "jug",
            "Глиняный кувшин",
            Category::Goods,
            7,
            None,
            "Кувшин, вылепленный из глины.",
        ),
        item_def(
            "candle",
            "Свеча",
            Category::Goods,
            6,
            None,
            "Свеча из волокна и угля. Нужна для походов в тёмные локации.",
        ),
        // ---------- Головные уборы и шлемы ----------
        item_def(
            "wretched_hat",
            "Убогая шапка",
            Category::Gear,
            26,
            Some(EquipSlot::Head),
            "Латаная шапка из обрезков ткани. Греет хоть как-то.",
        )
        .with_stats(ItemStats {
            max_energy: Some(5),
            ..ItemStats::default()
        }),
        item_def(
            "leather_hat",
            "Кожаная шапка",
            Category::Gear,
            28,
            Some(EquipSlot::Head),
            "Простая шапка из выделанной кожи.",
        )
        .with_stats(ItemStats {
            wood_bonus: Some(5.0),
            fiber_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "war_hat",
            "Шапка воина",
            Category::Gear,
            48,
            Some(EquipSlot::Head),
            "Кожаная шапка с железными пластинами.",
        )
        .with_stats(ItemStats {
            stone_bonus: Some(5.0),
            ore_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "iron_helmet",
            "Железный шлем",
            Category::Gear,
            95,
            Some(EquipSlot::Head),
            "Клёпаный шлем с кожаным подшлемником.",
        )
        .with_stats(ItemStats {
            max_energy: Some(15),
            fire_discount: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "coif",
            "Койф",
            Category::Gear,
            120,
            Some(EquipSlot::Head),
            "Кольчужный капюшон, прикрывающий шею и плечи.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(50),
        item_def(
            "wanderers_hood",
            "Капюшон странника",
            Category::Gear,
            40,
            Some(EquipSlot::Head),
            "Мягкий капюшон, не боится ни дождя, ни ветра.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(8.0),
            ..ItemStats::default()
        }),
        item_def(
            "armet",
            "Армет",
            Category::Gear,
            500,
            Some(EquipSlot::Head),
            "Закрытый рыцарский шлем с золотой насечкой.",
        )
        .with_stats(ItemStats {
            max_energy: Some(20),
            trade_bonus: Some(8.0),
            gold_bonus: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(150),
        // ---------- Броня ----------
        item_def(
            "iron_armor",
            "Железная броня",
            Category::Gear,
            270,
            Some(EquipSlot::Body),
            "Тяжёлая броня из железных пластин на кожаной основе.",
        )
        .with_stats(ItemStats {
            max_energy: Some(30),
            fire_discount: Some(10.0),
            ..ItemStats::default()
        })
        .with_fire_cost(200),
        item_def(
            "archers_jacket",
            "Куртка лучника",
            Category::Gear,
            65,
            Some(EquipSlot::Body),
            "Стёганая куртка, не мешает натягивать лук.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            fiber_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        // ---------- Пояса ----------
        item_def(
            "wanderers_belt",
            "Пояс странника",
            Category::Gear,
            48,
            Some(EquipSlot::Accessory1),
            "Ремень с кварцевыми бляхами. Дорога любит таких.",
        )
        .with_stats(ItemStats {
            inventory_slots: Some(3),
            trade_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        item_def(
            "belt_with_bag",
            "Пояс с сумкой",
            Category::Gear,
            85,
            Some(EquipSlot::Accessory1),
            "Широкий ремень с кожаной сумкой на боку.",
        )
        .with_stats(ItemStats {
            inventory_slots: Some(5),
            expedition_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        // ---------- Щиты ----------
        item_def(
            "wooden_shield",
            "Деревянный щит",
            Category::Gear,
            55,
            None,
            "Щит из досок, обитых по краю.",
        ),
        item_def(
            "norman_shield",
            "Норманский щит",
            Category::Gear,
            80,
            None,
            "Длинный щит с железным умбоном.",
        )
        .with_fire_cost(50),
        item_def(
            "iron_shield",
            "Железный щит",
            Category::Gear,
            185,
            None,
            "Цельнометаллический щит. Держит удар копья.",
        ),
        // ---------- Плащи ----------
        item_def(
            "beggars_cloak",
            "Плащ нищего",
            Category::Gear,
            38,
            Some(EquipSlot::Body),
            "Выцветший плащ с чужого плеча.",
        )
        .with_stats(ItemStats {
            inventory_slots: Some(2),
            trade_bonus: Some(2.0),
            ..ItemStats::default()
        }),
        item_def(
            "leather_coat",
            "Кожаный плащ",
            Category::Gear,
            68,
            Some(EquipSlot::Body),
            "Плотный плащ из сшитых кож.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(5.0),
            wood_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "researchers_cloak",
            "Плащ исследователя",
            Category::Gear,
            78,
            Some(EquipSlot::Body),
            "Плащ с кварцевыми застёжками и глубокими карманами.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            inventory_slots: Some(4),
            ..ItemStats::default()
        }),
        item_def(
            "travelers_cloak",
            "Плащ путешественника",
            Category::Gear,
            190,
            Some(EquipSlot::Body),
            "Дорожный плащ с каменной брошью. Видел дальние земли.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(15.0),
            expedition_energy_discount: Some(8.0),
            trade_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        // ---------- Перчатки ----------
        item_def(
            "beggars_gloves",
            "Перчатки нищего",
            Category::Gear,
            30,
            Some(EquipSlot::Hand),
            "Обрезанные перчатки без пальцев.",
        )
        .with_stats(ItemStats {
            fiber_bonus: Some(8.0),
            wood_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "chainmail_gloves",
            "Кольчужные перчатки",
            Category::Gear,
            105,
            Some(EquipSlot::Hand),
            "Кольчужные рукавицы кузнечной работы.",
        )
        .with_stats(ItemStats {
            fire_discount: Some(10.0),
            ore_bonus: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(100),
        // ---------- Обувь ----------
        item_def(
            "poor_boots",
            "Нищие ботинки",
            Category::Gear,
            46,
            Some(EquipSlot::Feet),
            "Стоптанные ботинки, перевязанные верёвкой.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(3.0),
            ..ItemStats::default()
        }),
        item_def(
            "leather_boots",
            "Кожаные ботинки",
            Category::Gear,
            58,
            Some(EquipSlot::Feet),
            "Крепкие кожаные ботинки на двойной подошве.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(5.0),
            expedition_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        item_def(
            "hiking_boots",
            "Походные ботинки",
            Category::Gear,
            90,
            Some(EquipSlot::Feet),
            "Обувь для долгих дорог: кожа, войлок и смола.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(8.0),
            expedition_bonus: Some(5.0),
            ..ItemStats::default()
        }),
        item_def(
            "iron_boots",
            "Железные ботинки",
            Category::Gear,
            195,
            Some(EquipSlot::Feet),
            "Сабатоны из железа. Тяжело, зато не пробить.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(10.0),
            max_energy: Some(10),
            ..ItemStats::default()
        })
        .with_fire_cost(150),
        item_def(
            "winter_boots",
            "Зимние ботинки",
            Category::Gear,
            80,
            Some(EquipSlot::Feet),
            "Тёплые ботинки на меху.",
        )
        .with_stats(ItemStats {
            expedition_energy_discount: Some(10.0),
            wood_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        // ---------- Инструменты ----------
        item_def(
            "wooden_shovel",
            "Деревянная лопата",
            Category::Gear,
            30,
            Some(EquipSlot::Hand),
            "Лопата из досок. Для мягкой земли.",
        )
        .with_stats(ItemStats {
            stone_bonus: Some(8.0),
            wood_bonus: Some(3.0),
            ..ItemStats::default()
        }),
        item_def(
            "iron_shovel",
            "Железная лопата",
            Category::Gear,
            65,
            Some(EquipSlot::Hand),
            "Лопата с железным лотком.",
        )
        .with_stats(ItemStats {
            stone_bonus: Some(12.0),
            ore_bonus: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(20),
        item_def(
            "hammer",
            "Молоток",
            Category::Gear,
            30,
            None,
            "Молоток кузнеца: железная голова на дубовой ручке.",
        )
        .with_fire_cost(15),
        item_def(
            "chainmail",
            "Кольчуга",
            Category::Goods,
            431,
            Some(EquipSlot::Body),
            "Долгая работа кузнеца: сотни колец.",
        )
        .with_stats(ItemStats {
            fire_discount: Some(10.0),
            expedition_energy_discount: Some(8.0),
            ..ItemStats::default()
        })
        .with_fire_cost(200),
        // ---------- Бижутерия ----------
        item_def(
            "silver_ring",
            "Серебряное кольцо",
            Category::Jewelry,
            33,
            Some(EquipSlot::Accessory1),
            "Простое кольцо из серебра.",
        )
        .with_stats(ItemStats {
            trade_bonus: Some(3.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "trader_ring",
            "Кольцо торговца",
            Category::Jewelry,
            44,
            Some(EquipSlot::Accessory1),
            "Серебряное кольцо с кварцем. Помогает торговаться.",
        )
        .with_stats(ItemStats {
            trade_bonus: Some(10.0),
            gold_bonus: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "miner_amulet",
            "Амулет шахтёра",
            Category::Jewelry,
            44,
            Some(EquipSlot::Accessory1),
            "Кварц чутко отзывается на руду в породе.",
        )
        .with_stats(ItemStats {
            ore_bonus: Some(15.0),
            stone_bonus: Some(15.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "wanderer_pendant",
            "Кулон странника",
            Category::Jewelry,
            42,
            Some(EquipSlot::Accessory1),
            "Серебряный кулон с драгоценным камнем. Приносит удачу в странствиях.",
        )
        .with_stats(ItemStats {
            expedition_bonus: Some(10.0),
            expedition_energy_discount: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(15),
        item_def(
            "gold_ring",
            "Золотое кольцо с камнем",
            Category::Jewelry,
            473,
            Some(EquipSlot::Accessory1),
            "Тяжёлое золото и яркий камень.",
        )
        .with_stats(ItemStats {
            gold_bonus: Some(20.0),
            trade_bonus: Some(5.0),
            ..ItemStats::default()
        })
        .with_fire_cost(60),
        item_def(
            "gold_pendant",
            "Золотая подвеска",
            Category::Jewelry,
            583,
            Some(EquipSlot::Accessory1),
            "Золотая подвеска с двумя камнями.",
        )
        .with_stats(ItemStats {
            trade_bonus: Some(10.0),
            gold_bonus: Some(10.0),
            ..ItemStats::default()
        })
        .with_fire_cost(80),
    ]
}

fn item_def(
    id: &str,
    name: &str,
    category: Category,
    base_price: u32,
    slot: Option<EquipSlot>,
    description: &str,
) -> Item {
    Item {
        id: id.to_string(),
        name: name.to_string(),
        category,
        base_price,
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

/// Ингредиент одной позиции.
fn need(item_id: &'static str, quantity: u32) -> Ingredient {
    Ingredient { item_id, quantity }
}

/// Рецепт крафта в одну строку.
fn make_recipe(
    id: &'static str,
    name: &'static str,
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
        name,
        ingredients,
        result_id,
        result_quantity,
        fire_cost,
    }
}

fn build_recipes() -> Vec<Recipe> {
    vec![
        // Промежуточные материалы.
        make_recipe("board", "Доска", "board", 1, 0, vec![need("wood", 2)]),
        make_recipe("cloth", "Ткань", "cloth", 1, 0, vec![need("fiber", 3)]),
        make_recipe(
            "whetstone",
            "Точильный камень",
            "whetstone",
            1,
            0,
            vec![need("stone", 2), need("quartz", 1)],
        ),
        make_recipe(
            "brick",
            "Кирпич",
            "brick",
            1,
            10,
            vec![need("clay", 2), need("coal", 1)],
        ),
        make_recipe(
            "iron_ingot",
            "Железный слиток",
            "iron_ingot",
            1,
            15,
            vec![need("iron", 2), need("coal", 1)],
        ),
        make_recipe(
            "gold_ingot",
            "Золотой слиток",
            "gold_ingot",
            1,
            30,
            vec![need("gold_ore", 2)],
        ),
        // Экипировка и инструменты.
        make_recipe(
            "backpack",
            "Кожаный рюкзак",
            "backpack",
            1,
            0,
            vec![need("leather", 3), need("board", 1)],
        ),
        make_recipe("robe", "Роба", "robe", 1, 0, vec![need("cloth", 5)]),
        make_recipe(
            "leather_gloves",
            "Кожаные перчатки",
            "leather_gloves",
            1,
            0,
            vec![need("leather", 3)],
        ),
        make_recipe(
            "iron_bracers",
            "Железные наручи",
            "iron_bracers",
            1,
            40,
            vec![need("iron_ingot", 5)],
        ),
        make_recipe(
            "winter_gloves",
            "Зимние перчатки",
            "winter_gloves",
            1,
            0,
            vec![need("leather", 3), need("cloth", 3)],
        ),
        make_recipe(
            "jacket",
            "Куртка",
            "jacket",
            1,
            0,
            vec![need("leather", 5), need("cloth", 7)],
        ),
        // Инструменты для фермы.
        make_recipe(
            "wooden_bucket",
            "Деревянное ведро",
            "wooden_bucket",
            1,
            0,
            vec![need("board", 5)],
        ),
        make_recipe(
            "iron_bucket",
            "Железное ведро",
            "iron_bucket",
            1,
            15,
            vec![need("iron_ingot", 2)],
        ),
        make_recipe(
            "sickle",
            "Серп",
            "sickle",
            1,
            30,
            vec![need("iron_ingot", 3), need("board", 1)],
        ),
        make_recipe(
            "scythe",
            "Коса",
            "scythe",
            1,
            35,
            vec![need("iron_ingot", 3), need("board", 2)],
        ),
        // Товары на продажу.
        make_recipe("jug", "Глиняный кувшин", "jug", 1, 0, vec![need("clay", 2)]),
        make_recipe(
            "candle",
            "Свеча",
            "candle",
            1,
            0,
            vec![need("fiber", 1), need("coal", 1)],
        ),
        // Головные уборы и шлемы.
        make_recipe(
            "wretched_hat",
            "Убогая шапка",
            "wretched_hat",
            1,
            0,
            vec![need("cloth", 4)],
        ),
        make_recipe(
            "leather_hat",
            "Кожаная шапка",
            "leather_hat",
            1,
            0,
            vec![need("leather", 3)],
        ),
        make_recipe(
            "war_hat",
            "Шапка воина",
            "war_hat",
            1,
            0,
            vec![need("leather", 3), need("iron_ingot", 1)],
        ),
        make_recipe(
            "iron_helmet",
            "Железный шлем",
            "iron_helmet",
            1,
            0,
            vec![need("leather", 2), need("cloth", 2), need("iron_ingot", 4)],
        ),
        make_recipe(
            "coif",
            "Койф",
            "coif",
            1,
            50,
            vec![need("cloth", 4), need("iron_ingot", 6)],
        ),
        make_recipe(
            "wanderers_hood",
            "Капюшон странника",
            "wanderers_hood",
            1,
            0,
            vec![need("cloth", 5)],
        ),
        make_recipe(
            "armet",
            "Армет",
            "armet",
            1,
            150,
            vec![need("iron_ingot", 10), need("gold_ingot", 1)],
        ),
        // Броня.
        make_recipe(
            "iron_armor",
            "Железная броня",
            "iron_armor",
            1,
            200,
            vec![need("iron_ingot", 15), need("leather", 4)],
        ),
        make_recipe(
            "archers_jacket",
            "Куртка лучника",
            "archers_jacket",
            1,
            0,
            vec![need("cloth", 5), need("leather", 3)],
        ),
        // Пояса.
        make_recipe(
            "wanderers_belt",
            "Пояс странника",
            "wanderers_belt",
            1,
            0,
            vec![need("leather", 3), need("quartz", 2)],
        ),
        make_recipe(
            "belt_with_bag",
            "Пояс с сумкой",
            "belt_with_bag",
            1,
            0,
            vec![need("leather", 7), need("cloth", 3)],
        ),
        // Щиты.
        make_recipe(
            "wooden_shield",
            "Деревянный щит",
            "wooden_shield",
            1,
            0,
            vec![need("board", 10)],
        ),
        make_recipe(
            "norman_shield",
            "Норманский щит",
            "norman_shield",
            1,
            50,
            vec![need("board", 8), need("iron_ingot", 2)],
        ),
        make_recipe(
            "iron_shield",
            "Железный щит",
            "iron_shield",
            1,
            0,
            vec![need("iron_ingot", 10), need("leather", 3)],
        ),
        // Плащи.
        make_recipe(
            "beggars_cloak",
            "Плащ нищего",
            "beggars_cloak",
            1,
            0,
            vec![need("cloth", 5)],
        ),
        make_recipe(
            "leather_coat",
            "Кожаный плащ",
            "leather_coat",
            1,
            0,
            vec![need("cloth", 3), need("leather", 5)],
        ),
        make_recipe(
            "researchers_cloak",
            "Плащ исследователя",
            "researchers_cloak",
            1,
            0,
            vec![need("leather", 5), need("quartz", 3)],
        ),
        make_recipe(
            "travelers_cloak",
            "Плащ путешественника",
            "travelers_cloak",
            1,
            0,
            vec![need("leather", 6), need("cloth", 4), need("gems", 1)],
        ),
        // Перчатки.
        make_recipe(
            "beggars_gloves",
            "Перчатки нищего",
            "beggars_gloves",
            1,
            0,
            vec![need("cloth", 4)],
        ),
        make_recipe(
            "chainmail_gloves",
            "Кольчужные перчатки",
            "chainmail_gloves",
            1,
            100,
            vec![need("iron_ingot", 5), need("cloth", 3)],
        ),
        // Обувь.
        make_recipe(
            "poor_boots",
            "Нищие ботинки",
            "poor_boots",
            1,
            0,
            vec![need("cloth", 5), need("leather", 1)],
        ),
        make_recipe(
            "leather_boots",
            "Кожаные ботинки",
            "leather_boots",
            1,
            0,
            vec![need("leather", 5), need("cloth", 2)],
        ),
        make_recipe(
            "hiking_boots",
            "Походные ботинки",
            "hiking_boots",
            1,
            0,
            vec![need("leather", 7), need("cloth", 4)],
        ),
        make_recipe(
            "iron_boots",
            "Железные ботинки",
            "iron_boots",
            1,
            150,
            vec![need("iron_ingot", 10), need("cloth", 5)],
        ),
        make_recipe(
            "winter_boots",
            "Зимние ботинки",
            "winter_boots",
            1,
            0,
            vec![need("leather", 5), need("cloth", 5)],
        ),
        // Инструменты.
        make_recipe(
            "wooden_shovel",
            "Деревянная лопата",
            "wooden_shovel",
            1,
            0,
            vec![need("board", 5)],
        ),
        make_recipe(
            "iron_shovel",
            "Железная лопата",
            "iron_shovel",
            1,
            20,
            vec![need("board", 3), need("iron_ingot", 3)],
        ),
        make_recipe(
            "hammer",
            "Молоток",
            "hammer",
            1,
            15,
            vec![need("board", 2), need("iron_ingot", 1)],
        ),
        make_recipe(
            "chainmail",
            "Кольчуга",
            "chainmail",
            1,
            200,
            vec![need("iron_ingot", 12), need("leather", 3)],
        ),
        // Бижутерия.
        make_recipe(
            "silver_ring",
            "Серебряное кольцо",
            "silver_ring",
            1,
            15,
            vec![need("silver", 1)],
        ),
        make_recipe(
            "trader_ring",
            "Кольцо торговца",
            "trader_ring",
            1,
            15,
            vec![need("silver", 1), need("quartz", 1)],
        ),
        make_recipe(
            "miner_amulet",
            "Амулет шахтёра",
            "miner_amulet",
            1,
            15,
            vec![need("silver", 1), need("quartz", 1)],
        ),
        make_recipe(
            "wanderer_pendant",
            "Кулон странника",
            "wanderer_pendant",
            1,
            15,
            vec![need("silver", 1), need("gems", 1)],
        ),
        make_recipe(
            "gold_ring",
            "Золотое кольцо с камнем",
            "gold_ring",
            1,
            60,
            vec![need("gold_ingot", 1), need("gems", 1)],
        ),
        make_recipe(
            "gold_pendant",
            "Золотая подвеска",
            "gold_pendant",
            1,
            80,
            vec![need("gold_ingot", 1), need("gems", 2)],
        ),
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
            Some(EquipSlot::Body)
        );
        assert_eq!(
            item("trader_ring").and_then(|item| item.slot),
            Some(EquipSlot::Accessory1)
        );
        assert_eq!(item("wood").and_then(|item| item.slot), None);
    }

    #[test]
    fn new_gear_uses_all_six_slots() {
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
            ("beggars_cloak", EquipSlot::Body),
            ("leather_coat", EquipSlot::Body),
            ("researchers_cloak", EquipSlot::Body),
            ("travelers_cloak", EquipSlot::Body),
            ("chainmail", EquipSlot::Body),
            ("wooden_shovel", EquipSlot::Hand),
            ("iron_shovel", EquipSlot::Hand),
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
            ("iron_ingot", 15),
            ("gold_ingot", 30),
            ("robe", 0),
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
            ("board", 5),
            ("cloth", 7),
            ("whetstone", 16),
            ("brick", 10),
            ("iron_ingot", 15),
            ("gold_ingot", 330),
            ("backpack", 32),
            ("robe", 39),
            ("leather_gloves", 27),
            ("iron_bracers", 83),
            ("winter_gloves", 50),
            ("jacket", 98),
            ("wooden_bucket", 47),
            ("iron_bucket", 42),
            ("sickle", 60),
            ("scythe", 72),
            ("jug", 7),
            ("candle", 6),
            ("chainmail", 431),
            ("wretched_hat", 26),
            ("leather_hat", 28),
            ("war_hat", 48),
            ("iron_helmet", 95),
            ("coif", 120),
            ("wanderers_hood", 40),
            ("armet", 500),
            ("iron_armor", 270),
            ("archers_jacket", 65),
            ("wanderers_belt", 48),
            ("belt_with_bag", 85),
            ("wooden_shield", 55),
            ("norman_shield", 80),
            ("iron_shield", 185),
            ("beggars_cloak", 38),
            ("leather_coat", 68),
            ("researchers_cloak", 78),
            ("travelers_cloak", 190),
            ("beggars_gloves", 30),
            ("chainmail_gloves", 105),
            ("poor_boots", 46),
            ("leather_boots", 58),
            ("hiking_boots", 90),
            ("iron_boots", 195),
            ("winter_boots", 80),
            ("wooden_shovel", 30),
            ("iron_shovel", 65),
            ("hammer", 30),
            ("silver_ring", 33),
            ("trader_ring", 44),
            ("miner_amulet", 44),
            ("wanderer_pendant", 42),
            ("gold_ring", 473),
            ("gold_pendant", 583),
        ];
        for (id, price) in expected {
            assert_eq!(item(id).map(|item| item.base_price), Some(price), "{id}");
        }
    }
}
