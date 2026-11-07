use std::time::Duration;

use dioxus::prelude::*;

use crate::game::actions::{self, BuildResult, GatherResult};
use crate::game::state::{ResourceKind, Theme};
use crate::game::{balance, GameState};

use super::save;

const MAIN_CSS: Asset = asset!("/assets/main.css");
const FIELD_ICON: Asset = asset!("/assets/icons/field.png");
const WOOD_ICON: Asset = asset!("/assets/icons/wood.png");
const COAL_ICON: Asset = asset!("/assets/icons/coal.png");

const TAB_FIELD: usize = 0;
const TAB_STUDIES: usize = 1;
const TAB_WORK: usize = 2;
const TAB_SETTINGS: usize = 7;

const TABS: [(&str, &str); 8] = [
    ("🌾", "Поле"),
    ("📜", "Изуч."),
    ("👷", "Труд"),
    ("🎒", "Сумка"),
    ("🧭", "Походы"),
    ("🔄", "Цикл"),
    ("📊", "Статы"),
    ("⚙️", "Настр."),
];

#[component]
pub fn App() -> Element {
    let mut state = use_signal(GameState::default);
    let active_tab = use_signal(|| TAB_FIELD);
    let toast = use_signal(String::new);
    let toast_seq = use_signal(|| 0u64);
    let mut loaded = use_signal(|| false);
    let mut dev_open = use_signal(|| false);

    // Загружаем сохранение один раз при старте.
    use_future(move || async move {
        match save::load().await {
            Ok(Some(mut saved)) => {
                saved.refresh_unlocks();
                state.set(saved);
            }
            Ok(None) => {}
            Err(error) => eprintln!("Не удалось загрузить сохранение: {error}"),
        }
        loaded.set(true);
    });

    // Игровой цикл: энергия восстанавливается только от дома.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(balance::TICK_MS)).await;
            if !*loaded.peek() {
                continue;
            }
            let mut guard = state.write();
            actions::tick(&mut guard, 1.0);
        }
    });

    // Автосохранение каждые две секунды.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !*loaded.peek() {
                continue;
            }
            let snapshot = state();
            if let Err(error) = save::save(&snapshot).await {
                eprintln!("Не удалось сохранить игру: {error}");
            }
        }
    });

    let snapshot = state();
    let theme_class = theme_css_class(snapshot.theme);
    let title = match active_tab() {
        TAB_SETTINGS => "Настройки",
        TAB_STUDIES => "Изучения",
        TAB_WORK => "Труд",
        _ => "Поле",
    };

    let screen = match active_tab() {
        TAB_FIELD => rsx! {
            FieldScreen { state, toast, toast_seq }
        },
        TAB_STUDIES => rsx! {
            StudiesScreen { state }
        },
        TAB_WORK => rsx! {
            WorkScreen { state }
        },
        TAB_SETTINGS => rsx! {
            SettingsScreen { state, toast, toast_seq }
        },
        index => rsx! {
            PlaceholderScreen { title: TABS[index].1.to_string() }
        },
    };

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app {theme_class}",
            TopBar {
                title: title.to_string(),
                on_dev: move |_| dev_open.set(!dev_open()),
            }
            main { class: "content", {screen} }
            if !toast().is_empty() {
                div { class: "toast", "{toast}" }
            }
            div { class: "bottom",
                ResourceBar { state: snapshot.clone() }
                TabBar { active: active_tab }
                EnergyBar { state: snapshot }
            }
            if dev_open() {
                DevPanel { state, on_close: move |_| dev_open.set(false) }
            }
        }
    }
}

#[component]
fn FieldScreen(
    state: Signal<GameState>,
    toast: Signal<String>,
    toast_seq: Signal<u64>,
) -> Element {
    let snapshot = state();

    let house_cost = balance::house_cost(snapshot.house_level);
    let storage_cost = balance::storage_cost(snapshot.storage_level);
    let worker_house_cost = balance::worker_house_cost(snapshot.worker_house_level);
    let research_house_cost = balance::research_house_cost(snapshot.research_house_level);
    let sawmill_cost = balance::sawmill_cost(snapshot.sawmill_level);
    let coal_pit_cost = balance::coal_pit_cost(snapshot.coal_pit_level);

    let affordable = |cost: balance::Cost| snapshot.wood >= cost.wood && snapshot.stone >= cost.stone;
    let house_affordable = affordable(house_cost);
    let storage_affordable = affordable(storage_cost);
    let worker_house_affordable = affordable(worker_house_cost);
    let research_house_affordable = affordable(research_house_cost);
    let sawmill_affordable = affordable(sawmill_cost);
    let coal_pit_affordable = affordable(coal_pit_cost);

    let gather_yield = balance::CLICK_YIELD + snapshot.dev.gather_yield;
    let rest_available = snapshot.energy < snapshot.energy_max;
    let wood_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && snapshot.wood < snapshot.cap(ResourceKind::Wood);
    let stone_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && snapshot.stone < snapshot.cap(ResourceKind::Stone);

    let house_description = if snapshot.house_level == 0 {
        "После постройки: +2 энергии за отдых и +1 энергии в секунду.".to_string()
    } else {
        format!(
            "Сейчас: +{} за отдых, +{}/с. Улучшение добавит ещё +1 к отдыху и +1/с.",
            format_num(snapshot.rest_energy()),
            format_num(snapshot.energy_regen())
        )
    };

    let rest_info = format!(
        "+{} энергии за клик. Пассивно: +{:.2}/с.",
        format_num(snapshot.rest_energy()),
        snapshot.energy_regen()
    );

    let storage_description = format!(
        "Сейчас лимит ресурсов: {}. Улучшение добавит +{}.",
        format_num(snapshot.cap(ResourceKind::Wood)),
        format_num(balance::STORAGE_CAP_PER_LEVEL)
    );

    let worker_house_description = if snapshot.worker_house_level == 0 {
        "Даёт 1 работника. Работников можно назначить на лесопилку и угольную яму."
            .to_string()
    } else {
        format!(
            "Сейчас работников: {}. Улучшение добавит ещё одного.",
            snapshot.worker_slots()
        )
    };

    let research_house_description = if snapshot.research_house_level == 0 {
        "Даёт 1 исследователя для изучения новых построек.".to_string()
    } else {
        format!(
            "Сейчас исследователей: {}. Улучшение добавит ещё одного.",
            snapshot.researcher_slots()
        )
    };

    let sawmill_description = format!(
        "Работники добывают дерево: +{}/с с каждого. Уровень даёт одно рабочее место.",
        format_num(balance::SAWMILL_WOOD_PER_WORKER_PER_SEC)
    );

    let coal_pit_description = format!(
        "Работники тратят {} дерева и получают {} угля за {} с. Уровень даёт одно рабочее место.",
        format_num(balance::COAL_PIT_WOOD_PER_BATCH),
        format_num(balance::COAL_PIT_COAL_PER_BATCH),
        format_num(balance::COAL_PIT_BATCH_SECONDS)
    );

    rsx! {
        CurrentGoal { state: snapshot.clone() }

        CollapsibleSection { title: "Отдых".to_string(),
            ActionRow {
                label: "Отдых".to_string(),
                info: rest_info,
                available: rest_available,
                on_press: move |_| do_rest(state, toast, toast_seq),
            }
        }

        if snapshot.gathering_unlocked() {
            CollapsibleSection { title: "Сбор".to_string(),
                ActionRow {
                    label: "Собрать дерево".to_string(),
                    info: format!("+{} за клик (−1 энергия).", format_num(gather_yield)),
                    available: wood_available,
                    on_press: move |_| do_gather(state, toast, toast_seq, ResourceKind::Wood),
                }
                if snapshot.stone_unlocked() {
                    ActionRow {
                        label: "Собрать камень".to_string(),
                        info: format!("+{} за клик (−1 энергия).", format_num(gather_yield)),
                        available: stone_available,
                        on_press: move |_| do_gather(state, toast, toast_seq, ResourceKind::Stone),
                    }
                }
            }
        }

        if snapshot.stone_unlocked() {
            CollapsibleSection { title: "Хранение".to_string(),
                BuildRow {
                    title: "Склад".to_string(),
                    level: snapshot.storage_level,
                    description: storage_description,
                    cost: cost_text(storage_cost),
                    affordable: storage_affordable,
                    on_build: move |_| do_build_storage(state, toast, toast_seq),
                }
            }
            CollapsibleSection { title: "Дома".to_string(),
                BuildRow {
                    title: "Свой дом".to_string(),
                    level: snapshot.house_level,
                    description: house_description,
                    cost: cost_text(house_cost),
                    affordable: house_affordable,
                    on_build: move |_| do_build_house(state, toast, toast_seq),
                }
                if snapshot.house_level > 0 {
                    BuildRow {
                        title: "Дом для работников".to_string(),
                        level: snapshot.worker_house_level,
                        description: worker_house_description,
                        cost: cost_text(worker_house_cost),
                        affordable: worker_house_affordable,
                        on_build: move |_| do_build_with(state, toast, toast_seq, "Дом для работников", actions::build_worker_house),
                    }
                    BuildRow {
                        title: "Дом для изучения".to_string(),
                        level: snapshot.research_house_level,
                        description: research_house_description,
                        cost: cost_text(research_house_cost),
                        affordable: research_house_affordable,
                        on_build: move |_| do_build_with(state, toast, toast_seq, "Дом для изучения", actions::build_research_house),
                    }
                }
            }
        }

        if snapshot.house_level > 0 {
            CollapsibleSection { title: "Постройки".to_string(),
                BuildRow {
                    title: "Лесопилка".to_string(),
                    level: snapshot.sawmill_level,
                    description: sawmill_description,
                    cost: cost_text(sawmill_cost),
                    affordable: sawmill_affordable,
                    on_build: move |_| do_build_with(state, toast, toast_seq, "Лесопилка", actions::build_sawmill),
                }
                if snapshot.coal_pit_researched {
                    BuildRow {
                        title: "Угольная яма".to_string(),
                        level: snapshot.coal_pit_level,
                        description: coal_pit_description,
                        cost: cost_text(coal_pit_cost),
                        affordable: coal_pit_affordable,
                        on_build: move |_| do_build_with(state, toast, toast_seq, "Угольная яма", actions::build_coal_pit),
                    }
                } else {
                    div { class: "locked-note",
                        "Угольную яму можно открыть на вкладке «Изучения»."
                    }
                }
            }
        }
    }
}

#[component]
fn StudiesScreen(state: Signal<GameState>) -> Element {
    let mut state = state;
    let snapshot = state();
    let done = snapshot.coal_pit_researched;
    let slots = snapshot.researcher_slots();
    let assigned = snapshot.coal_pit_researchers;
    let free = snapshot.researchers_free();
    let ratio = snapshot.coal_pit_research_ratio();
    let percent = format!("{:.0}%", ratio * 100.0);
    let fill_style = format!("width: {:.1}%", ratio * 100.0);
    let speed = snapshot.coal_pit_research_speed();
    let can_minus = assigned > 0;
    let can_plus = !done && free > 0;

    rsx! {
        section { class: "goal",
            div { class: "goal-head",
                span { class: "goal-label", "Исследователи" }
                span { class: "goal-count", "{free} свободно из {slots}" }
            }
            div { class: "goal-text",
                if slots == 0 {
                    "Построй дом для изучения на вкладке «Поле»."
                } else if done {
                    "Все доступные изучения завершены."
                } else {
                    "Назначь исследователей на изучение."
                }
            }
        }

        section { class: "section",
            div { class: "section-head static",
                span { class: "section-title", "Изучения" }
            }
            div { class: "section-body",
                div { class: "research",
                    div { class: "research-head",
                        span { class: "research-name", "Угольная яма" }
                        span { class: if done { "research-status done" } else { "research-status" },
                            if done { "изучено" } else { "{percent}" }
                        }
                    }
                    div { class: "research-desc",
                        if done {
                            "Открыта. Назначь работников на угольную яму во вкладке «Труд»."
                        } else {
                            "Открывает угольную яму: работники тратят дерево и добывают уголь."
                        }
                    }
                    div { class: "research-track",
                        div { class: "research-fill", style: "{fill_style}" }
                    }
                    div { class: "research-foot",
                        span { class: "research-speed", "+{speed:.1} опыта/с" }
                        div { class: "stepper",
                            button {
                                class: "step-btn",
                                disabled: !can_minus,
                                onclick: move |_| {
                                    let mut guard = state.write();
                                    actions::adjust_coal_pit_researchers(&mut guard, -1);
                                },
                                "−"
                            }
                            span { class: "step-value", "{assigned}" }
                            button {
                                class: "step-btn",
                                disabled: !can_plus,
                                onclick: move |_| {
                                    let mut guard = state.write();
                                    actions::adjust_coal_pit_researchers(&mut guard, 1);
                                },
                                "+"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn WorkScreen(state: Signal<GameState>) -> Element {
    let mut state = state;
    let snapshot = state();
    let slots = snapshot.worker_slots();
    let free = snapshot.workers_free();
    let sawmill_workers = snapshot.sawmill_workers;
    let sawmill_slots = snapshot.sawmill_slots();
    let sawmill_rate = snapshot.sawmill_wood_per_sec();
    let coal_workers = snapshot.coal_pit_workers;
    let coal_slots = snapshot.coal_pit_slots();
    let coal_rate = snapshot.coal_pit_coal_per_sec();
    let coal_wood = snapshot.coal_pit_wood_per_sec();
    let batch_percent = format!("{:.0}%", snapshot.coal_pit_progress.clamp(0.0, 1.0) * 100.0);
    let batch_fill = format!(
        "width: {:.1}%",
        snapshot.coal_pit_progress.clamp(0.0, 1.0) * 100.0
    );

    rsx! {
        section { class: "goal",
            div { class: "goal-head",
                span { class: "goal-label", "Работники" }
                span { class: "goal-count", "{free} свободно из {slots}" }
            }
            div { class: "goal-text",
                if slots == 0 {
                    "Построй дом для работников на вкладке «Поле»."
                } else {
                    "Назначай работников на лесопилку и угольную яму."
                }
            }
        }

        section { class: "section",
            div { class: "section-head static",
                span { class: "section-title", "Работы" }
            }
            div { class: "section-body",
                WorkerRow {
                    title: "Лесопилка".to_string(),
                    detail: format!("+{}/с дерева", format_num(sawmill_rate)),
                    assigned: sawmill_workers,
                    capacity: sawmill_slots,
                    free,
                    on_minus: move |_| {
                        let mut guard = state.write();
                        actions::adjust_sawmill_workers(&mut guard, -1);
                    },
                    on_plus: move |_| {
                        let mut guard = state.write();
                        actions::adjust_sawmill_workers(&mut guard, 1);
                    },
                }

                if snapshot.coal_pit_researched {
                    WorkerRow {
                        title: "Угольная яма".to_string(),
                        detail: format!("+{}/с угля · −{}/с дерева", format_num(coal_rate), format_num(coal_wood)),
                        assigned: coal_workers,
                        capacity: coal_slots,
                        free,
                        on_minus: move |_| {
                            let mut guard = state.write();
                            actions::adjust_coal_pit_workers(&mut guard, -1);
                        },
                        on_plus: move |_| {
                            let mut guard = state.write();
                            actions::adjust_coal_pit_workers(&mut guard, 1);
                        },
                    }
                    div { class: "batch",
                        div { class: "batch-track",
                            div { class: "batch-fill", style: "{batch_fill}" }
                        }
                        span { class: "batch-label", "партия {batch_percent}" }
                    }
                } else {
                    div { class: "locked-note",
                        "Угольная яма не изучена. Открой её во вкладке «Изучения»."
                    }
                }
            }
        }
    }
}

#[component]
fn WorkerRow(
    title: String,
    detail: String,
    assigned: u32,
    capacity: u32,
    free: u32,
    on_minus: EventHandler<()>,
    on_plus: EventHandler<()>,
) -> Element {
    let can_minus = assigned > 0;
    let can_plus = free > 0 && assigned < capacity;

    rsx! {
        div { class: "worker",
            div { class: "worker-main",
                span { class: "worker-name", "{title}" }
                span { class: "worker-detail", "{detail}" }
            }
            div { class: "stepper",
                button {
                    class: "step-btn",
                    disabled: !can_minus,
                    onclick: move |_| on_minus.call(()),
                    "−"
                }
                span { class: "step-value", "{assigned} / {capacity}" }
                button {
                    class: "step-btn",
                    disabled: !can_plus,
                    onclick: move |_| on_plus.call(()),
                    "+"
                }
            }
        }
    }
}

#[component]
fn SettingsScreen(
    state: Signal<GameState>,
    toast: Signal<String>,
    toast_seq: Signal<u64>,
) -> Element {
    let mut state = state;
    let reset_armed = use_signal(|| false);
    let theme = state().theme;

    rsx! {
        section { class: "settings",
            h2 { class: "settings-title", "Внешний вид" }
            div { class: "theme-list",
                ThemeOption {
                    name: "Классический".to_string(),
                    color: "#004487".to_string(),
                    active: theme == Theme::Classic,
                    on_pick: move |_| {
                        state.write().theme = Theme::Classic;
                    },
                }
                ThemeOption {
                    name: "Тёмный".to_string(),
                    color: "#444444".to_string(),
                    active: theme == Theme::Dark,
                    on_pick: move |_| {
                        state.write().theme = Theme::Dark;
                    },
                }
            }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Прогресс" }
            p { class: "settings-note", "Игра сохраняется автоматически каждые 2 секунды." }
            button {
                class: if reset_armed() { "danger-btn armed" } else { "danger-btn" },
                onclick: move |_| reset_progress(state, toast, toast_seq, reset_armed),
                if reset_armed() {
                    "Нажми ещё раз, чтобы сбросить"
                } else {
                    "Сбросить прогресс"
                }
            }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Об игре" }
            p { class: "settings-note", "«Поле» — версия v0.4. Rust + Dioxus 0.7." }
        }
    }
}

#[component]
fn ThemeOption(name: String, color: String, active: bool, on_pick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: if active { "theme-option active" } else { "theme-option" },
            onclick: move |_| on_pick.call(()),
            span { class: "theme-swatch", style: "background: {color};" }
            span { class: "theme-name", "{name}" }
            if active {
                span { class: "theme-check", "✓" }
            }
        }
    }
}

#[component]
fn PlaceholderScreen(title: String) -> Element {
    rsx! {
        div { class: "placeholder",
            div { class: "placeholder-icon", "🚧" }
            h2 { class: "placeholder-title", "{title}" }
            p { class: "placeholder-text", "Этот раздел появится в следующих версиях." }
        }
    }
}

#[component]
fn TopBar(title: String, on_dev: EventHandler<()>) -> Element {
    rsx! {
        header { class: "topbar",
            button { class: "icon-btn", aria_label: "Меню",
                span { class: "burger" }
            }
            h1 { class: "topbar-title", "{title}" }
            button { class: "icon-btn", aria_label: "Уведомления",
                svg {
                    class: "bell",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "1.8",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M6 9a6 6 0 0 1 12 0c0 4 1.4 5.4 1.9 5.9H4.1C4.6 14.4 6 13 6 9z" }
                    path { d: "M10 18.5a2 2 0 0 0 4 0" }
                }
            }
            button {
                class: "icon-btn dev-icon",
                title: "Панель разработчика (временно)",
                aria_label: "Панель разработчика",
                onclick: move |_| on_dev.call(()),
                "🛠"
            }
        }
    }
}

#[component]
fn DevPanel(state: Signal<GameState>, on_close: EventHandler<()>) -> Element {
    let mut state = state;
    let snapshot = state();
    let dev = snapshot.dev.clone();

    rsx! {
        div { class: "dev-overlay",
            div { class: "dev-panel",
                div { class: "dev-head",
                    h2 { class: "dev-title", "Панель разработчика" }
                    button {
                        class: "dev-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                p { class: "dev-note", "Временная панель для тестов. В релизе её не будет." }

                section { class: "dev-group",
                    h3 { class: "dev-group-title", "Энергия" }
                    DevNumberRow {
                        label: "Сейчас".to_string(),
                        value: snapshot.energy,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            let max = state.peek().energy_max;
                            state.write().energy = value.clamp(0.0, max);
                        },
                    }
                    DevNumberRow {
                        label: "Максимум".to_string(),
                        value: snapshot.energy_max,
                        step: "10".to_string(),
                        on_change: move |value: f64| {
                            let mut guard = state.write();
                            guard.energy_max = value.max(1.0);
                            guard.energy = guard.energy.min(guard.energy_max);
                        },
                    }
                    DevNumberRow {
                        label: "В секунду (бонус)".to_string(),
                        value: dev.energy_regen,
                        step: "0.5".to_string(),
                        on_change: move |value: f64| {
                            state.write().dev.energy_regen = value.max(0.0);
                        },
                    }
                    DevNumberRow {
                        label: "За отдых (бонус)".to_string(),
                        value: dev.rest_bonus,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().dev.rest_bonus = value.max(0.0);
                        },
                    }
                    DevNumberRow {
                        label: "За клик добычи (бонус)".to_string(),
                        value: dev.gather_yield,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().dev.gather_yield = value.max(0.0);
                        },
                    }
                }

                section { class: "dev-group",
                    h3 { class: "dev-group-title", "Постройки" }
                    DevNumberRow {
                        label: "Дом, уровень".to_string(),
                        value: snapshot.house_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().house_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Склад, уровень".to_string(),
                        value: snapshot.storage_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().storage_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Дом для работников".to_string(),
                        value: snapshot.worker_house_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().worker_house_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Дом для изучения".to_string(),
                        value: snapshot.research_house_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().research_house_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Лесопилка, уровень".to_string(),
                        value: snapshot.sawmill_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().sawmill_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Угольная яма, уровень".to_string(),
                        value: snapshot.coal_pit_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().coal_pit_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Работников: лесопилка".to_string(),
                        value: snapshot.sawmill_workers as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().sawmill_workers = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Работников: угольная яма".to_string(),
                        value: snapshot.coal_pit_workers as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().coal_pit_workers = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Исследователей".to_string(),
                        value: snapshot.coal_pit_researchers as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().coal_pit_researchers = value.max(0.0) as u32;
                        },
                    }
                    button {
                        class: "dev-btn dev-wide",
                        onclick: move |_| {
                            let mut guard = state.write();
                            guard.coal_pit_researched = !guard.coal_pit_researched;
                            if guard.coal_pit_researched {
                                guard.coal_pit_level = guard.coal_pit_level.max(1);
                                guard.coal_pit_research = balance::COAL_PIT_RESEARCH_COST;
                                guard.coal_seen = true;
                            }
                        },
                        if snapshot.coal_pit_researched {
                            "Сбросить изучение угольной ямы"
                        } else {
                            "Изучить угольную яму сразу"
                        }
                    }
                }

                section { class: "dev-group",
                    h3 { class: "dev-group-title", "Ресурсы" }
                    DevResourceRow {
                        icon: "🌲".to_string(),
                        name: "Дерево".to_string(),
                        value: snapshot.wood,
                        speed: dev.wood_per_sec,
                        on_value: move |value: f64| {
                            let mut guard = state.write();
                            guard.wood = value.max(0.0);
                            guard.refresh_unlocks();
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.wood_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "🪨".to_string(),
                        name: "Камень".to_string(),
                        value: snapshot.stone,
                        speed: dev.stone_per_sec,
                        on_value: move |value: f64| {
                            state.write().stone = value.max(0.0);
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.stone_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "⛏️".to_string(),
                        name: "Железо".to_string(),
                        value: snapshot.iron,
                        speed: dev.iron_per_sec,
                        on_value: move |value: f64| {
                            state.write().iron = value.max(0.0);
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.iron_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "🪙".to_string(),
                        name: "Уголь".to_string(),
                        value: snapshot.coal,
                        speed: dev.coal_per_sec,
                        on_value: move |value: f64| {
                            state.write().coal = value.max(0.0);
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.coal_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "💰".to_string(),
                        name: "Золото".to_string(),
                        value: snapshot.gold,
                        speed: dev.gold_per_sec,
                        on_value: move |value: f64| {
                            state.write().gold = value.max(0.0);
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.gold_per_sec = value.max(0.0);
                        },
                    }
                }

                button {
                    class: "dev-reset",
                    onclick: move |_| {
                        let mut guard = state.write();
                        guard.dev = Default::default();
                        guard.energy = guard.energy_max;
                    },
                    "Сбросить настройки разработчика"
                }
            }
        }
    }
}

#[component]
fn DevNumberRow(
    label: String,
    value: f64,
    step: String,
    on_change: EventHandler<f64>,
) -> Element {
    rsx! {
        div { class: "dev-row",
            span { class: "dev-label", "{label}" }
            input {
                class: "dev-input",
                r#type: "number",
                step: "{step}",
                value: "{value}",
                onchange: move |event| {
                    if let Ok(parsed) = event.value().parse::<f64>() {
                        on_change.call(parsed);
                    }
                },
            }
        }
    }
}

#[component]
fn DevResourceRow(
    icon: String,
    name: String,
    value: f64,
    speed: f64,
    on_value: EventHandler<f64>,
    on_speed: EventHandler<f64>,
) -> Element {
    rsx! {
        div { class: "dev-resource",
            div { class: "dev-resource-head",
                span { class: "dev-resource-name", "{icon} {name}" }
                span { class: "dev-resource-value", "{format_num(value)}" }
            }
            div { class: "dev-row",
                span { class: "dev-label", "Количество" }
                input {
                    class: "dev-input",
                    r#type: "number",
                    step: "1",
                    value: "{value}",
                    onchange: move |event| {
                        if let Ok(parsed) = event.value().parse::<f64>() {
                            on_value.call(parsed);
                        }
                    },
                }
                button { class: "dev-btn", onclick: move |_| on_value.call(value + 10.0), "+10" }
                button { class: "dev-btn", onclick: move |_| on_value.call(value + 100.0), "+100" }
                button { class: "dev-btn", onclick: move |_| on_value.call(0.0), "0" }
            }
            div { class: "dev-row",
                span { class: "dev-label", "В секунду" }
                input {
                    class: "dev-input",
                    r#type: "number",
                    step: "0.1",
                    value: "{speed}",
                    onchange: move |event| {
                        if let Ok(parsed) = event.value().parse::<f64>() {
                            on_speed.call(parsed);
                        }
                    },
                }
            }
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
                span { class: chev_class }
            }
            if is_open {
                div { class: "section-body", {children} }
            }
        }
    }
}

#[component]
fn ActionRow(label: String, info: String, available: bool, on_press: EventHandler<()>) -> Element {
    let mut open = use_signal(|| false);
    let is_open = open();
    let chev_class = if is_open { "chev open" } else { "chev" };

    rsx! {
        div { class: "action",
            div { class: "action-line",
                button {
                    class: if available { "action-main" } else { "action-main unavailable" },
                    disabled: !available,
                    onclick: move |_| on_press.call(()),
                    span { class: "action-label", "{label}" }
                }
                button {
                    class: "chev-btn",
                    aria_label: "Подробнее",
                    onclick: move |_| open.set(!open()),
                    span { class: chev_class }
                }
            }
            if is_open {
                div { class: "action-info", "{info}" }
            }
        }
    }
}

#[component]
fn BuildRow(
    title: String,
    level: u32,
    description: String,
    cost: String,
    affordable: bool,
    on_build: EventHandler<()>,
) -> Element {
    let mut open = use_signal(|| false);
    let is_open = open();
    let chev_class = if is_open { "chev open" } else { "chev" };
    let level_label = if level == 0 {
        "не построено".to_string()
    } else {
        format!("уровень {level}")
    };
    let button_label = if level == 0 { "Построить" } else { "Улучшить" };

    rsx! {
        div { class: "build",
            div { class: "build-sub",
                span { class: "build-title", "{title}" }
                span { class: "build-level", "{level_label}" }
                span {
                    class: if affordable { "build-cost" } else { "build-cost poor" },
                    "{cost}"
                }
            }
            div { class: "action-line",
                button {
                    class: if affordable { "action-main" } else { "action-main unavailable" },
                    disabled: !affordable,
                    onclick: move |_| on_build.call(()),
                    span { class: "action-label", "{button_label}" }
                }
                button {
                    class: "chev-btn",
                    aria_label: "Подробнее",
                    onclick: move |_| open.set(!open()),
                    span { class: chev_class }
                }
            }
            if is_open {
                div { class: "build-info", "{description}" }
            }
        }
    }
}

#[component]
fn CurrentGoal(state: GameState) -> Element {
    let total = balance::REST_CLICKS_TO_UNLOCK_GATHER;

    let (goal_text, goal_hint, goal_progress) = if !state.gathering_unlocked() {
        let done = state.rest_clicks.min(total);
        let left = total.saturating_sub(done);
        let width = format!("{:.0}%", done as f64 / total as f64 * 100.0);
        let count = format!("{done} / {total}");
        (
            format!("Отдохни ещё {left}, чтобы открыть добычу дерева."),
            String::new(),
            Some((width, count)),
        )
    } else if state.house_level == 0 {
        (
            "Построй свой дом — он даст энергию и усилит отдых.".to_string(),
            cost_text(balance::house_cost(0)),
            None,
        )
    } else if state.worker_house_level == 0 {
        (
            "Построй дом для работников, чтобы получить первого работника.".to_string(),
            cost_text(balance::worker_house_cost(0)),
            None,
        )
    } else if state.research_house_level == 0 {
        (
            "Построй дом для изучения, чтобы начать исследования.".to_string(),
            cost_text(balance::research_house_cost(0)),
            None,
        )
    } else if state.sawmill_level == 0 {
        (
            "Построй лесопилку и назначь на неё работника.".to_string(),
            cost_text(balance::sawmill_cost(0)),
            None,
        )
    } else if state.sawmill_workers == 0 {
        (
            "Назначь работника на лесопилку во вкладке «Труд».".to_string(),
            String::new(),
            None,
        )
    } else if !state.coal_pit_researched {
        (
            "Назначь исследователя на угольную яму во вкладке «Изучения».".to_string(),
            String::new(),
            None,
        )
    } else if state.wood >= state.cap(ResourceKind::Wood)
        || state.stone >= state.cap(ResourceKind::Stone)
    {
        (
            "Склад полон. Улучши склад, чтобы добывать дальше.".to_string(),
            cost_text(balance::storage_cost(state.storage_level)),
            None,
        )
    } else {
        (
            "Развивай дом: больше энергии за отдых и в секунду.".to_string(),
            cost_text(balance::house_cost(state.house_level)),
            None,
        )
    };

    rsx! {
        section { class: "goal",
            div { class: "goal-head",
                span { class: "goal-label", "Текущая цель" }
                if let Some((_, count)) = goal_progress.clone() {
                    span { class: "goal-count", "{count}" }
                }
            }
            div { class: "goal-text", "{goal_text}" }
            match goal_progress {
                Some((width, _)) => rsx! {
                    div { class: "goal-track",
                        div { class: "goal-fill", style: "width: {width}" }
                    }
                },
                None => rsx! {
                    if !goal_hint.is_empty() {
                        div { class: "goal-cost", "{goal_hint}" }
                    }
                },
            }
        }
    }
}

#[component]
fn ResourceBar(state: GameState) -> Element {
    let mut items: Vec<(ResourceKind, &str, f64, f64)> = Vec::new();

    if state.wood_seen || state.wood > 0.0 {
        items.push((
            ResourceKind::Wood,
            "Дерево",
            state.wood,
            state.cap(ResourceKind::Wood),
        ));
    }
    if state.stone_seen || state.stone > 0.0 {
        items.push((
            ResourceKind::Stone,
            "Камень",
            state.stone,
            state.cap(ResourceKind::Stone),
        ));
    }
    if state.coal_seen || state.coal > 0.0 {
        items.push((
            ResourceKind::Coal,
            "Уголь",
            state.coal,
            state.cap(ResourceKind::Coal),
        ));
    }

    rsx! {
        div { class: "resources",
            if items.is_empty() {
                span { class: "resources-empty", "Ресурсов пока нет — отдохни и начни добывать" }
            } else {
                for (kind, name, value, cap) in items {
                    div { class: "resource", key: "{name}",
                        if kind == ResourceKind::Wood {
                            img { class: "resource-img", src: WOOD_ICON, alt: "Дерево" }
                        } else if kind == ResourceKind::Coal {
                            img { class: "resource-img", src: COAL_ICON, alt: "Уголь" }
                        } else {
                            span { class: "resource-icon", "🪨" }
                        }
                        span { class: "resource-name", "{name}" }
                        span {
                            class: if value >= cap { "resource-value full" } else { "resource-value" },
                            "{format_num(value)} / {format_num(cap)}"
                        }
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
                    if i == TAB_FIELD {
                        img {
                            class: "tab-img",
                            src: FIELD_ICON,
                            alt: "Поле",
                        }
                    } else {
                        span { class: "tab-icon", "{icon}" }
                    }
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
        state.energy_regen()
    );

    rsx! {
        div { class: "energy",
            div { class: "energy-fill", style: "{fill_style}" }
            div { class: "energy-content",
                span { class: "energy-bolt", "⚡" }
                span { class: "energy-label", "{label}" }
            }
        }
    }
}

fn do_rest(mut state: Signal<GameState>, toast: Signal<String>, toast_seq: Signal<u64>) {
    let was_unlocked = state.peek().gathering_unlocked();
    let gained = {
        let mut guard = state.write();
        actions::rest(&mut guard)
    };
    let now_unlocked = state.peek().gathering_unlocked();

    if !was_unlocked && now_unlocked {
        notify(toast, toast_seq, "Открылась добыча дерева!");
    } else {
        notify(toast, toast_seq, rest_message(gained));
    }
}

fn do_gather(
    mut state: Signal<GameState>,
    toast: Signal<String>,
    toast_seq: Signal<u64>,
    kind: ResourceKind,
) {
    let result = {
        let mut guard = state.write();
        actions::gather(&mut guard, kind)
    };
    // Успешная добыча не показывает плашку — только ошибки и открытия.
    if !matches!(result, GatherResult::Gathered(value) if value > 0.0) {
        notify(toast, toast_seq, gather_message(kind, result));
    }
}

fn do_build_with(
    mut state: Signal<GameState>,
    toast: Signal<String>,
    toast_seq: Signal<u64>,
    name: &'static str,
    action: fn(&mut GameState) -> BuildResult,
) {
    let result = {
        let mut guard = state.write();
        action(&mut guard)
    };

    let message = match result {
        BuildResult::Built(level) => format!("{name}: уровень {level}"),
        other => build_message(name, other),
    };
    notify(toast, toast_seq, message);
}

fn do_build_house(mut state: Signal<GameState>, toast: Signal<String>, toast_seq: Signal<u64>) {
    let result = {
        let mut guard = state.write();
        actions::build_house(&mut guard)
    };

    let message = match result {
        BuildResult::Built(level) => {
            format!("Дом улучшен до ур. {level}. Энергия: +{level}/с")
        }
        other => build_message("Дом", other),
    };
    notify(toast, toast_seq, message);
}

fn do_build_storage(mut state: Signal<GameState>, toast: Signal<String>, toast_seq: Signal<u64>) {
    let result = {
        let mut guard = state.write();
        actions::build_storage(&mut guard)
    };

    let message = match result {
        BuildResult::Built(level) => {
            let cap = state.peek().cap(ResourceKind::Wood);
            format!("Склад улучшен до ур. {level}. Лимит: {}", format_num(cap))
        }
        other => build_message("Склад", other),
    };
    notify(toast, toast_seq, message);
}

fn reset_progress(
    mut state: Signal<GameState>,
    toast: Signal<String>,
    toast_seq: Signal<u64>,
    mut reset_armed: Signal<bool>,
) {
    if *reset_armed.peek() {
        let theme = state.peek().theme;
        let fresh = GameState {
            theme,
            ..GameState::default()
        };
        state.set(fresh);
        reset_armed.set(false);

        spawn(async move {
            if let Err(error) = save::clear().await {
                eprintln!("Не удалось удалить сохранение: {error}");
            }
        });
        notify(toast, toast_seq, "Прогресс сброшен");
    } else {
        reset_armed.set(true);
        notify(toast, toast_seq, "Нажми ещё раз для сброса");

        spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if *reset_armed.peek() {
                reset_armed.set(false);
            }
        });
    }
}

fn notify(mut toast: Signal<String>, mut toast_seq: Signal<u64>, message: impl Into<String>) {
    let id = *toast_seq.peek() + 1;
    toast_seq.set(id);
    toast.set(message.into());

    spawn(async move {
        tokio::time::sleep(Duration::from_millis(1800)).await;
        if *toast_seq.peek() == id {
            toast.set(String::new());
        }
    });
}

fn theme_css_class(theme: Theme) -> &'static str {
    match theme {
        Theme::Classic => "theme-classic",
        Theme::Dark => "theme-dark",
    }
}

fn cost_text(cost: balance::Cost) -> String {
    format!(
        "🌲 {}   🪨 {}",
        format_num(cost.wood),
        format_num(cost.stone)
    )
}

fn gather_message(kind: ResourceKind, result: GatherResult) -> String {
    match result {
        GatherResult::Gathered(value) if value > 0.0 => {
            format!("+{} {}", format_num(value), kind.name())
        }
        GatherResult::Gathered(_) => "Склад полон".to_string(),
        GatherResult::NoEnergy => "Не хватает энергии — отдохни".to_string(),
        GatherResult::StorageFull => format!("Склад полон: {}", kind.name()),
        GatherResult::Locked => "Это ещё не открыто".to_string(),
    }
}

fn build_message(name: &str, result: BuildResult) -> String {
    match result {
        BuildResult::Built(level) => format!("{name}: уровень {level}"),
        BuildResult::NotEnough(cost) => format!(
            "Не хватает: нужно {} дерева и {} камня",
            format_num(cost.wood),
            format_num(cost.stone)
        ),
        BuildResult::Locked => "Ещё не открыто".to_string(),
        BuildResult::MaxLevel => "Максимальный уровень".to_string(),
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
