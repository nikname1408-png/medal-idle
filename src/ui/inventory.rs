//! Инвентарь, экипировка и крафт.

use dioxus::prelude::*;

use crate::game::actions::{self, CraftError};
use crate::game::items::{self, Category, EquipSlot, Item};
use crate::game::state::{EquipError, GameState};

use super::app::refresh_view;

const ICON_BAG: Asset = asset!("/assets/icons/bag.png");
const ICON_COAL: Asset = asset!("/assets/icons/coal.png");
const ICON_FIRE: Asset = asset!("/assets/icons/fire.png");
const ICON_GEAR: Asset = asset!("/assets/icons/gear.png");
const ICON_GOLD: Asset = asset!("/assets/icons/gold.png");
const ICON_IRON: Asset = asset!("/assets/icons/iron.png");
const ICON_SCROLL: Asset = asset!("/assets/icons/scroll.png");
const ICON_STONE: Asset = asset!("/assets/icons/stone.png");
const ICON_WOOD: Asset = asset!("/assets/icons/wood.png");

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
        displayed.sort_by(|left, right| left.name.cmp(&right.name));
    }
    // Пустые ячейки до размера сумки: без рюкзака стандарт, с рюкзаком — больше.
    let total_slots = view.inventory_slots() as usize;
    let item_cells = displayed.len();
    let empty_cells = total_slots.saturating_sub(item_cells);

    rsx! {
        section { class: "inv-head",
            div { class: "inv-title-row",
                h2 { class: "inv-title", "Инвентарь" }
                div { class: "inv-head-actions",
                    button {
                        class: if sort_alpha() { "inv-tool active" } else { "inv-tool" },
                        onclick: move |_| {
                            let next = !sort_alpha();
                            sort_alpha.set(next);
                            message.set(
                                if next {
                                    "Сортировка: по названию".to_string()
                                } else {
                                    "Сортировка: по каталогу".to_string()
                                },
                            );
                        },
                        "Сортировать"
                    }
                    button {
                        class: if craft_now { "inv-tool active" } else { "inv-tool" },
                        onclick: move |_| {
                            craft_open.set(!craft_now);
                            selected.set(None);
                        },
                        "Создать предметы"
                    }
                }
            }
        }

        // Во время крафта экипировка скрыта — только сетка предметов.
        if !craft_now {
            section { class: "inv-equipment",
                div { class: "inv-subtitle", "Экипировка" }
                for slot in EquipSlot::ALL {
                    EquipmentSlot {
                        key: "{slot.label()}",
                        slot,
                        state,
                        view: view.clone(),
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
                div { class: "inv-subtitle", "Предметы" }
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
                div { class: "inv-detail empty",
                    "Нажми на предмет, чтобы увидеть название, цену и описание."
                }
            }
        }
    }
}

#[component]
fn EquipmentSlot(slot: EquipSlot, state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let item = view.equipped_item(slot);
    let name = item.map(|item| item.name.as_str()).unwrap_or("Пусто");
    let bonus = item.map(|item| item.stats.summary()).unwrap_or_default();
    let slot_icon = item.map(|item| item_icon(item).0);
    let can_unequip = item.is_some();

    rsx! {
        div { class: "equip-row",
            div { class: "equip-slot",
                if let Some(icon) = slot_icon {
                    img { class: "icon", src: "{icon}", alt: "" }
                } else {
                    "{slot_short(slot)}"
                }
            }
            div { class: "equip-main",
                span { class: "equip-label", "{slot.label()}" }
                span { class: "equip-value", "{name}" }
                if !bonus.is_empty() {
                    span { class: "equip-bonus", "{bonus}" }
                }
            }
            button {
                class: if can_unequip { "equip-btn" } else { "equip-btn disabled" },
                disabled: !can_unequip,
                onclick: move |_| {
                    {
                        let mut guard = state.write();
                        actions::unequip_slot(&mut guard, slot);
                    }
                    refresh_view(state);
                },
                "Снять"
            }
        }
    }
}

#[component]
fn ItemCell(item: Item, count: u32, selected: bool, on_select: EventHandler<String>) -> Element {
    let item_id = item.id.clone();
    let icon = item_icon(&item).0;
    let class = if selected {
        "inv-item selected"
    } else {
        "inv-item"
    };

    rsx! {
        button {
            class: "{class}",
            onclick: move |_| on_select.call(item_id.clone()),
            img { class: "inv-item-icon icon", src: "{icon}", alt: "" }
            if count > 1 {
                span { class: "inv-badge", "{count}" }
            }
        }
    }
}

/// Иконка предмета: загруженный PNG.
struct ItemIcon(Asset);

fn item_icon(item: &Item) -> ItemIcon {
    let asset = match item.id.as_str() {
        "wood" | "board" => ICON_WOOD,
        "leather" => ICON_STONE,
        "silver" | "cloth" => ICON_GOLD,
        "quartz" => ICON_STONE,
        "iron_ingot" => ICON_IRON,
        "backpack" => ICON_BAG,
        "trader_ring" => ICON_GOLD,
        "iron_sword" => ICON_IRON,
        "iron_armor" => ICON_GEAR,
        "coal_amulet" => ICON_COAL,
        "brick" => ICON_STONE,
        "clay" | "fiber" => ICON_STONE,
        _ => {
            if item.category == Category::Gear {
                ICON_GEAR
            } else if item.fire_cost > 0 {
                ICON_FIRE
            } else if item.slot.is_some() {
                ICON_SCROLL
            } else {
                ICON_COAL
            }
        }
    };
    ItemIcon(asset)
}

/// Название ингредиента: активный ресурс склада или предмет каталога.
pub(crate) fn ingredient_name(id: &str) -> &str {
    match id {
        "wood" => "Дерево",
        "stone" => "Камень",
        "iron" => "Руда",
        "clay" => "Глина",
        "coal" => "Уголь",
        _ => items::item(id).map(|item| item.name.as_str()).unwrap_or(id),
    }
}

/// Экран крафта. Открывается из инвентаря.
#[component]
pub fn CraftScreen(
    state: Signal<GameState>,
    view: GameState,
    message: Signal<String>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        section { class: "inv-craft",
            div { class: "inv-title-row",
                h2 { class: "inv-title", "Крафт" }
                button {
                    class: "inv-tool",
                    onclick: move |_| on_close.call(()),
                    "Назад"
                }
            }
            div { class: "inv-grid",
                for recipe in items::recipes() {
                    RecipeCell {
                        key: "{recipe.id}",
                        recipe: recipe.clone(),
                        state,
                        view: view.clone(),
                        message,
                    }
                }
            }
        }
    }
}

/// Квадратик рецепта: иконка предмета, выпадающая карточка по тапу.
#[component]
fn RecipeCell(
    recipe: items::Recipe,
    state: Signal<GameState>,
    view: GameState,
    message: Signal<String>,
) -> Element {
    let mut state = state;
    let mut message = message;
    let mut open = use_signal(|| false);
    let Some(result) = items::item(recipe.result_id) else {
        return rsx! {};
    };

    let ingredients = recipe
        .ingredients
        .iter()
        .map(|ingredient| {
            let name = ingredient_name(ingredient.item_id);
            let owned = view.ingredient_amount(ingredient.item_id);
            format!("{name} {owned}/{}", ingredient.quantity)
        })
        .collect::<Vec<_>>()
        .join(" · ");

    let enough = recipe
        .ingredients
        .iter()
        .all(|ingredient| view.ingredient_amount(ingredient.item_id) >= ingredient.quantity);
    let has_space = view.can_add_item(recipe.result_id, recipe.result_quantity);
    let has_forge = recipe.fire_cost == 0 || view.forge_level > 0;
    let fire_ok = has_forge && view.current_fire >= recipe.fire_cost;
    let available = enough && has_space && fire_ok;
    let recipe_id = recipe.id.to_string();
    let result_name = result.name.clone();
    let result_description = result.description.clone();
    let fire_cost = recipe.fire_cost;
    let fire_now = view.current_fire;
    let icon = item_icon(result).0;
    let fire_class = if fire_ok {
        "craft-fire"
    } else {
        "craft-fire poor"
    };
    let open_now = open();

    rsx! {
        div { class: "craft-cell",
            button {
                class: if open_now { "inv-item selected" } else { "inv-item" },
                onclick: move |_| open.set(!open_now),
                img { class: "inv-item-icon icon", src: "{icon}", alt: "" }
            }
            if open_now {
                div { class: "craft-card",
                    div { class: "craft-name", "{result_name}" }
                    div { class: "craft-desc", "{result_description}" }
                    div { class: "craft-cost", "{ingredients}" }
                    if fire_cost > 0 {
                        div { class: "{fire_class}",
                            "Огонь: нужно {fire_cost} / в кузнице {fire_now}"
                        }
                    }
                    if !has_space {
                        div { class: "craft-warning", "Нет места в сумке" }
                    } else if !fire_ok {
                        div { class: "craft-warning poor", "Недостаточно огня!" }
                    }
                    button {
                        class: if available { "craft-btn" } else { "craft-btn disabled" },
                        disabled: !available,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::craft(&mut guard, &recipe_id)
                            };
                            message.set(match result {
                                Ok(()) => String::new(),
                                Err(error) => craft_error_text(error).to_string(),
                            });
                            open.set(false);
                            refresh_view(state);
                        },
                        "Создать"
                    }
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
    let equipped = item
        .slot
        .map(|slot| view.equipment.get(slot) == Some(item.id.as_str()))
        .unwrap_or(false);
    let can_equip = item.slot.is_some() && count > 0 && !equipped;
    let item_id = item.id.clone();
    let item_name = item.name.clone();
    let category = item.category.label();
    let rarity = item.rarity();
    let description = item.description.clone();
    let stats = item.stats.summary();
    let base_price = item.base_price;
    let slot = item.slot;
    let icon = item_icon(&item).0;
    let message_now = message();

    rsx! {
        section { class: "inv-detail",
            div { class: "inv-detail-head",
                img { class: "inv-detail-icon icon", src: "{icon}", alt: "" }
                span { class: "inv-detail-name", "{item_name}" }
                span { class: "inv-detail-price", "{base_price} зл." }
                button {
                    class: "inv-detail-close",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div { class: "inv-detail-slot", "[{category} · {rarity}]" }
            div { class: "inv-detail-desc", "{description}" }
            div { class: "inv-detail-stats",
                span { "В сумке: {count}" }
                span { "Бонусы: {stats}" }
                if let Some(slot) = slot {
                    span { "Слот: {slot.label()}" }
                }
            }

            if !message_now.is_empty() {
                div { class: "trade-error", "{message_now}" }
            }

            div { class: "inv-detail-actions",
                if let Some(slot) = slot {
                    if equipped {
                        button {
                            class: "inv-tool",
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    actions::unequip_slot(&mut guard, slot);
                                }
                                refresh_view(state);
                            },
                            "Снять"
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
                                message.set(match result {
                                    Ok(()) => String::new(),
                                    Err(error) => equip_error_text(error).to_string(),
                                });
                                refresh_view(state);
                            },
                            "Надеть"
                        }
                    }
                }
                button {
                    class: "inv-tool",
                    onclick: move |_| on_close.call(()),
                    "Закрыть"
                }
            }
        }
    }
}

fn slot_short(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Hand => "Р",
        EquipSlot::Body => "Т",
        EquipSlot::Accessory => "А",
    }
}

fn craft_error_text(error: CraftError) -> &'static str {
    error.text()
}

fn equip_error_text(error: EquipError) -> &'static str {
    error.text()
}
