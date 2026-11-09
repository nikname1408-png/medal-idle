//! Экран походов: локации, длительный поход и найденное.

use std::time::Duration;

use dioxus::prelude::*;

use crate::game::balance;
use crate::game::expedition::{self, ExpeditionError, ExpeditionLocation};
use crate::game::state::GameState;

use super::app::{format_num, refresh_view};
use super::inventory::ingredient_name;

/// Полный экран походов.
#[component]
pub fn ExpeditionScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut message = use_signal(String::new);
    let mut good = use_signal(|| false);
    // Локация, в которой игрок сейчас: `None` — поход не идёт.
    let mut active = use_signal(|| None::<&'static str>);
    let mut found = use_signal(String::new);
    let mut elapsed = use_signal(|| 0u32);
    let mut stopping = use_signal(|| false);
    let energy = view.energy;

    // Тик похода: раз в секунду, пока поход не остановлен и игра загружена.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(balance::TICK_MS)).await;
            let Some(location_id) = *active.peek() else {
                continue;
            };
            // Стоит ли ждать остальное: пока идёт остановка, тиков не делаем.
            if *stopping.peek() {
                active.set(None);
                stopping.set(false);
                continue;
            }

            let tick = {
                let mut guard = state.write();
                expedition::step_expedition(&mut guard, location_id, 1.0)
            };
            if !tick.loot.is_empty() {
                let line = tick
                    .loot
                    .iter()
                    .map(|loot| format!("+{} × {}", loot.quantity, ingredient_name(&loot.item_id)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let previous = found.peek().clone();
                found.set(join_lines(&previous, &line));
                good.set(true);
            }
            if tick.overflow {
                message.set("Склад полон — часть добычи сгорела.".to_string());
            }
            if tick.interrupted {
                message.set("Поход прерван: рюкзак полон.".to_string());
                good.set(false);
            }
            if tick.out_of_energy {
                message.set("Поход прерван: энергия закончилась.".to_string());
                good.set(false);
            }
            if tick.is_over() {
                active.set(None);
            }
            elapsed += 1;
            refresh_view(state);
        }
    });

    let message_text = message();
    let good_now = good();
    let found_text = found();
    let active_now = active();
    let elapsed_now = elapsed();

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
                    "Поход идёт, пока есть энергия ({format_num(balance::EXPEDITION_ENERGY_PER_TICK)}/с). Находки появляются по дороге; нажми «Закончить поход», чтобы вернуться."
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

            if let Some(location_id) = active_now {
                div { class: "exp-live",
                    div { class: "exp-live-head",
                        span { class: "exp-live-name",
                            "В пути: {expedition::location(location_id).map(|l| l.name).unwrap_or(\"\")}"
                        }
                        span { class: "exp-live-time", "В пути {format_num(elapsed_now as f64)} с" }
                    }
                    button {
                        class: "exp-stop",
                        onclick: move |_| stopping.set(true),
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
                        busy: active_now.is_some(),
                        current: active_now == Some(location.id),
                        on_go: move |_| {
                            message.set(String::new());
                            found.set(String::new());
                            good.set(false);
                            elapsed.set(0);
                            active.set(Some(location.id));
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

/// Добавляет строку к списку находок, склеивая одинаковые пункты.
fn join_lines(previous: &str, line: &str) -> String {
    if previous.is_empty() {
        line.to_string()
    } else {
        format!("{previous}\n{line}")
    }
}

/// Текст ошибки похода (для будущих вызовов).
#[allow(dead_code)]
fn error_text(error: ExpeditionError) -> &'static str {
    error.text()
}
