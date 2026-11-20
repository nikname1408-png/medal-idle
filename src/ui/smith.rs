//! Вкладка «Кузнец» внутри «Труда»: два мастера, очередь рецептов,
//! три линии улучшений, горн и жалованье.

use dioxus::prelude::*;

use crate::game::actions;
use crate::game::balance;
use crate::game::city::{self, SmithLine};
use crate::game::i18n;
use crate::game::items;
use crate::game::GameState;

use super::app::{format_duration, format_num, refresh_view, CostLabel};
use super::inventory::{ingredient_name, ItemIcon};

/// Золото с подписью ресурса.
fn money(value: f64) -> String {
    format!("{} {}", format_num(value), i18n::t("res.gold"))
}

/// Вкладка «Кузнец» в «Труде».
#[component]
pub fn SmithSection(state: Signal<GameState>, view: GameState) -> Element {
    rsx! {
        if !view.city.unlocked {
            div { class: "locked-note", "{i18n::t(\"smith.locked\")}" }
        } else {
            SmithCard { state, view: view.clone(), index: 0 }
            SmithCard { state, view: view.clone(), index: 1 }
        }
    }
}

/// Карточка одного кузнеца.
#[component]
fn SmithCard(state: Signal<GameState>, view: GameState, index: usize) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut picker = use_signal(|| false);

    let smith = view.city.smiths[index].clone();
    let title = i18n::tf("smith.title", &[&(index + 1).to_string()]);

    if !smith.built {
        let cost = balance::smith_build_cost(index);
        let can_build = actions::can_afford(&view, cost);
        let need_permit = index > 0 && !view.city.smith_second_permit;
        let build_label = i18n::tf("smith.build", &[&money(cost.gold)]);
        return rsx! {
            section { class: "section smith-card",
                div { class: "section-head static",
                    span { class: "section-title", "{title}" }
                }
                div { class: "section-body",
                    p { class: "smith-hint", "{i18n::t(\"smith.build_hint\")}" }
                    if need_permit {
                        p { class: "smith-warn", "{i18n::t(\"smith.need_permit\")}" }
                    }
                    CostLabel { view: view.clone(), cost }
                    button {
                        class: "city-btn",
                        disabled: !can_build || need_permit,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::build_smith(&mut guard, index)
                            };
                            match outcome {
                                Ok(()) => message.set(String::new()),
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{build_label}"
                    }
                    if !message().is_empty() {
                        div { class: "city-msg", "{message()}" }
                    }
                }
            }
        };
    }

    let now = view.total_play_seconds;
    let queue = smith.recipes.clone();
    let slots = city::smith_slots(&view, index);
    let next_in = (smith.next_craft_at - now).max(0.0);
    let next_text = if queue.is_empty() || smith.paused {
        String::new()
    } else {
        i18n::tf("smith.next_craft", &[&format_duration(next_in)])
    };
    let status_text = if smith.paused {
        i18n::t("smith.status.paused")
    } else if queue.is_empty() {
        i18n::t("smith.status.idle")
    } else {
        i18n::t("smith.status.working")
    };
    let now_item = queue
        .first()
        .and_then(|id| items::recipe(id))
        .and_then(|recipe| items::item(recipe.result_id))
        .cloned();
    let fire_capacity = balance::smith_fire_capacity(smith.forge_level);
    let per_coal = balance::smith_fire_per_coal(smith.forge_level);
    let fire_text = i18n::tf(
        "smith.fire_line",
        &[
            &smith.fire.to_string(),
            &fire_capacity.to_string(),
            &per_coal.to_string(),
        ],
    );
    let wage_text = i18n::tf("smith.wage_line", &[&money(smith.wage_due)]);
    let coal_text = i18n::tf(
        "smith.coal_line",
        &[&format_num(view.active_amount("coal") as f64)],
    );
    let queue_text = i18n::tf(
        "smith.queue_line",
        &[&queue.len().to_string(), &slots.to_string()],
    );
    let pay_left = (view.city.next_salary_at - now).max(0.0);
    let pay_text = i18n::tf("hire.next_pay", &[&format_duration(pay_left)]);

    let cells: Vec<Option<String>> = (0..slots).map(|i| queue.get(i).cloned()).collect();
    let picker_list: Vec<&'static str> = items::recipes()
        .iter()
        .filter(|recipe| actions::recipe_visible(&view, recipe))
        .filter(|recipe| !queue.iter().any(|id| id == recipe.id))
        .map(|recipe| recipe.id)
        .collect();
    let picker_open = picker();
    let selected_now = selected();

    rsx! {
        section { class: "section smith-card",
            div { class: "section-head static",
                span { class: "section-title", "{title}" }
                span { class: "smith-status", "{status_text}" }
            }
            div { class: "section-body",
                if smith.paused {
                    div { class: "city-msg", "{i18n::t(\"smith.paused\")}" }
                }
                div { class: "smith-now",
                    span { class: "smith-now-label", "{i18n::t(\"smith.now\")}" }
                    if let Some(item) = now_item {
                        ItemIcon { item, class: "smith-now-icon".to_string() }
                    } else {
                        span { class: "smith-now-empty", "{i18n::t(\"smith.now_empty\")}" }
                    }
                    if !next_text.is_empty() {
                        span { class: "smith-timer", "{next_text}" }
                    }
                }
                div { class: "smith-meta",
                    span { "{fire_text}" }
                    span { "{wage_text}" }
                }
                div { class: "smith-meta",
                    span { "{coal_text}" }
                    span { "{pay_text}" }
                }
                div { class: "smith-queue-label",
                    span { "{queue_text}" }
                    span { class: "smith-hint-inline", "{i18n::t(\"smith.queue_hint\")}" }
                }
                div { class: "inv-grid smith-grid",
                    for (position , cell) in cells.into_iter().enumerate() {
                        {
                            match cell {
                                Some(recipe_id) => {
                                    let id = recipe_id.clone();
                                    let is_selected = selected_now.as_deref()
                                        == Some(recipe_id.as_str());
                                    let item = items::recipe(&recipe_id)
                                        .and_then(|recipe| items::item(recipe.result_id))
                                        .cloned();
                                    rsx! {
                                        button {
                                            class: if is_selected { "inv-item selected" } else { "inv-item" },
                                            onclick: move |_| {
                                                selected.set(Some(id.clone()));
                                                picker.set(false);
                                            },
                                            if let Some(item) = item {
                                                ItemIcon { item, class: "inv-item-icon".to_string() }
                                            }
                                            span { class: "smith-order", "{position + 1}" }
                                        }
                                    }
                                }
                                None => rsx! {
                                    button {
                                        class: "inv-item empty smith-add",
                                        onclick: move |_| picker.set(!picker_open),
                                        "＋"
                                    }
                                },
                            }
                        }
                    }
                }
                if picker_open {
                    div { class: "smith-picker",
                        for recipe_id in picker_list {
                            {
                                let id = recipe_id.to_string();
                                let item = items::recipe(recipe_id)
                                    .and_then(|recipe| items::item(recipe.result_id))
                                    .cloned();
                                rsx! {
                                    button {
                                        class: "inv-item",
                                        onclick: move |_| {
                                            selected.set(Some(id.clone()));
                                            picker.set(false);
                                        },
                                        if let Some(item) = item {
                                            ItemIcon { item, class: "inv-item-icon".to_string() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(selected_id) = selected_now.clone() {
                    SmithRecipeCard {
                        state,
                        view: view.clone(),
                        index,
                        recipe_id: selected_id,
                        queue: queue.clone(),
                        smith_fire: smith.fire,
                        message,
                        on_close: move |_| selected.set(None),
                    }
                }
                SmithUpgradeRow { state, view: view.clone(), index, line: SmithLine::Speed, message }
                SmithUpgradeRow { state, view: view.clone(), index, line: SmithLine::Forge, message }
                SmithUpgradeRow { state, view: view.clone(), index, line: SmithLine::Slots, message }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
            }
        }
    }
}

/// Карточка выбранного рецепта: в крафт, из крафта и стрелки приоритета.
#[component]
fn SmithRecipeCard(
    state: Signal<GameState>,
    view: GameState,
    index: usize,
    recipe_id: String,
    queue: Vec<String>,
    smith_fire: u32,
    message: Signal<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let mut message = message;
    let Some(recipe) = items::recipe(&recipe_id) else {
        return rsx! {};
    };
    let Some(result) = items::item(recipe.result_id) else {
        return rsx! {};
    };
    let in_queue = queue.iter().any(|id| id == &recipe_id);
    let position = queue.iter().position(|id| id == &recipe_id);
    let slots = city::smith_slots(&view, index);
    let full = queue.len() >= slots;
    let min_level = items::recipe_min_smith_level(recipe.id);
    let locked = view.smith_level() < min_level;

    let fire_need = actions::fire_cost(&view, recipe);
    let ingredients: Vec<(String, u32, u32, bool)> = recipe
        .ingredients
        .iter()
        .map(|ingredient| {
            let need = ingredient.quantity;
            let have = if ingredient.item_id == "fire" {
                smith_fire
            } else {
                view.ingredient_amount(ingredient.item_id)
            };
            (
                ingredient_name(ingredient.item_id).to_string(),
                have,
                need,
                have < need,
            )
        })
        .collect();

    let can_up = position.map(|p| p > 0).unwrap_or(false);
    let can_down = position.map(|p| p + 1 < queue.len()).unwrap_or(false);
    let id_up = recipe_id.clone();
    let id_down = recipe_id.clone();
    let id_remove = recipe_id.clone();
    let id_add = recipe_id.clone();
    let add_label = if full {
        i18n::t("smith.full").to_string()
    } else if locked {
        i18n::tf("craft.requires_level", &[&min_level.to_string()])
    } else {
        i18n::t("smith.add_to_craft").to_string()
    };

    rsx! {
        section { class: "craft-panel smith-recipe-card",
            div { class: "craft-panel-top",
                div { class: "craft-panel-left",
                    div { class: "craft-panel-name", "{recipe.name()}" }
                    div { class: "craft-panel-type", "{result.category.label()}" }
                    div { class: "craft-panel-cost",
                        for (name , have , need , lack) in ingredients.iter() {
                            div { key: "{name}", class: "craft-need",
                                span { "{name}: " }
                                span { class: if *lack { "lack-text" } else { "" }, "{have}/{need}" }
                            }
                        }
                    }
                    div { class: "smith-fire-cost",
                        "{i18n::tf(\"smith.fire_cost\", &[&fire_need.to_string()])}"
                    }
                }
                div { class: "craft-panel-icon",
                    ItemIcon { item: (*result).clone(), class: "craft-icon".to_string() }
                }
            }
            div { class: "craft-panel-buttons",
                if in_queue {
                    button {
                        class: "craft-btn",
                        disabled: !can_up,
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                let _ = city::move_smith_recipe(&mut guard, index, &id_up, -1);
                            }
                            refresh_view(state);
                        },
                        "↑"
                    }
                    button {
                        class: "craft-btn",
                        disabled: !can_down,
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                let _ = city::move_smith_recipe(&mut guard, index, &id_down, 1);
                            }
                            refresh_view(state);
                        },
                        "↓"
                    }
                    button {
                        class: "craft-btn",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::unassign_smith_recipe(&mut guard, index, &id_remove);
                            }
                            refresh_view(state);
                            on_close.call(());
                        },
                        "{i18n::t(\"smith.remove_from_craft\")}"
                    }
                } else {
                    button {
                        class: if full || locked { "craft-btn disabled" } else { "craft-btn" },
                        disabled: full || locked,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::assign_smith_recipe(&mut guard, index, &id_add)
                            };
                            match outcome {
                                Ok(()) => {
                                    message.set(String::new());
                                    on_close.call(());
                                }
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{add_label}"
                    }
                }
                button {
                    class: "craft-btn ghost",
                    onclick: move |_| on_close.call(()),
                    "{i18n::t(\"btn.close\")}"
                }
            }
        }
    }
}

/// Строка одной линии улучшений.
#[component]
fn SmithUpgradeRow(
    state: Signal<GameState>,
    view: GameState,
    index: usize,
    line: SmithLine,
    message: Signal<String>,
) -> Element {
    let mut state = state;
    let mut message = message;
    let smith = &view.city.smiths[index];
    let level = line.level(smith);
    let max = line.max_level();
    let cost = line.cost(level);
    let can_buy = actions::can_afford(&view, cost);
    let level_text = i18n::tf("smith.level", &[&level.to_string(), &max.to_string()]);
    let effect = match line {
        SmithLine::Speed => {
            // Показываем десятые доли секунды: иначе 2.8 с и ровно 2.0 с
            // округляются до «2 с», и последнее улучшение кажется пустым.
            let interval = balance::smith_interval(level);
            let text = if interval < 10.0 {
                let value = format!("{interval:.1}");
                let value = value.trim_end_matches(".0").to_string();
                i18n::tf("time.seconds", &[&value])
            } else {
                format_duration(interval)
            };
            i18n::tf("smith.interval_line", &[&text])
        }
        SmithLine::Forge => i18n::tf(
            "smith.forge_line",
            &[
                &smith.fire.to_string(),
                &balance::smith_fire_capacity(level).to_string(),
                &balance::smith_fire_per_coal(level).to_string(),
            ],
        ),
        SmithLine::Slots => i18n::tf(
            "smith.slots_line",
            &[
                &smith.recipes.len().to_string(),
                &balance::smith_slots(level).to_string(),
            ],
        ),
    };

    rsx! {
        div { class: "smith-upgrade",
            div { class: "smith-upgrade-main",
                span { class: "worker-name", "{line.title()}" }
                span { class: "worker-detail", "{level_text}" }
                span { class: "worker-detail", "{effect}" }
            }
            if level >= max {
                span { class: "smith-max", "{i18n::t(\"smith.max\")}" }
            } else {
                div { class: "smith-upgrade-buy",
                    CostLabel { view: view.clone(), cost }
                    button {
                        class: "city-btn small",
                        disabled: !can_buy,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::upgrade_smith(&mut guard, index, line)
                            };
                            match outcome {
                                Ok(()) => message.set(String::new()),
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"smith.upgrade\")}"
                    }
                }
            }
        }
    }
}
