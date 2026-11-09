//! Экран походов: локации по цепочке, редкости лута и найденное.

use dioxus::prelude::*;

use crate::game::balance;
use crate::game::expedition::{self, ExpeditionLocation};
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
                .map(|(item_id, quantity, _)| format!("+{quantity} × {}", ingredient_name(item_id)))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let current_id = view.current_location();

    rsx! {
        section { class: "exp-screen",
            div { class: "exp-head",
                div { class: "exp-title-row",
                    h2 { class: "exp-title", "Походы" }
                    span { class: "exp-energy",
                        "Энергия: {format_num(energy)} / {format_num(view.max_energy())}"
                    }
                }
                p { class: "exp-hint",
                    "Каждую секунду пути есть {balance::EXPEDITION_LOOT_CHANCE}% шанс находки — поход идёт даже с закрытой вкладкой. Карты новых локаций падают только в последней открытой."
                }
                div { class: "exp-stats",
                    span { class: "exp-stat",
                        "Рюкзак: {view.inventory_used_slots()}/{view.inventory_slots()}"
                    }
                }
                if view.inventory_free_slots() == 0 {
                    div { class: "exp-warning",
                        "Рюкзак полон: инвентарная находка прервёт поход."
                    }
                }
            }

            if let Some(location_id) = active_id.clone() {
                div { class: "exp-live",
                    div { class: "exp-live-head",
                        span { class: "exp-live-name",
                            "В пути: {expedition::location(&location_id).map(|l| l.name).unwrap_or(\"\")}"
                        }
                        span { class: "exp-live-time", "В пути {format_num(elapsed as f64)} с" }
                    }
                    button {
                        class: "exp-stop",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                if let Some(active) = guard.active_expedition.as_mut() {
                                    if active.notice.is_none() {
                                        active.notice = Some("Поход окончен.".to_string());
                                    }
                                    active.done = true;
                                }
                            }
                            refresh_view(state);
                        },
                        "Закончить поход"
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
                    div { class: "exp-found-title", "Найдено в этом походе" }
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
        "Закрыто"
    } else if !meets_requirement {
        "Нужен предмет"
    } else if current {
        "Идёт поход"
    } else if busy {
        "Занят"
    } else {
        "В поход"
    };
    let unlock_text = expedition::unlock_map(location.id)
        .map(|map_id| format!("Карта: {} (дроп или торговец)", ingredient_name(map_id)))
        .unwrap_or_default();

    rsx! {
        div {
            class: if !unlocked { "exp-card locked" } else if current { "exp-card current" } else { "exp-card" },
            div { class: "exp-card-head",
                span { class: "exp-card-name", "{location.name}" }
                if is_current && unlocked {
                    span { class: "exp-card-chance", "текущая" }
                }
            }
            div { class: "exp-card-desc", "{location.description}" }
            if !unlocked {
                div { class: "exp-card-desc", "{unlock_text}" }
            }
            if let Some((item_id, label)) = location.requires_item {
                div {
                    class: if meets_requirement { "exp-card-desc" } else { "exp-card-desc poor" },
                    "Требуется: {label} ({ingredient_name(item_id)})"
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
                div { class: "exp-card-desc",
                    "Особый лут: Чертёж паровой машины — {balance::EXPEDITION_STEAM_BLUEPRINT_CHANCE}% в секунду"
                }
            }
            div { class: "exp-card-foot",
                span { class: "exp-card-cost",
                    "−{format_num(location.energy_per_second)} энергии/с"
                }
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
