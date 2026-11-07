use std::time::Duration;

use dioxus::prelude::*;

use crate::game::actions::{self, BuildResult, GatherResult};
use crate::game::state::{ResourceKind, Theme};
use crate::game::{balance, GameState};

use super::save;

const MAIN_CSS: Asset = asset!("/assets/main.css");

const TAB_FIELD: usize = 0;
const TAB_SETTINGS: usize = 7;

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
    let title = if active_tab() == TAB_SETTINGS {
        "Настройки"
    } else {
        "Поле"
    };

    let screen = match active_tab() {
        TAB_FIELD => rsx! {
            FieldScreen { state, toast, toast_seq }
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

    let rests_left = balance::REST_CLICKS_TO_UNLOCK_GATHER.saturating_sub(snapshot.rest_clicks);
    let house_cost = balance::house_cost(snapshot.house_level);
    let storage_cost = balance::storage_cost(snapshot.storage_level);
    let house_affordable = snapshot.wood >= house_cost.wood && snapshot.stone >= house_cost.stone;
    let storage_affordable =
        snapshot.wood >= storage_cost.wood && snapshot.stone >= storage_cost.stone;

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

    rsx! {
        CollapsibleSection { title: "Отдых".to_string(),
            ActionRow {
                label: "Отдых".to_string(),
                info: rest_info,
                on_press: move |_| do_rest(state, toast, toast_seq),
            }
            if !snapshot.gathering_unlocked() {
                div { class: "hint",
                    "Осталось отдохнуть: {rests_left}. После этого откроется добыча дерева."
                }
            } else if snapshot.house_level == 0 {
                div { class: "hint",
                    "Построй свой дом — он начнёт восстанавливать энергию сам."
                }
            }
        }

        if snapshot.gathering_unlocked() {
            CollapsibleSection { title: "Сбор".to_string(),
                ActionRow {
                    label: "Собрать дерево".to_string(),
                    info: "+1 дерево за клик (−1 энергия).".to_string(),
                    on_press: move |_| do_gather(state, toast, toast_seq, ResourceKind::Wood),
                }
                if snapshot.stone_unlocked() {
                    ActionRow {
                        label: "Собрать камень".to_string(),
                        info: "+1 камень за клик (−1 энергия).".to_string(),
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
            p { class: "settings-note", "Цвет энергии одинаковый во всех темах: #BAD118." }
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
            p { class: "settings-note",
                "Сброс вернёт игру в самое начало. Тема оформления сохранится."
            }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Об игре" }
            p { class: "settings-note", "«Поле» — версия v0.3. Rust + Dioxus 0.7." }
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
            button { class: "icon-btn", "☰" }
            h1 { class: "topbar-title", "{title}" }
            button { class: "icon-btn", "🔔" }
            button {
                class: "icon-btn dev-icon",
                title: "Панель разработчика (временно)",
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
fn BuildRow(
    title: String,
    level: u32,
    description: String,
    cost: String,
    affordable: bool,
    on_build: EventHandler<()>,
) -> Element {
    let level_label = if level == 0 {
        "не построено".to_string()
    } else {
        format!("уровень {level}")
    };
    let button_label = if level == 0 { "Построить" } else { "Улучшить" };

    rsx! {
        div { class: "build",
            div { class: "build-head",
                span { class: "build-title", "{title}" }
                span { class: "build-level", "{level_label}" }
            }
            div { class: "build-desc", "{description}" }
            div {
                class: if affordable { "build-cost" } else { "build-cost poor" },
                "{cost}"
            }
            button {
                class: "build-btn",
                onclick: move |_| on_build.call(()),
                "{button_label}"
            }
        }
    }
}

#[component]
fn ResourceBar(state: GameState) -> Element {
    let mut items: Vec<(&str, &str, f64, f64)> = Vec::new();

    if state.wood_seen || state.wood > 0.0 {
        items.push((
            "🌲",
            "Дерево",
            state.wood,
            state.cap(ResourceKind::Wood),
        ));
    }
    if state.stone_seen || state.stone > 0.0 {
        items.push((
            "🪨",
            "Камень",
            state.stone,
            state.cap(ResourceKind::Stone),
        ));
    }

    rsx! {
        div { class: "resources",
            if items.is_empty() {
                span { class: "resources-empty", "Ресурсов пока нет — отдохни и начни добывать" }
            } else {
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
        state.energy_regen()
    );

    rsx! {
        div { class: "energy",
            div { class: "energy-fill", style: "{fill_style}" }
            div { class: "energy-label", "{label}" }
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
    notify(toast, toast_seq, gather_message(kind, result));
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
        "Нужно: {} дерева, {} камня",
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
