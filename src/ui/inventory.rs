//! Инвентарь, экипировка и крафт.

use dioxus::prelude::*;

use crate::game::actions::{self, CraftError};
use crate::game::expedition;
use crate::game::i18n;
use crate::game::items::{self, Category, EquipSlot, Item};
use crate::game::state::{EquipError, GameState};

use super::app::refresh_view;

/// Смайлик предмета по смыслу. PNG-спрайтов предметов пока нет.
pub(crate) fn item_icon(item: &Item) -> &'static str {
    match item.id.as_str() {
        "wood" => "🪵",
        "stone" => "🥌",
        "iron" => "⛏",
        "coal" => "⚫",
        "clay" => "🧱",
        "leather" => "🥾",
        "fiber" => "🌾",
        "quartz" => "🔷",
        "silver" => "⚪",
        "gold_ore" => "🪙",
        "gems" => "💎",
        "board" => "🪵",
        "cloth" => "🧵",
        "whetstone" => "🔪",
        "brick" => "🧱",
        "stone_block" => "🪨",
        "iron_ingot" => "🔩",
        "silver_ingot" => "🥈",
        "gold_ingot" => "🟡",
        "backpack" => "🎒",
        "robe" => "👘",
        "leather_gloves" => "🧤",
        "iron_bracers" => "🦾",
        "winter_gloves" => "🧤",
        "jacket" => "🧥",
        "wooden_bucket" => "🪣",
        "iron_bucket" => "🪣",
        "sickle" => "🌙",
        "scythe" => "🌾",
        "jug" => "🏺",
        "candle" => "🕯",
        "chainmail" => "🛡",
        "silver_ring" => "💍",
        "trader_ring" => "💍",
        "miner_amulet" => "📿",
        "wanderer_pendant" => "📿",
        "gold_ring" => "💍",
        "gold_pendant" => "📿",
        "map_forest" => "🗺",
        "map_mines" => "🗺",
        "map_quarry" => "🗺",
        "map_ruins" => "🗺",
        "steam_blueprint" => "📜",
        "steam_prototype" => "⚙️",
        _ => match item.category {
            Category::Gear => "🎽",
            Category::Jewelry => "💍",
            Category::Goods => "📦",
            Category::Consumable => "🧪",
            Category::Material => "📦",
        },
    }
}

/// PNG-иконка предмета, если для него есть спрайт.
pub(crate) fn item_icon_asset(item: &Item) -> Option<Asset> {
    Some(match item.id.as_str() {
        "wood" => asset!("/assets/icons/wood.png"),
        "stone" => asset!("/assets/icons/stone.png"),
        "iron" => asset!("/assets/icons/item_iron.png"),
        "coal" => asset!("/assets/icons/coal.png"),
        "clay" => asset!("/assets/icons/item_clay.png"),
        "leather" => asset!("/assets/icons/item_leather.png"),
        "fiber" => asset!("/assets/icons/item_fiber.png"),
        "quartz" => asset!("/assets/icons/item_quartz.png"),
        "silver" => asset!("/assets/icons/item_silver.png"),
        "gold_ore" => asset!("/assets/icons/item_gold_ore.png"),
        "gems" => asset!("/assets/icons/item_gems.png"),
        "board" => asset!("/assets/icons/item_board.png"),
        "cloth" => asset!("/assets/icons/item_cloth.png"),
        "whetstone" => asset!("/assets/icons/item_whetstone.png"),
        "iron_ingot" => asset!("/assets/icons/item_iron_ingot.png"),
        "brick" => asset!("/assets/icons/item_brick.png"),
        "steam_prototype" => asset!("/assets/icons/item_steam_prototype.png"),
        "silver_ingot" => asset!("/assets/icons/item_silver_ingot.png"),
        "gold_ingot" => asset!("/assets/icons/item_gold_ingot.png"),
        "stone_block" => asset!("/assets/icons/item_stone_block.png"),
        "backpack" => asset!("/assets/icons/item_backpack.png"),
        "robe" => asset!("/assets/icons/item_robe.png"),
        "leather_gloves" => asset!("/assets/icons/item_leather_gloves.png"),
        "iron_bracers" => asset!("/assets/icons/item_iron_bracers.png"),
        "winter_gloves" => asset!("/assets/icons/item_winter_gloves.png"),
        "jacket" => asset!("/assets/icons/item_jacket.png"),
        "wooden_bucket" => asset!("/assets/icons/item_wooden_bucket.png"),
        "iron_bucket" => asset!("/assets/icons/item_iron_bucket.png"),
        "jug" => asset!("/assets/icons/item_jug.png"),
        "candle" => asset!("/assets/icons/item_candle.png"),
        "chainmail" => asset!("/assets/icons/item_chainmail.png"),
        "silver_ring" => asset!("/assets/icons/item_silver_ring.png"),
        "trader_ring" => asset!("/assets/icons/item_trader_ring.png"),
        "miner_amulet" => asset!("/assets/icons/item_miner_amulet.png"),
        "wanderer_pendant" => asset!("/assets/icons/item_wanderer_pendant.png"),
        "gold_ring" => asset!("/assets/icons/gold_ring.png"),
        "gold_pendant" => asset!("/assets/icons/item_gold_pendant.png"),
        "sickle" => asset!("/assets/icons/item_sickle.png"),
        "scythe" => asset!("/assets/icons/item_scythe.png"),
        "wretched_hat" => asset!("/assets/icons/item_wretched_hat.png"),
        "leather_hat" => asset!("/assets/icons/item_leather_hat.png"),
        "war_hat" => asset!("/assets/icons/item_war_hat.png"),
        "iron_helmet" => asset!("/assets/icons/item_iron_helmet.png"),
        "coif" => asset!("/assets/icons/item_coif.png"),
        "wanderers_hood" => asset!("/assets/icons/item_wanderers_hood.png"),
        "armet" => asset!("/assets/icons/item_armet.png"),
        "iron_armor" => asset!("/assets/icons/item_iron_armor.png"),
        "archers_jacket" => asset!("/assets/icons/item_archers_jacket.png"),
        "wanderers_belt" => asset!("/assets/icons/item_wanderers_belt.png"),
        "belt_with_bag" => asset!("/assets/icons/item_belt_with_bag.png"),
        "wooden_shield" => asset!("/assets/icons/item_wooden_shield.png"),
        "norman_shield" => asset!("/assets/icons/item_norman_shield.png"),
        "iron_shield" => asset!("/assets/icons/item_iron_shield.png"),
        "beggars_cloak" => asset!("/assets/icons/item_beggars_cloak.png"),
        "leather_coat" => asset!("/assets/icons/item_leather_coat.png"),
        "researchers_cloak" => asset!("/assets/icons/item_researchers_cloak.png"),
        "travelers_cloak" => asset!("/assets/icons/item_travelers_cloak.png"),
        "beggars_gloves" => asset!("/assets/icons/item_beggars_gloves.png"),
        "chainmail_gloves" => asset!("/assets/icons/item_chainmail_gloves.png"),
        "poor_boots" => asset!("/assets/icons/item_poor_boots.png"),
        "leather_boots" => asset!("/assets/icons/item_leather_boots.png"),
        "hiking_boots" => asset!("/assets/icons/item_hiking_boots.png"),
        "iron_boots" => asset!("/assets/icons/item_iron_boots.png"),
        "winter_boots" => asset!("/assets/icons/item_winter_boots.png"),
        "wooden_shovel" => asset!("/assets/icons/item_wooden_shovel.png"),
        "iron_shovel" => asset!("/assets/icons/item_iron_shovel.png"),
        "hammer" => asset!("/assets/icons/item_hammer.png"),
        "map_forest" | "map_mines" | "map_quarry" | "map_ruins" => {
            asset!("/assets/icons/item_map.png")
        }
        "steam_blueprint" => asset!("/assets/icons/scroll.png"),
        _ => return None,
    })
}

/// Иконка предмета: PNG, если есть, иначе эмодзи.
#[component]
pub(crate) fn ItemIcon(item: Item, class: String) -> Element {
    let emoji = item_icon(&item);
    if let Some(asset) = item_icon_asset(&item) {
        rsx! {
            img { class: "{class} icon", src: "{asset}", alt: "" }
        }
    } else {
        rsx! {
            span { class: "{class}", "{emoji}" }
        }
    }
}

/// Полный экран инвентаря.
#[component]
pub fn InventoryScreen(
    state: Signal<GameState>,
    view: GameState,
    back: Signal<u32>,
    on_back: EventHandler<()>,
) -> Element {
    let mut selected = use_signal(|| None::<String>);
    let mut craft_open = use_signal(|| false);
    let mut sort_alpha = use_signal(|| false);
    let mut message = use_signal(String::new);
    let mut handled_back = use_signal(|| *back.peek());
    let mut selected_equip = use_signal(|| None::<EquipSlot>);

    use_effect(move || {
        let count = back();
        if count <= *handled_back.peek() {
            return;
        }
        handled_back.set(count);
        if selected.peek().is_some() {
            selected.set(None);
        } else if *craft_open.peek() {
            craft_open.set(false);
        } else if selected_equip.peek().is_some() {
            selected_equip.set(None);
        } else {
            on_back.call(());
        }
    });

    let selected_now = selected();
    let craft_now = craft_open();
    let message_text = message();
    // В сетке — только то, что реально лежит в сумке.
    let mut displayed: Vec<Item> = view
        .inventory
        .iter()
        .filter(|(_, quantity)| **quantity > 0)
        .filter_map(|(item_id, _)| items::item(item_id).cloned())
        .collect();
    if sort_alpha() {
        displayed.sort_by(|left, right| left.name().cmp(right.name()));
    }
    // Пустые ячейки до размера сумки: без рюкзака стандарт, с рюкзаком — больше.
    let total_slots = view.inventory_slots() as usize;
    let item_cells = displayed.len();
    let empty_cells = total_slots.saturating_sub(item_cells);

    rsx! {
        section { class: "inv-head",
            div { class: "inv-title-row",
                h2 { class: "inv-title", "{i18n::t(\"bag.title\")}" }
                div { class: "inv-head-actions",
                    button {
                        class: if sort_alpha() { "inv-tool active" } else { "inv-tool" },
                        onclick: move |_| {
                            let next = !sort_alpha();
                            sort_alpha.set(next);
                            let key = if next {
                                "bag.sort_name"
                            } else {
                                "bag.sort_catalog"
                            };
                            message.set(i18n::t(key).to_string());
                        },
                        "{i18n::t(\"bag.sort\")}"
                    }
                    button {
                        class: if craft_now { "inv-tool active" } else { "inv-tool" },
                        onclick: move |_| {
                            craft_open.set(!craft_now);
                            selected.set(None);
                        },
                        "{i18n::t(\"bag.craft_open\")}"
                    }
                }
            }
        }

        // Во время крафта экипировка скрыта — только сетка предметов.
        if !craft_now {
            section { class: "inv-equipment",
                div { class: "inv-subtitle", "{i18n::t(\"bag.equipment\")}" }
                div { class: "equip-grid",
                    EquipmentSlot {
                        slot: EquipSlot::Head,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Head),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Head) {
                                None
                            } else {
                                Some(EquipSlot::Head)
                            };
                            selected_equip.set(next);
                        },
                    }
                    EquipmentSlot {
                        slot: EquipSlot::Accessory1,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Accessory1),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Accessory1) {
                                None
                            } else {
                                Some(EquipSlot::Accessory1)
                            };
                            selected_equip.set(next);
                        },
                    }
                    EquipmentSlot {
                        slot: EquipSlot::Body,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Body),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Body) {
                                None
                            } else {
                                Some(EquipSlot::Body)
                            };
                            selected_equip.set(next);
                        },
                    }
                    EquipmentSlot {
                        slot: EquipSlot::Hand,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Hand),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Hand) {
                                None
                            } else {
                                Some(EquipSlot::Hand)
                            };
                            selected_equip.set(next);
                        },
                    }
                    EquipmentSlot {
                        slot: EquipSlot::Feet,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Feet),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Feet) {
                                None
                            } else {
                                Some(EquipSlot::Feet)
                            };
                            selected_equip.set(next);
                        },
                    }
                    EquipmentSlot {
                        slot: EquipSlot::Accessory2,
                        state,
                        view: view.clone(),
                        selected: selected_equip() == Some(EquipSlot::Accessory2),
                        on_select: move |_| {
                            let next = if selected_equip() == Some(EquipSlot::Accessory2) {
                                None
                            } else {
                                Some(EquipSlot::Accessory2)
                            };
                            selected_equip.set(next);
                        },
                    }
                }
                if let Some(slot) = selected_equip() {
                    EquipDetail {
                        slot,
                        state,
                        view: view.clone(),
                        on_close: move |_| selected_equip.set(None),
                    }
                }
                div { class: "inv-sets",
                    for index in 0..4usize {
                        button {
                            key: "{index}",
                            class: if view.active_set == index { "set-btn active" } else { "set-btn" },
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    guard.switch_set(index);
                                }
                                refresh_view(state);
                            },
                            "Set {index + 1}"
                        }
                    }
                }
            }
        }

        if !message_text.is_empty() {
            div { class: "inv-message", "{message_text}" }
        }

        if craft_now {
            CraftScreen {
                state,
                view: view.clone(),
                message,
                on_close: move |_| craft_open.set(false),
            }
        } else {
            section { class: "inv-grid-wrap",
                div { class: "inv-subtitle", "{i18n::t(\"bag.items\")}" }
                div { class: "inv-grid",
                    for item in displayed {
                        ItemCell {
                            key: "{item.id}",
                            item: item.clone(),
                            count: view.item_count(&item.id),
                            selected: selected_now.as_deref() == Some(item.id.as_str()),
                            on_select: move |item_id: String| {
                                selected.set(Some(item_id));
                                message.set(String::new());
                            },
                        }
                    }
                    for index in 0..empty_cells {
                        div { key: "empty-{index}", class: "inv-item empty" }
                    }
                }
            }

            if let Some(item_id) = selected_now {
                if let Some(item) = items::item(&item_id) {
                    InventoryCard {
                        state,
                        view: view.clone(),
                        item: item.clone(),
                        message,
                        on_close: move |_| selected.set(None),
                    }
                }
            } else {
                div { class: "inv-detail empty", "{i18n::t(\"bag.empty_hint\")}" }
            }
        }
    }
}

#[component]
fn EquipmentSlot(
    slot: EquipSlot,
    state: Signal<GameState>,
    view: GameState,
    selected: bool,
    on_select: EventHandler<()>,
) -> Element {
    let _ = state;
    let item = view.equipped_item(slot);
    let class = if selected {
        "equip-cell selected"
    } else {
        "equip-cell"
    };

    rsx! {
        button { class: "{class}", onclick: move |_| on_select.call(()),
            div { class: "equip-cell-icon",
                if let Some(item) = item {
                    ItemIcon {
                        item: (*item).clone(),
                        class: "equip-icon".to_string(),
                    }
                } else {
                    "{slot_short(slot)}"
                }
            }
            div { class: "equip-cell-label", "{slot.label()}" }
            if let Some(item) = item {
                div { class: "equip-cell-name", "{item.name()}" }
            } else {
                div { class: "equip-cell-name empty", "{i18n::t(\"bag.empty_slot\")}" }
            }
        }
    }
}

#[component]
fn EquipDetail(
    slot: EquipSlot,
    state: Signal<GameState>,
    view: GameState,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let item = view.equipped_item(slot);
    let name = item
        .map(|item| item.name())
        .unwrap_or_else(|| i18n::t("bag.empty_slot"));
    let bonus = item.map(|item| item.stats.summary()).unwrap_or_default();
    let can_unequip = item.is_some();

    rsx! {
        div { class: "equip-detail",
            div { class: "equip-detail-head",
                span { class: "equip-detail-label", "{slot.label()}" }
                span { class: "equip-detail-name", "{name}" }
            }
            if !bonus.is_empty() {
                div { class: "equip-detail-bonus", "{bonus}" }
            }
            div { class: "equip-detail-actions",
                if can_unequip {
                    button {
                        class: "inv-tool",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                actions::unequip_slot(&mut guard, slot);
                            }
                            refresh_view(state);
                            on_close.call(());
                        },
                        "{i18n::t(\"btn.unequip\")}"
                    }
                }
                button { class: "inv-tool", onclick: move |_| on_close.call(()), "{i18n::t(\"btn.close\")}" }
            }
        }
    }
}

#[component]
fn ItemCell(item: Item, count: u32, selected: bool, on_select: EventHandler<String>) -> Element {
    let item_id = item.id.clone();
    let class = if selected {
        "inv-item selected"
    } else {
        "inv-item"
    };

    rsx! {
        button {
            class: "{class}",
            onclick: move |_| on_select.call(item_id.clone()),
            ItemIcon { item: item.clone(), class: "inv-item-icon".to_string() }
            if count > 1 {
                span { class: if count >= items::ITEM_STACK_LIMIT { "inv-badge full" } else { "inv-badge" },
                    "{count}"
                }
            }
        }
    }
}

/// Название ингредиента: предмет каталога, а для «огня» (которого нет в
/// каталоге) — отдельная строка словаря.
pub(crate) fn ingredient_name(id: &str) -> &'static str {
    if id == "fire" {
        return i18n::t("res.fire");
    }
    items::item(id)
        .map(|item| item.name())
        .unwrap_or_else(|| i18n::catalog::name(id))
}

/// Экран крафта. Открывается из инвентаря.
#[component]
pub fn CraftScreen(
    state: Signal<GameState>,
    view: GameState,
    message: Signal<String>,
    on_close: EventHandler<()>,
) -> Element {
    // Выбран только один рецепт: карточка живёт снизу сетки, без наслоений.
    let mut open = use_signal(|| None::<&'static str>);

    let open_now = open();
    let blocked = open_now.and_then(|recipe_id| {
        let recipe = items::recipe(recipe_id)?;
        let fire = actions::fire_cost(&view, recipe);
        if recipe.fire_cost > 0 && view.forge_level == 0 {
            Some(i18n::t("craft.blocked_forge").to_string())
        } else if view.current_fire < fire {
            Some(i18n::tf(
                "craft.blocked_fire",
                &[&fire.to_string(), &view.current_fire.to_string()],
            ))
        } else if recipe.id != items::STEAM_PROTOTYPE_RECIPE
            && !view.can_add_item(recipe.result_id, recipe.result_quantity)
        {
            Some(i18n::t("bag.no_room").to_string())
        } else {
            None
        }
    });
    let crafts_now = open_now
        .and_then(items::recipe)
        .map(|recipe| actions::affordable_crafts(&view, recipe))
        .unwrap_or(0);

    rsx! {
        section { class: "inv-craft",
            div { class: "inv-title-row",
                h2 { class: "inv-title", "{i18n::t(\"bag.craft\")}" }
                button { class: "inv-tool", onclick: move |_| on_close.call(()), "{i18n::t(\"btn.back\")}" }
            }
            div { class: "inv-grid",
                for recipe in items::recipes().iter().filter(|recipe| actions::recipe_visible(&view, recipe)) {
                    RecipeCell {
                        key: "{recipe.id}",
                        recipe_id: recipe.id,
                        state,
                        view: view.clone(),
                        selected: open_now == Some(recipe.id),
                        on_select: move |recipe_id: &'static str| {
                            open.set(if open_now == Some(recipe_id) { None } else { Some(recipe_id) });
                        },
                    }
                }
            }
            if let Some(recipe_id) = open_now {
                if let Some(recipe) = items::recipe(recipe_id) {
                    CraftPanel {
                        recipe: recipe.clone(),
                        view: view.clone(),
                        crafts_now,
                        blocked,
                        state,
                        message,
                    }
                }
            }
        }
    }
}

/// Квадратик рецепта в сетке.
#[component]
fn RecipeCell(
    recipe_id: &'static str,
    state: Signal<GameState>,
    view: GameState,
    selected: bool,
    on_select: EventHandler<&'static str>,
) -> Element {
    let _ = state;
    let Some(recipe) = items::recipe(recipe_id) else {
        return rsx! {};
    };
    let Some(result) = items::item(recipe.result_id) else {
        return rsx! {};
    };
    let class = if selected {
        "inv-item selected"
    } else {
        "inv-item"
    };

    rsx! {
        button { class: "{class}", onclick: move |_| on_select.call(recipe_id),
            ItemIcon { item: (*result).clone(), class: "inv-item-icon".to_string() }
            if let Some(crafts) = crafts_badge(&view, recipe) {
                span { class: "inv-badge", "{crafts}" }
            }
        }
    }
}

/// Значок «сколько раз можно скрафтить прямо сейчас».
fn crafts_badge(view: &GameState, recipe: &items::Recipe) -> Option<u32> {
    let crafts = actions::affordable_crafts(view, recipe);
    (crafts > 0).then_some(crafts.min(99))
}

/// Описание выбранного рецепта: слева данные, справа иконка и ресурсы.
#[component]
fn CraftPanel(
    recipe: items::Recipe,
    view: GameState,
    crafts_now: u32,
    blocked: Option<String>,
    state: Signal<GameState>,
    message: Signal<String>,
) -> Element {
    let mut state = state;
    let mut message = message;
    let Some(result) = items::item(recipe.result_id) else {
        return rsx! {};
    };

    let name = recipe.name();
    let category = result.category.label();
    let stats = result.stats.summary();
    let owned = view.item_count(result.id.as_str());
    let result_id = result.id.clone();

    let mut missing = false;
    let ingredients: Vec<(String, u32, u32, bool)> = recipe
        .ingredients
        .iter()
        .map(|ingredient| {
            let need = ingredient.quantity;
            let have = view.ingredient_amount(ingredient.item_id);
            let lack = have < need;
            missing |= lack;
            (
                ingredient_name(ingredient.item_id).to_string(),
                have,
                need,
                lack,
            )
        })
        .collect();

    let can_one = crafts_now > 0 && blocked.is_none() && !missing;
    let can_ten = can_one && crafts_now >= 10;
    let can_hundred = can_one && crafts_now >= 100;
    let recipe_id = recipe.id.to_string();
    let result_quantity = recipe.result_quantity;
    let single_label = if recipe.id == items::STEAM_PROTOTYPE_RECIPE {
        i18n::t("btn.make").to_string()
    } else {
        i18n::t("btn.craft").to_string()
    };
    let make_button = move |times: u32| -> Element {
        let recipe_id = recipe_id.clone();
        let single_label = single_label.clone();
        let enabled = match times {
            1 => can_one,
            10 => can_ten,
            _ => can_hundred,
        };
        let label = match times {
            1 => single_label,
            other => i18n::tf("btn.craft_n", &[&other.to_string()]),
        };
        rsx! {
            button {
                class: if enabled { "craft-btn" } else { "craft-btn disabled" },
                disabled: !enabled,
                onclick: move |_| {
                    let result = {
                        let mut guard = state.write();
                        actions::craft_times(&mut guard, &recipe_id, times)
                    };
                    message
                        .set(
                            match result {
                                Ok(_) => String::new(),
                                Err(error) => craft_error_text(error).to_string(),
                            },
                        );
                    refresh_view(state);
                },
                "{label}"
            }
        }
    };

    rsx! {
        section { class: "craft-panel",
            div { class: "craft-panel-top",
                div { class: "craft-panel-left",
                    div { class: "craft-panel-name", "{name}" }
                    div { class: "craft-panel-type", "{category}" }
                    div { class: "craft-panel-stats", "{stats}" }
                    div { class: "craft-panel-owned", "{i18n::tf(\"bag.owned\", &[&owned.to_string()])}" }
                }
                div { class: "craft-panel-icon",
                    ItemIcon {
                        item: (*result).clone(),
                        class: "craft-icon".to_string(),
                    }
                }
            }

            div { class: "craft-panel-bottom",
                div { class: "craft-panel-buttons",
                    {make_button(1)}
                    if can_ten {
                        {make_button(10)}
                    }
                    if can_hundred {
                        {make_button(100)}
                    }
                }
                div { class: "craft-panel-cost",
                    for (ingredient , have , need , lack) in ingredients.iter() {
                        div {
                            key: "{ingredient}",
                            class: if *lack { "craft-need lack" } else { "craft-need" },
                            "{ingredient}: {have}/{need}"
                        }
                    }
                    if result_quantity > 1 {
                        div { class: "craft-need",
                            "{i18n::tf(\"bag.output\", &[&result_quantity.to_string()])}"
                        }
                    }
                }
            }

            if let Some(text) = blocked {
                div { class: "craft-panel-blocked", "{text}" }
            }
            if !result_id.is_empty() {
                if missing {
                    div { class: "craft-panel-hint", "{i18n::t(\"bag.lack\")}" }
                }
            }
        }
    }
}

#[component]
fn InventoryCard(
    state: Signal<GameState>,
    view: GameState,
    item: Item,
    message: Signal<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let mut message = message;
    let count = view.item_count(&item.id);
    let equipped_slot = match item.slot {
        Some(EquipSlot::Accessory1) => {
            if view.equipment.get(EquipSlot::Accessory1) == Some(item.id.as_str()) {
                Some(EquipSlot::Accessory1)
            } else if view.equipment.get(EquipSlot::Accessory2) == Some(item.id.as_str()) {
                Some(EquipSlot::Accessory2)
            } else {
                None
            }
        }
        Some(slot) if view.equipment.get(slot) == Some(item.id.as_str()) => Some(slot),
        _ => None,
    };
    let equipped = equipped_slot.is_some();
    let can_equip = item.slot.is_some() && count > 0 && !equipped;
    let item_id = item.id.clone();
    let study_item_id = item_id.clone();
    let item_name = item.name();
    let category = item.category.label();
    let rarity = item.rarity();
    let description = item.description();
    let stats = item.stats.summary();
    let base_price = item.base_price;
    let slot = item.slot;
    let message_now = message();
    let map_location = expedition::location_for_map(&item.id);
    let map_open = map_location
        .map(|location_id| view.location_unlocked(location_id))
        .unwrap_or(false);
    let can_study_map = map_location.is_some() && count > 0 && !map_open;
    let is_blueprint = item.id == items::STEAM_BLUEPRINT_ID;
    let blueprint_item_id = item_id.clone();
    let blueprint_studied = view.steam_blueprint_studied;
    let can_study_blueprint = is_blueprint && count > 0 && !blueprint_studied;
    let mut map_ok = use_signal(|| false);

    rsx! {
        section { class: "inv-detail",
            div { class: "inv-detail-head",
                ItemIcon { item: item.clone(), class: "inv-detail-icon".to_string() }
                span { class: "inv-detail-name", "{item_name}" }
                span { class: "inv-detail-price",
                    "{i18n::tf(\"bag.price\", &[&base_price.to_string()])}"
                }
                button {
                    class: "inv-detail-close",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div { class: "inv-detail-slot", "[{category} · {rarity}]" }
            div { class: "inv-detail-desc", "{description}" }
            div { class: "inv-detail-stats",
                span {
                    "{i18n::tf(\"bag.in_bag\", &[&count.to_string(), &items::ITEM_STACK_LIMIT.to_string()])}"
                }
                span { "{i18n::tf(\"bag.bonuses\", &[&stats])}" }
                if let Some(slot) = slot {
                    span { "{i18n::tf(\"bag.slot_line\", &[slot.label()])}" }
                }
                if let Some(location_id) = map_location {
                    if let Some(location) = expedition::location(location_id) {
                        span { "{i18n::tf(\"bag.location_line\", &[location.name()])}" }
                    }
                    if map_open {
                        span { "{i18n::t(\"bag.map_open\")}" }
                    }
                }
                if is_blueprint {
                    span {
                        if blueprint_studied {
                            "{i18n::t(\"bag.blueprint_done\")}"
                        } else {
                            "{i18n::t(\"bag.blueprint_new\")}"
                        }
                    }
                }
            }

            if !message_now.is_empty() {
                div { class: if map_ok() { "trade-error ok" } else { "trade-error" }, "{message_now}" }
            }

            div { class: "inv-detail-actions",
                if map_location.is_some() {
                    button {
                        class: if can_study_map { "inv-tool" } else { "inv-tool disabled" },
                        disabled: !can_study_map,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::study_map(&mut guard, &study_item_id)
                            };
                            map_ok.set(result.is_ok());
                            message
                                .set(
                                    match result {
                                        Ok(location_id) => {
                                            let location_name = expedition::location(location_id)
                                                .map(|location| location.name())
                                                .unwrap_or_else(|| i18n::t("hike.new_location"));
                                            i18n::tf("bag.map_opened", &[location_name])
                                        }
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            refresh_view(state);
                        },
                        "{i18n::t(\"btn.study_map\")}"
                    }
                }
                if is_blueprint {
                    button {
                        class: if can_study_blueprint { "inv-tool" } else { "inv-tool disabled" },
                        disabled: !can_study_blueprint,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::study_blueprint(&mut guard, &blueprint_item_id)
                            };
                            map_ok.set(result.is_ok());
                            message
                                .set(
                                    match result {
                                        Ok(()) => {
                                            i18n::t("bag.blueprint_studied").to_string()
                                        }
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            refresh_view(state);
                        },
                        if blueprint_studied {
                            "{i18n::t(\"bag.blueprint_done\")}"
                        } else {
                            "{i18n::t(\"btn.study_blueprint\")}"
                        }
                    }
                }
                if slot.is_some() {
                    if let Some(equipped_slot) = equipped_slot {
                        button {
                            class: "inv-tool",
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    actions::unequip_slot(&mut guard, equipped_slot);
                                }
                                refresh_view(state);
                            },
                            "{i18n::t(\"btn.unequip\")}"
                        }
                    } else {
                        button {
                            class: if can_equip { "inv-tool" } else { "inv-tool disabled" },
                            disabled: !can_equip,
                            onclick: move |_| {
                                let result = {
                                    let mut guard = state.write();
                                    actions::equip_item(&mut guard, &item_id)
                                };
                                message
                                    .set(
                                        match result {
                                            Ok(()) => String::new(),
                                            Err(error) => equip_error_text(error).to_string(),
                                        },
                                    );
                                refresh_view(state);
                            },
                            "{i18n::t(\"btn.equip\")}"
                        }
                    }
                }
                button { class: "inv-tool", onclick: move |_| on_close.call(()), "{i18n::t(\"btn.close\")}" }
            }
        }
    }
}

fn slot_short(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Hand => i18n::t("slot.short.hand"),
        EquipSlot::Body => i18n::t("slot.short.body"),
        EquipSlot::Head => i18n::t("slot.short.head"),
        EquipSlot::Feet => i18n::t("slot.short.feet"),
        EquipSlot::Accessory1 => i18n::t("slot.short.accessory1"),
        EquipSlot::Accessory2 => i18n::t("slot.short.accessory2"),
    }
}

fn craft_error_text(error: CraftError) -> &'static str {
    error.text()
}

fn equip_error_text(error: EquipError) -> &'static str {
    error.text()
}
