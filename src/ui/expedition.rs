//! Экран походов: локации, кнопка похода и результат.

use dioxus::prelude::*;

use crate::game::balance;
use crate::game::expedition::{self, ExpeditionError, ExpeditionLocation, ExpeditionReport};
use crate::game::state::GameState;

use super::app::{format_num, refresh_view};
use super::inventory::ingredient_name;

/// Полный экран походов.
#[component]
pub fn ExpeditionScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut message = use_signal(String::new);
    let mut good = use_signal(|| false);
    let energy = view.energy;
    let message_text = message();
    let good_now = good();
    let cost = balance::EXPEDITION_ENERGY_COST;

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
                    "Один поход — {balance::EXPEDITION_TICKS} с пути и {format_num(cost)} энергии. Каждую секунду есть шанс найти крупную партию. Уголь в походах не попадается."
                }
                div { class: "exp-stats",
                    span { class: "exp-stat",
                        "Рюкзак: {view.inventory_used_slots()}/{view.inventory_slots()}"
                    }
                    span { class: "exp-stat", "Шанс указан за секунду пути." }
                }
                if view.inventory_free_slots() == 0 {
                    div { class: "exp-warning",
                        "Рюкзак полон: инвентарная находка прервёт поход."
                    }
                }
            }
            if !message_text.is_empty() {
                div {
                    class: if good_now { "exp-message" } else { "exp-message poor" },
                    "{message_text}"
                }
            }
            div { class: "exp-list",
                for location in expedition::LOCATIONS {
                    ExpeditionCard {
                        key: "{location.id}",
                        location,
                        energy,
                        on_go: move |_| {
                            let result = {
                                let mut guard = state.write();
                                expedition::run_expedition(&mut guard, location.id)
                            };
                            match result {
                                Ok(report) => {
                                    good.set(!report.loot.is_empty() && !report.interrupted);
                                    message.set(report_text(&report));
                                }
                                Err(error) => {
                                    good.set(false);
                                    message.set(error_text(error).to_string());
                                }
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
fn ExpeditionCard(location: ExpeditionLocation, energy: f64, on_go: EventHandler<()>) -> Element {
    let cost = balance::EXPEDITION_ENERGY_COST;
    let can_go = energy + 1e-9 >= cost;
    let active = drop_list(location.active_drops);
    let inventory = drop_list(location.inventory_drops);

    rsx! {
        div { class: "exp-card",
            div { class: "exp-card-head",
                span { class: "exp-card-name", "{location.name}" }
                span { class: "exp-card-chance", "{location.chance_percent}% / с" }
            }
            div { class: "exp-card-desc", "{location.description}" }
            div { class: "exp-card-drops", "Склад: {active}" }
            div { class: "exp-card-drops", "Рюкзак: {inventory}" }
            div { class: "exp-card-foot",
                span { class: "exp-card-cost", "Энергия: {format_num(cost)}" }
                button {
                    class: if can_go { "exp-go" } else { "exp-go disabled" },
                    disabled: !can_go,
                    onclick: move |_| on_go.call(()),
                    "В поход"
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

/// Текст результата похода.
fn report_text(report: &ExpeditionReport) -> String {
    let found = report
        .loot
        .iter()
        .map(|loot| format!("+{} × {}", loot.quantity, ingredient_name(&loot.item_id)))
        .collect::<Vec<_>>()
        .join(", ");

    let mut text = if report.interrupted {
        if found.is_empty() {
            "Поход прерван: в рюкзаке нет места.".to_string()
        } else {
            format!("Найдено: {found}. Поход прерван: рюкзак полон.")
        }
    } else if found.is_empty() {
        if report.overflow {
            "Склад полон — вся добыча сгорела.".to_string()
        } else {
            "Ничего не найдено. Попробуй ещё раз.".to_string()
        }
    } else {
        format!("Найдено: {found}.")
    };

    if report.overflow && !found.is_empty() {
        text.push_str(" Склад полон — часть добычи сгорела.");
    }
    text
}

fn error_text(error: ExpeditionError) -> &'static str {
    error.text()
}
