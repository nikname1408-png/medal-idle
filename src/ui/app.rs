use dioxus::prelude::*;

use crate::game::actions::{self, GatherResult};
use crate::game::state::ResourceKind;
use crate::game::{balance, GameState};

const MAIN_CSS: Asset = asset!("/assets/main.css");

const TABS: [(&str, &str); 8] = [
    ("🌾", "Поле"),
    ("📜", "Изучения"),
    ("👷", "Труд"),
    ("🎒", "Инвентарь"),
    ("🧭", "Экспедиции"),
    ("🔄", "Перерождение"),
    ("📊", "Статистика"),
    ("⚙️", "Настройки"),
];

#[component]
pub fn App() -> Element {
    let mut state = use_signal(GameState::default);
    let active_tab = use_signal(|| 0usize);
    let mut toast = use_signal(String::new);

    // Игровой цикл: раз в секунду начисляем энергию.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(balance::TICK_MS)).await;
            let mut guard = state.write();
            actions::tick(&mut guard, 1.0);
        }
    });

    let snapshot = state();

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app",
            TopBar {}
            main { class: "content",
                CollapsibleSection { title: "Отдых".to_string(),
                    ActionRow {
                        label: "Отдых".to_string(),
                        info: "+10 энергии за клик".to_string(),
                        on_press: move |_| {
                            let gained = {
                                let mut guard = state.write();
                                actions::rest(&mut guard)
                            };
                            toast.set(rest_message(gained));
                        },
                    }
                }
                CollapsibleSection { title: "Сбор".to_string(),
                    ActionRow {
                        label: "Собрать камень".to_string(),
                        info: "+1 камень за клик (−1 энергия)".to_string(),
                        on_press: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::gather(&mut guard, ResourceKind::Stone)
                            };
                            toast.set(gather_message(ResourceKind::Stone, result));
                        },
                    }
                    ActionRow {
                        label: "Собрать дерево".to_string(),
                        info: "+1 дерево за клик (−1 энергия)".to_string(),
                        on_press: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::gather(&mut guard, ResourceKind::Wood)
                            };
                            toast.set(gather_message(ResourceKind::Wood, result));
                        },
                    }
                    ActionRow {
                        label: "Собрать железо".to_string(),
                        info: "+1 железо за клик (−1 энергия)".to_string(),
                        on_press: move |_| {
                            let result = {
                                let mut guard = state.write();
                                actions::gather(&mut guard, ResourceKind::Iron)
                            };
                            toast.set(gather_message(ResourceKind::Iron, result));
                        },
                    }
                }
                CollapsibleSection { title: "Продукция".to_string(),
                    ActionRow {
                        label: "Лесопилка".to_string(),
                        info: "Работяги добывают дерево автоматически".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                    ActionRow {
                        label: "Угольная яма".to_string(),
                        info: "Переработка дерева в уголь".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                    ActionRow {
                        label: "Кузница".to_string(),
                        info: "Переработка руды в железо и ковка мечей".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                }
                CollapsibleSection { title: "Хранение".to_string(),
                    ActionRow {
                        label: "Склад".to_string(),
                        info: "Увеличивает лимит ресурсов".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                    ActionRow {
                        label: "Амбар".to_string(),
                        info: "Откроется позже".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                    ActionRow {
                        label: "Банк".to_string(),
                        info: "Увеличивает лимит золота".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                }
                CollapsibleSection { title: "Дома".to_string(),
                    ActionRow {
                        label: "Дом для работяг".to_string(),
                        info: "+1 работник за уровень".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                    ActionRow {
                        label: "Исследовательская лаборатория".to_string(),
                        info: "Исследования зданий, чертежей и предметов".to_string(),
                        on_press: move |_| toast.set("Откроется позже".to_string()),
                    }
                }
            }
            if !toast().is_empty() {
                div { class: "toast", "{toast}" }
            }
            div { class: "bottom",
                ResourceBar { state: snapshot.clone() }
                TabBar { active: active_tab }
                EnergyBar { state: snapshot }
            }
        }
    }
}

#[component]
fn TopBar() -> Element {
    rsx! {
        header { class: "topbar",
            button { class: "icon-btn", "☰" }
            h1 { class: "topbar-title", "Поле" }
            button { class: "icon-btn", "🔔" }
        }
    }
}

#[component]
fn CollapsibleSection(title: String, children: Element) -> Element {
    let mut open = use_signal(|| true);
    let is_open = open();
    let chev_class = if is_open { "chev open" } else { "chev" };

    rsx! {
        section { class: "section",
            button {
                class: "section-head",
                onclick: move |_| open.set(!open()),
                span { class: "section-title", "{title}" }
                span { class: chev_class, "▾" }
            }
            if is_open {
                div { class: "section-body", {children} }
            }
        }
    }
}

#[component]
fn ActionRow(label: String, info: String, on_press: EventHandler<()>) -> Element {
    let mut open = use_signal(|| false);
    let is_open = open();
    let arrow = if is_open { "▴" } else { "▾" };

    rsx! {
        div { class: "action",
            div { class: "action-row",
                button {
                    class: "action-main",
                    onclick: move |_| on_press.call(()),
                    span { class: "action-label", "{label}" }
                }
                button {
                    class: "chev-btn",
                    onclick: move |_| open.set(!open()),
                    span { class: "chev", "{arrow}" }
                }
            }
            if is_open {
                div { class: "action-info", "{info}" }
            }
        }
    }
}

#[component]
fn ResourceBar(state: GameState) -> Element {
    let items = [
        ("🌲", "Дерево", state.wood, state.wood_cap),
        ("🪨", "Камень", state.stone, state.stone_cap),
        ("⛏️", "Железо", state.iron, state.iron_cap),
        ("🪙", "Уголь", state.coal, state.coal_cap),
        ("💰", "Золото", state.gold, state.gold_cap),
    ];

    rsx! {
        div { class: "resources",
            for (icon, name, value, cap) in items {
                div { class: "resource", key: "{name}",
                    span { class: "resource-icon", "{icon}" }
                    span {
                        class: if value >= cap { "resource-value full" } else { "resource-value" },
                        "{format_num(value)}"
                    }
                }
            }
        }
    }
}

#[component]
fn TabBar(active: Signal<usize>) -> Element {
    rsx! {
        nav { class: "tabs",
            for (i, (icon, label)) in TABS.iter().enumerate() {
                button {
                    key: "{i}",
                    class: if active() == i { "tab active" } else { "tab" },
                    onclick: move |_| active.set(i),
                    span { class: "tab-icon", "{icon}" }
                    span { class: "tab-label", "{label}" }
                }
            }
        }
    }
}

#[component]
fn EnergyBar(state: GameState) -> Element {
    let fill_style = format!("width: {:.1}%", state.energy_ratio() * 100.0);
    let label = format!(
        "{} / {} (+{:.2}/с)",
        format_num(state.energy),
        format_num(state.energy_max),
        state.energy_regen
    );

    rsx! {
        div { class: "energy",
            div { class: "energy-fill", style: "{fill_style}" }
            div { class: "energy-label", "{label}" }
        }
    }
}

fn gather_message(kind: ResourceKind, result: GatherResult) -> String {
    match result {
        GatherResult::Gathered(value) if value > 0.0 => {
            format!("+{} {}", format_num(value), kind.name())
        }
        GatherResult::Gathered(_) => "Склад полон".to_string(),
        GatherResult::NoEnergy => "Не хватает энергии".to_string(),
        GatherResult::StorageFull => format!("Склад полон: {}", kind.name()),
    }
}

fn rest_message(gained: f64) -> String {
    if gained > 0.0 {
        format!("+{} энергии", format_num(gained))
    } else {
        "Энергия уже полная".to_string()
    }
}

fn format_num(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
