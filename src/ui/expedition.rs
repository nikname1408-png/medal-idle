//! Экран походов: локации, длительный поход и найденное.

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
                    "Поход идёт, пока есть энергия ({format_num(balance::EXPEDITION_ENERGY_PER_TICK)}/с) — даже если закрыть эту вкладку. Нажми «Закончить поход», чтобы вернуться."
                }
                div { class: "exp-stats",
                    span { class: "exp-stat",
                        "Рюкзак: {view.inventory_used_slots()}/{view.inventory_slots()}"
                    }
                    span { class: "exp-stat", "Шанс указан за 10 с пути." }
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
    busy: bool,
    current: bool,
    on_go: EventHandler<()>,
) -> Element {
    let can_go = !busy && energy + 1e-9 >= balance::EXPEDITION_ENERGY_PER_TICK;
    let active = drop_list(location.active_drops);
    let inventory = drop_list(location.inventory_drops);
    let button_label = if current {
        "Идёт поход"
    } else if busy {
        "Занят"
    } else {
        "В поход"
    };

    rsx! {
        div { class: if current { "exp-card current" } else { "exp-card" },
            div { class: "exp-card-head",
                span { class: "exp-card-name", "{location.name}" }
                span { class: "exp-card-chance", "{location.chance_percent}% / 10 с" }
            }
            div { class: "exp-card-desc", "{location.description}" }
            div { class: "exp-card-drops", "Склад: {active}" }
            div { class: "exp-card-drops", "Рюкзак: {inventory}" }
            div { class: "exp-card-foot",
                span { class: "exp-card-cost",
                    "−{format_num(balance::EXPEDITION_ENERGY_PER_TICK)}/с"
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

/// Строка с дропами локации.
fn drop_list(drops: &[(&str, u32, u32)]) -> String {
    drops
        .iter()
        .map(|(id, min, max)| format!("{} {min}–{max}", ingredient_name(id)))
        .collect::<Vec<_>>()
        .join(", ")
}
