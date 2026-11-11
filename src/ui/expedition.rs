//! Экран походов: локации по цепочке, редкости лута и найденное.

use dioxus::prelude::*;

use crate::game::balance;
use crate::game::expedition::{self, ExpeditionLocation};
use crate::game::i18n;
use crate::game::state::{ActiveExpedition, GameState};

use super::app::{format_num, refresh_view};
use super::inventory::ingredient_name;

/// Полный экран походов.
#[component]
pub fn ExpeditionScreen(state: Signal<GameState>, view: GameState) -> Element {
    let energy = view.energy;
    // Поход живёт в состоянии игры и идёт в общем тике, поэтому экран только
    // показывает его и меняет по кнопкам.
    let active = view.active_expedition.clone();
    let active_id = active
        .as_ref()
        .filter(|expedition| !expedition.done)
        .map(|expedition| expedition.location_id.clone());
    let elapsed = active
        .as_ref()
        .map(|expedition| expedition.elapsed)
        .unwrap_or(0);
    let message_text = active
        .as_ref()
        .and_then(|expedition| expedition.notice.clone())
        .unwrap_or_default();
    let finished = active
        .as_ref()
        .map(|expedition| expedition.done)
        .unwrap_or(false);
    let good_now = !finished && !message_text.is_empty();
    let found_text = active
        .as_ref()
        .map(|expedition| {
            expedition
                .found
                .iter()
                .map(|(item_id, quantity, _)| {
                    i18n::tf(
                        "hike.found_line",
                        &[&quantity.to_string(), ingredient_name(item_id)],
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let current_id = view.current_location();
    let energy_text = i18n::tf(
        "hike.energy",
        &[&format_num(energy), &format_num(view.max_energy())],
    );
    let hint_text = i18n::tf("hike.hint", &[&balance::EXPEDITION_LOOT_CHANCE.to_string()]);
    let bag_text = i18n::tf(
        "hike.bag",
        &[
            &view.inventory_used_slots().to_string(),
            &view.inventory_slots().to_string(),
        ],
    );

    rsx! {
        section { class: "exp-screen",
            div { class: "exp-head",
                div { class: "exp-title-row",
                    h2 { class: "exp-title", "{i18n::t(\"sec.hike\")}" }
                    span { class: "exp-energy", "{energy_text}" }
                }
                p { class: "exp-hint", "{hint_text}" }
                div { class: "exp-stats",
                    span { class: "exp-stat", "{bag_text}" }
                }
                if view.inventory_free_slots() == 0 {
                    div { class: "exp-warning", "{i18n::t(\"hike.bag_full\")}" }
                }
            }

            if let Some(location_id) = active_id.clone() {
                div { class: "exp-live",
                    div { class: "exp-live-head",
                        span { class: "exp-live-name",
                            "{i18n::tf(\"hike.on_the_way\", &[expedition::location(&location_id).map(|l| l.name()).unwrap_or(\"\")])}"
                        }
                        span { class: "exp-live-time",
                            "{i18n::tf(\"hike.elapsed\", &[&format_num(elapsed as f64)])}"
                        }
                    }
                    button {
                        class: "exp-stop",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                if let Some(active) = guard.active_expedition.as_mut() {
                                    if active.notice.is_none() {
                                        active.notice =
                                            Some(i18n::t("hike.finished").to_string());
                                    }
                                    active.done = true;
                                }
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"btn.finish_hike\")}"
                    }
                }
            }

            if !message_text.is_empty() {
                div {
                    class: if good_now { "exp-message" } else { "exp-message poor" },
                    "{message_text}"
                }
            }

            if !found_text.is_empty() {
                div { class: "exp-found",
                    div { class: "exp-found-title", "{i18n::t(\"hike.found\")}" }
                    div { class: "exp-found-list", "{found_text}" }
                }
            }

            div { class: "exp-list",
                for location in expedition::LOCATIONS {
                    ExpeditionCard {
                        key: "{location.id}",
                        location,
                        energy,
                        unlocked: view.location_unlocked(location.id),
                        meets_requirement: view.meets_location_requirement(location.requires_item),
                        is_current: current_id == location.id,
                        busy: active_id.is_some(),
                        current: active_id.as_deref() == Some(location.id),
                        on_go: move |_| {
                            {
                                let mut guard = state.write();
                                guard.expedition_timer = 0.0;
                                guard.expedition_candle_timer = 0;
                                guard.active_expedition = Some(ActiveExpedition::new(location.id));
                            }
                            refresh_view(state);
                        },
                    }
                }
            }
        }
    }
}

/// Карточка одной локации.
#[component]
fn ExpeditionCard(
    location: ExpeditionLocation,
    energy: f64,
    unlocked: bool,
    meets_requirement: bool,
    is_current: bool,
    busy: bool,
    current: bool,
    on_go: EventHandler<()>,
) -> Element {
    let can_go =
        unlocked && meets_requirement && !busy && energy + 1e-9 >= location.energy_per_second;
    let button_label = if !unlocked {
        i18n::t("hike.locked")
    } else if !meets_requirement {
        i18n::t("hike.needs_item")
    } else if current {
        i18n::t("hike.running")
    } else if busy {
        i18n::t("hike.busy")
    } else {
        i18n::t("hike.go")
    };
    let unlock_text = expedition::unlock_map(location.id)
        .map(|map_id| i18n::tf("hike.map_line", &[ingredient_name(map_id)]))
        .unwrap_or_default();
    let requirement_text = match location.requires_item {
        None => String::new(),
        Some("candle") => i18n::tf(
            "hike.need_candle",
            &[
                i18n::t("req.candle"),
                &balance::CANDLE_SECONDS_PER_ITEM.to_string(),
            ],
        ),
        Some(item_id) => i18n::tf(
            "hike.need_item",
            &[i18n::t("req.torch_or_candle"), ingredient_name(item_id)],
        ),
    };
    let special_text = i18n::tf(
        "hike.special_loot",
        &[&format_num(balance::EXPEDITION_STEAM_BLUEPRINT_ROLL as f64)],
    );
    let cost_text = i18n::tf("hike.cost", &[&format_num(location.energy_per_second)]);

    rsx! {
        div {
            class: if !unlocked { "exp-card locked" } else if current { "exp-card current" } else { "exp-card" },
            div { class: "exp-card-head",
                span { class: "exp-card-name", "{location.name()}" }
                if is_current && unlocked {
                    span { class: "exp-card-chance", "{i18n::t(\"hike.current\")}" }
                }
            }
            div { class: "exp-card-desc", "{location.description()}" }
            if !unlocked {
                div { class: "exp-card-desc", "{unlock_text}" }
            }
            if location.requires_item.is_some() {
                div {
                    class: if meets_requirement { "exp-card-desc" } else { "exp-card-desc poor" },
                    "{requirement_text}"
                }
            }
            div { class: "exp-card-loot",
                for entry in location.loot.iter() {
                    div { class: "exp-loot-row", key: "{entry.item_id}",
                        span { class: "exp-loot-rarity", style: "color: {entry.rarity.color()}",
                            "{entry.rarity.name()}"
                        }
                        span { class: "exp-loot-name", "{ingredient_name(entry.item_id)}" }
                        span { class: "exp-loot-qty", "{entry.min_quantity}–{entry.max_quantity}" }
                    }
                }
            }
            if location.id == "ancient_ruins" {
                div { class: "exp-card-desc", "{special_text}" }
            }
            div { class: "exp-card-foot",
                span { class: "exp-card-cost", "{cost_text}" }
                button {
                    class: if can_go { "exp-go" } else { "exp-go disabled" },
                    disabled: !can_go,
                    onclick: move |_| on_go.call(()),
                    "{button_label}"
                }
            }
        }
    }
}
