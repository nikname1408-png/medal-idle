//! Инвентарь, экипировка и крафт.

use dioxus::prelude::*;

use crate::game::actions::{self, CraftError, ExpeditionError};
use crate::game::items::{self, Category, EquipSlot, Item};
use crate::game::state::{is_active_resource, EquipError, GameState};

use super::app::{format_num, refresh_view};
use super::trader::format_money;

/// Полный экран инвентаря.
#[component]
pub fn InventoryScreen(
    state: Signal<GameState>,
    view: GameState,
    back: Signal<u32>,
    on_back: EventHandler<()>,
    on_trade: EventHandler<()>,
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
    let trade_percent = (view.trade_bonus * 100.0).round() as i32;
    // В сетке — только штучные предметы; активные ресурсы живут на складе.
    let mut displayed: Vec<Item> = items::catalog()
        .iter()
        .filter(|item| !is_active_resource(&item.id))
        .cloned()
        .collect();
    if sort_alpha() {
        displayed.sort_by(|left, right| left.name.cmp(&right.name));
    }

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
                        onclick: move |_| craft_open.set(!craft_now),
                        "Создать предметы"
                    }
                }
            }
            div { class: "inv-stats",
                span { class: "inv-stat", "Золото: {format_money(view.gold)}" }
                span { class: "inv-stat",
                    "Энергия: {format_num(view.energy)}/{format_num(view.max_energy())}"
                }
                span { class: "inv-stat",
                    "Ячейки: {view.inventory_used_slots()}/{view.inventory_slots()}"
                }
                span { class: "inv-stat", "Торговля: +{trade_percent}%" }
            }
        }

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
                    "Выбери предмет, чтобы увидеть название, цену и описание."
                }
            }
        }

        div { class: "inv-actions",
            button {
                class: "inv-tool",
                onclick: move |_| do_expedition(state, message),
                "Поход"
            }
            button {
                class: "inv-tool",
                onclick: move |_| do_add_resources(state, message),
                "+ Ресурсы"
            }
            button {
                class: "inv-tool",
                onclick: move |_| on_trade.call(()),
                "Торговец"
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
    let slot_icon = item.map(item_icon).unwrap_or_else(|| slot_short(slot));
    let can_unequip = item.is_some();

    rsx! {
        div { class: "equip-row",
            div { class: "equip-slot", "{slot_icon}" }
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
    let icon = item_icon(&item);
    let class = if count == 0 {
        "inv-item empty"
    } else if selected {
        "inv-item selected"
    } else {
        "inv-item"
    };

    rsx! {
        button {
            class: "{class}",
            onclick: move |_| on_select.call(item_id.clone()),
            span { class: "inv-item-icon", "{icon}" }
            if count > 0 {
                span { class: "inv-badge", "{count}" }
            }
        }
    }
}

/// Текстовая заглушка иконки предмета.
fn item_icon(item: &Item) -> &'static str {
    match item.id.as_str() {
        "iron_ore" => "⛏",
        "wood" => "🪵",
        "leather" => "🟫",
        "fiber" => "🌾",
        "silver" => "⚪",
        "quartz" => "💎",
        "iron_ingot" => "🔩",
        "cloth" => "🧵",
        "strength_potion" => "🧪",
        "backpack" => "🎒",
        "trader_ring" => "💍",
        "iron_sword" => "🗡",
        "iron_armor" => "🛡",
        "coal_amulet" => "🔮",
        "brick" => "🧱",
        "board" => "🪵",
        _ => match item.category {
            Category::Material => "📦",
            Category::Gear => "⚔",
            Category::Jewelry => "💍",
            Category::Goods => "🧵",
            Category::Consumable => "🧪",
        },
    }
}

/// Название ингредиента: активный ресурс склада или предмет каталога.
fn ingredient_name(id: &str) -> &str {
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
            div { class: "craft-list",
                for recipe in items::recipes() {
                    RecipeRow {
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

#[component]
fn RecipeRow(
    recipe: items::Recipe,
    state: Signal<GameState>,
    view: GameState,
    message: Signal<String>,
) -> Element {
    let mut state = state;
    let mut message = message;
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
    let fire_cost = recipe.fire_cost;
    let fire_now = view.current_fire;
    let fire_class = if fire_ok {
        "craft-fire"
    } else {
        "craft-fire poor"
    };

    rsx! {
        div { class: "craft-row",
            div { class: "craft-main",
                div { class: "craft-name", "{result_name}" }
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
                    refresh_view(state);
                },
                "Создать"
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
    let icon = item_icon(&item);
    let message_now = message();

    rsx! {
        section { class: "inv-detail",
            div { class: "inv-detail-head",
                span { class: "inv-detail-icon", "{icon}" }
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

fn do_expedition(mut state: Signal<GameState>, mut message: Signal<String>) {
    let result = {
        let mut guard = state.write();
        actions::simulate_expedition(&mut guard)
    };
    message.set(match result {
        Ok(loot) => loot
            .into_iter()
            .map(|(item_id, quantity)| {
                let name = ingredient_name(&item_id);
                format!("Поход: +{quantity} × {name}")
            })
            .collect::<Vec<_>>()
            .join(", "),
        Err(error) => expedition_error_text(error).to_string(),
    });
    refresh_view(state);
}

fn do_add_resources(mut state: Signal<GameState>, mut message: Signal<String>) {
    let added = {
        let mut guard = state.write();
        actions::add_demo_resources(&mut guard)
    };
    message.set(if added == 0 {
        "Сумка полна — освободи место.".to_string()
    } else {
        format!("Добавлено материалов: {added}.")
    });
    refresh_view(state);
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

fn expedition_error_text(error: ExpeditionError) -> &'static str {
    error.text()
}

fn equip_error_text(error: EquipError) -> &'static str {
    error.text()
}
