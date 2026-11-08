use std::time::Duration;

use dioxus::prelude::*;

use crate::game::actions::{self, BuildResult};
use crate::game::state::{AchievementStatus, ResearchKind, ResourceKind, Theme};
use crate::game::{balance, GameState};

use super::inventory::InventoryScreen;
use super::save;
use super::trader::TradeScreen;

/// Снимок состояния для UI: обновляется после каждого действия и раз в тик.
static VIEW: GlobalSignal<GameState> = Signal::global(GameState::default);

const MAIN_CSS: Asset = asset!("/assets/main.css");
const FIELD_ICON: Asset = asset!("/assets/icons/field.png");
const WOOD_ICON: Asset = asset!("/assets/icons/wood.png");
const STONE_ICON: Asset = asset!("/assets/icons/stone.png");
const IRON_ICON: Asset = asset!("/assets/icons/iron.png");
const COAL_ICON: Asset = asset!("/assets/icons/coal.png");
const GOLD_ICON: Asset = asset!("/assets/icons/gold.png");
const BAG_ICON: Asset = asset!("/assets/icons/bag.png");
const SCROLL_ICON: Asset = asset!("/assets/icons/scroll.png");
const GEAR_ICON: Asset = asset!("/assets/icons/gear.png");

const TAB_FIELD: usize = 0;
const TAB_STUDIES: usize = 1;
const TAB_WORK: usize = 2;
const TAB_BAG: usize = 3;
const TAB_CYCLE: usize = 5;
const TAB_STATS: usize = 6;
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

/// Картинка вкладки, если для неё уже нарисован спрайт.
fn tab_image(index: usize) -> Option<Asset> {
    match index {
        TAB_FIELD => Some(FIELD_ICON),
        TAB_STUDIES => Some(SCROLL_ICON),
        TAB_BAG => Some(BAG_ICON),
        TAB_SETTINGS => Some(GEAR_ICON),
        _ => None,
    }
}

/// Спрайт ресурса для строки ресурсов и цен.
fn resource_icon(kind: ResourceKind) -> Option<Asset> {
    match kind {
        ResourceKind::Wood => Some(WOOD_ICON),
        ResourceKind::Stone => Some(STONE_ICON),
        ResourceKind::Iron => Some(IRON_ICON),
        ResourceKind::Coal => Some(COAL_ICON),
        ResourceKind::Gold => Some(GOLD_ICON),
    }
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(GameState::default);
    let mut active_tab = use_signal(|| TAB_FIELD);
    let mut bell_open = use_signal(|| false);
    let mut trader_open = use_signal(|| false);
    let mut loaded = use_signal(|| false);
    let mut dev_open = use_signal(|| false);
    let mut back = use_signal(|| 0u32);

    // Загружаем сохранение один раз при старте.
    use_future(move || async move {
        match save::load().await {
            Ok(Some(mut saved)) => {
                let now = actions::unix_now();
                if saved.last_seen > 0 && now > saved.last_seen {
                    let gained = actions::offline_time((now - saved.last_seen) as f64);
                    if gained > 0.0 {
                        saved.time = (saved.time + gained).min(balance::TIME_CAP);
                    }
                }
                saved.last_seen = now;
                saved.refresh_unlocks();
                saved.migrate();
                state.set(saved.clone());
                *VIEW.write() = saved;
            }
            Ok(None) => {}
            Err(error) => eprintln!("Не удалось загрузить сохранение: {error}"),
        }
        loaded.set(true);
    });

    // Игровой цикл: энергия восстанавливается только от дома.
    // После тика обновляем снимок UI; действия игрока обновляют его сразу.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(balance::TICK_MS)).await;
            if !*loaded.peek() {
                continue;
            }
            let snapshot = {
                let mut guard = state.write();
                actions::tick(&mut guard, 1.0);
                guard.clone()
            };
            *VIEW.write() = snapshot;
        }
    });

    // Автосохранение раз в пять секунд.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if !*loaded.peek() {
                continue;
            }
            let mut snapshot = state.peek().clone();
            snapshot.last_seen = actions::unix_now();
            if let Err(error) = save::save(&snapshot).await {
                eprintln!("Не удалось сохранить игру: {error}");
            }
        }
    });

    // Системная кнопка «Назад» и свайп слева направо поднимают счётчик `back`.
    use_future(move || async move {
        let script = r#"
            (() => {
                const send = (message) => {
                    try { dioxus.send(message); } catch (error) {}
                };
                history.pushState({ pole: true }, '');
                window.addEventListener('popstate', () => {
                    send('back');
                    history.pushState({ pole: true }, '');
                });
                let startX = null;
                let startY = null;
                document.addEventListener('touchstart', (event) => {
                    if (event.touches.length !== 1) return;
                    const touch = event.touches[0];
                    if (touch.clientX <= 40) {
                        startX = touch.clientX;
                        startY = touch.clientY;
                    } else {
                        startX = null;
                    }
                }, { passive: true });
                document.addEventListener('touchend', (event) => {
                    if (startX === null) return;
                    const touch = event.changedTouches[0];
                    const dx = touch.clientX - startX;
                    const dy = Math.abs(touch.clientY - startY);
                    startX = null;
                    if (dx > 80 && dy < 70) send('back');
                }, { passive: true });
            })();
        "#;
        let mut eval = document::eval(script);
        loop {
            match eval.recv::<String>().await {
                Ok(message) if message == "back" => {
                    back.set(back() + 1);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    // Экраны перерисовываются по снимку `VIEW`; сами действия пишут в `state`.
    let snapshot = VIEW();
    let theme_class = theme_css_class(snapshot.theme);
    let title = match active_tab() {
        TAB_SETTINGS => "Настройки",
        TAB_STUDIES => "Изучения",
        TAB_WORK => "Труд",
        TAB_BAG => "Сумка",
        TAB_CYCLE => "Цикл",
        TAB_STATS => "Статы",
        _ => "Поле",
    };

    let screen = if trader_open() {
        rsx! {}
    } else {
        match active_tab() {
            TAB_FIELD => rsx! {
                FieldScreen { state, view: snapshot.clone() }
            },
            TAB_STUDIES => rsx! {
                StudiesScreen { state, view: snapshot.clone() }
            },
            TAB_WORK => rsx! {
                WorkScreen { state, view: snapshot.clone() }
            },
            TAB_BAG => rsx! {
                InventoryScreen {
                    state,
                    view: snapshot.clone(),
                    back,
                    on_back: move |_| active_tab.set(TAB_FIELD),
                    on_trade: move |_| trader_open.set(true),
                }
            },
            TAB_CYCLE => rsx! {
                CycleScreen { state, view: snapshot.clone() }
            },
            TAB_STATS => rsx! {
                StatsScreen { view: snapshot.clone() }
            },
            TAB_SETTINGS => rsx! {
                SettingsScreen { state, view: snapshot.clone() }
            },
            index => rsx! {
                PlaceholderScreen { title: TABS[index].1.to_string() }
            },
        }
    };

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app {theme_class}",
            TopBar {
                title: title.to_string(),
                bell_open: bell_open(),
                trader_open: trader_open(),
                on_bell: move |_| bell_open.set(!bell_open()),
                on_trader: move |_| trader_open.set(!trader_open()),
                on_dev: move |_| dev_open.set(!dev_open()),
            }
            main { class: "content", {screen} }
            div { class: "bottom",
                ResourceBar { view: snapshot.clone() }
                TabBar { active: active_tab }
                EnergyBar { view: snapshot.clone() }
            }
            if bell_open() {
                GoalPanel { view: snapshot.clone(), on_close: move |_| bell_open.set(false) }
            }
            if trader_open() {
                TradeScreen {
                    state,
                    view: snapshot.clone(),
                    back,
                    on_close: move |_| trader_open.set(false),
                }
            }
            if dev_open() {
                DevPanel { state, on_close: move |_| dev_open.set(false) }
            }
        }
    }
}

#[component]
fn FieldScreen(state: Signal<GameState>, view: GameState) -> Element {
    let snapshot = view;

    let storage_cost = balance::storage_cost(snapshot.storage_level);
    let worker_house_cost = balance::worker_house_cost(snapshot.worker_house_level);
    let research_house_cost = balance::research_house_cost(snapshot.research_house_level);
    let sawmill_cost = balance::sawmill_cost(snapshot.sawmill_level);
    let mine_cost = balance::mine_cost(snapshot.mine_level);
    let coal_pit_cost = balance::coal_pit_cost(snapshot.coal_pit_level);
    let forge_cost = balance::forge_cost(snapshot.forge_level);
    let bank_cost = balance::bank_cost(snapshot.bank_level);
    let house_cost = balance::house_cost(snapshot.house_level);
    let comfort_cost = balance::comfort_cost(snapshot.comfort_level);

    let affordable =
        |cost: balance::Cost| snapshot.wood >= cost.wood && snapshot.stone >= cost.stone;
    let storage_affordable = affordable(storage_cost);
    let worker_house_affordable = affordable(worker_house_cost);
    let research_house_affordable = affordable(research_house_cost);
    let sawmill_affordable = affordable(sawmill_cost);
    let mine_affordable = affordable(mine_cost);
    let coal_pit_affordable = affordable(coal_pit_cost);
    let forge_affordable = affordable(forge_cost);
    let bank_affordable = affordable(bank_cost);
    let house_affordable = affordable(house_cost);
    let comfort_affordable = affordable(comfort_cost);

    let gather_yield =
        (balance::CLICK_YIELD + snapshot.dev.gather_yield) * snapshot.production_multiplier();
    let rest_available = snapshot.energy < snapshot.max_energy();
    let wood_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && snapshot.wood < snapshot.cap(ResourceKind::Wood);
    let stone_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && snapshot.stone < snapshot.cap(ResourceKind::Stone);

    let fire_max = snapshot.fire_max();
    let fire_ratio = if fire_max > 0.0 {
        (snapshot.forge_fire / fire_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fire_fill = format!("width: {:.1}%", fire_ratio * 100.0);
    let can_feed = snapshot.coal >= 1.0 && snapshot.forge_fire < fire_max - 0.01;
    let auto_class = if snapshot.forge_auto_feed {
        "toggle-btn on"
    } else {
        "toggle-btn"
    };
    let time_available = snapshot.time >= 1.0;
    let time_warp_label = format!(
        "Провернуть {} с (−1 время)",
        format_num(balance::TIME_WARP_SECONDS)
    );

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
        "Даёт 1 работника. Работников можно назначить на лесопилку и угольную яму.".to_string()
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

    let mine_description = format!(
        "Работники добывают железо: +{}/с с каждого. Уровень даёт одно рабочее место.",
        format_num(balance::MINE_IRON_PER_WORKER_PER_SEC)
    );

    let forge_description = format!(
        "Огонь из угля нужен для крафта предметов. Один уголь — {} с огня. Уровень повышает запас огня.",
        format_num(balance::FIRE_SECONDS_PER_COAL)
    );

    let bank_description = format!(
        "Сейчас лимит золота: {}. Улучшение добавит +{}.",
        format_num(snapshot.gold_cap()),
        format_num(balance::GOLD_CAP_PER_LEVEL)
    );

    let house_description = if snapshot.house_level == 0 {
        "После постройки: +1.5 энергии за отдых и +1 энергии в секунду.".to_string()
    } else {
        format!(
            "Сейчас: +{} за отдых, +{}/с. Улучшение добавит ещё +1 к отдыху и +1/с.",
            format_num(snapshot.rest_energy()),
            format_num(snapshot.energy_regen())
        )
    };

    let comfort_description = format!(
        "Сейчас максимум энергии: {}. Улучшение добавит +{}.",
        format_num(snapshot.max_energy()),
        format_num(balance::ENERGY_CAP_PER_COMFORT)
    );

    rsx! {
        CollapsibleSection { title: "Энергия".to_string(),
            div { class: "goal",
                div { class: "goal-head",
                    span { class: "goal-label", "Энергия" }
                    span { class: "goal-count",
                        "{format_num(snapshot.energy)} / {format_num(snapshot.max_energy())}"
                    }
                }
                div { class: "goal-text",
                    "Отдых даёт +{format_num(snapshot.rest_energy())} за клик. Пассивно: +{format_num(snapshot.energy_regen())}/с."
                }
            }
            BuildRow {
                title: "Свой дом".to_string(),
                level: snapshot.house_level,
                description: house_description,
                cost: house_cost,
                affordable: house_affordable,
                on_build: move |_| do_build_house(state),
            }
            BuildRow {
                title: "Уют".to_string(),
                level: snapshot.comfort_level,
                description: comfort_description,
                cost: comfort_cost,
                affordable: comfort_affordable,
                on_build: move |_| do_build_comfort(state),
            }
        }

        CollapsibleSection { title: "Отдых".to_string(),
            ActionRow {
                label: "Отдых".to_string(),
                info: rest_info,
                available: rest_available,
                on_press: move |_| do_rest(state),
            }
        }

        if snapshot.gathering_unlocked() {
            CollapsibleSection { title: "Сбор".to_string(),
                ActionRow {
                    label: "Дерево".to_string(),
                    info: format!("+{} за клик (−1 энергия).", format_num(gather_yield)),
                    available: wood_available,
                    on_press: move |_| do_gather(state, ResourceKind::Wood),
                }
                if snapshot.stone_unlocked() {
                    ActionRow {
                        label: "Камень".to_string(),
                        info: format!("+{} за клик (−1 энергия).", format_num(gather_yield)),
                        available: stone_available,
                        on_press: move |_| do_gather(state, ResourceKind::Stone),
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
                    cost: storage_cost,
                    affordable: storage_affordable,
                    on_build: move |_| do_build_storage(state),
                }
            }
        }

        if snapshot.house_level > 0 {
            CollapsibleSection { title: "Дома".to_string(),
                BuildRow {
                    title: "Дом для работников".to_string(),
                    level: snapshot.worker_house_level,
                    description: worker_house_description,
                    cost: worker_house_cost,
                    affordable: worker_house_affordable,
                    on_build: move |_| do_build_with(state, actions::build_worker_house),
                }
                BuildRow {
                    title: "Дом для изучения".to_string(),
                    level: snapshot.research_house_level,
                    description: research_house_description,
                    cost: research_house_cost,
                    affordable: research_house_affordable,
                    on_build: move |_| do_build_with(state, actions::build_research_house),
                }
            }
        }

        if snapshot.house_level > 0 {
            CollapsibleSection { title: "Постройки".to_string(),
                BuildRow {
                    title: "Лесопилка".to_string(),
                    level: snapshot.sawmill_level,
                    description: sawmill_description,
                    cost: sawmill_cost,
                    affordable: sawmill_affordable,
                    on_build: move |_| do_build_with(state, actions::build_sawmill),
                }
                if snapshot.coal_pit_researched {
                    BuildRow {
                        title: "Угольная яма".to_string(),
                        level: snapshot.coal_pit_level,
                        description: coal_pit_description,
                        cost: coal_pit_cost,
                        affordable: coal_pit_affordable,
                        on_build: move |_| do_build_with(state, actions::build_coal_pit),
                    }
                    BuildRow {
                        title: "Шахта".to_string(),
                        level: snapshot.mine_level,
                        description: mine_description,
                        cost: mine_cost,
                        affordable: mine_affordable,
                        on_build: move |_| do_build_with(state, actions::build_mine),
                    }
                } else {
                    div { class: "locked-note",
                        "Угольную яму и шахту можно открыть на вкладке «Изучения»."
                    }
                }
            }
        }

        if snapshot.forge_researched {
            CollapsibleSection { title: "Кузница".to_string(),
                BuildRow {
                    title: "Кузница".to_string(),
                    level: snapshot.forge_level,
                    description: forge_description,
                    cost: forge_cost,
                    affordable: forge_affordable,
                    on_build: move |_| do_build_with(state, actions::build_forge),
                }
                if snapshot.forge_level > 0 {
                    div { class: "forge",
                        div { class: "forge-head",
                            span { "Огонь" }
                            span { class: "forge-fire-value",
                                "{format_num(snapshot.forge_fire)} / {format_num(fire_max)} с"
                            }
                        }
                        div { class: "fire-track",
                            div { class: "fire-fill", style: "{fire_fill}" }
                        }
                        div { class: "forge-actions",
                            button {
                                class: if can_feed { "forge-btn" } else { "forge-btn disabled" },
                                disabled: !can_feed,
                                onclick: move |_| do_forge_feed(state),
                                "Подбросить уголь"
                            }
                            button {
                                class: "{auto_class}",
                                onclick: move |_| {
                                    {
                                        let mut guard = state.write();
                                        actions::toggle_forge_auto_feed(&mut guard);
                                    }
                                    refresh_view(state);
                                },
                                "Авто"
                            }
                        }
                        div { class: "forge-info",
                            "Крафт предметов тратит огонь. Сейчас: {format_num(snapshot.forge_fire)} с."
                        }
                    }
                }
            }
        }

        if snapshot.trader_researched {
            CollapsibleSection { title: "Банк".to_string(),
                BuildRow {
                    title: "Банк".to_string(),
                    level: snapshot.bank_level,
                    description: bank_description,
                    cost: bank_cost,
                    affordable: bank_affordable,
                    on_build: move |_| do_build_with(state, actions::build_bank),
                }
            }
        }

        if snapshot.time > 0.0 || snapshot.cycles > 0 {
            CollapsibleSection { title: "Время".to_string(),
                div { class: "time-box",
                    div { class: "time-value", "{format_num(snapshot.time)}" }
                    div { class: "time-note",
                        "Время копится, пока игра закрыта. Тратится на тайм-варп здесь и в изучениях."
                    }
                }
                button {
                    class: if time_available { "forge-btn" } else { "forge-btn disabled" },
                    disabled: !time_available,
                    onclick: move |_| do_time_warp(state),
                    "{time_warp_label}"
                }
            }
        }
    }
}

#[component]
fn StudiesScreen(state: Signal<GameState>, view: GameState) -> Element {
    let snapshot = view;
    let slots = snapshot.researcher_slots();
    let free = snapshot.researchers_free();
    let all_done = ResearchKind::ALL
        .iter()
        .filter(|kind| snapshot.research_available(**kind))
        .all(|kind| snapshot.research_done(*kind));

    rsx! {
        section { class: "goal",
            div { class: "goal-head",
                span { class: "goal-label", "Исследователи" }
                span { class: "goal-count", "{free} свободно из {slots}" }
            }
            div { class: "goal-text",
                if slots == 0 {
                    "Построй дом для изучения на вкладке «Поле»."
                } else if all_done {
                    "Все доступные изучения завершены."
                } else {
                    "Назначь исследователей на изучение или ускорь его временем."
                }
            }
        }

        section { class: "section",
            div { class: "section-head static",
                span { class: "section-title", "Изучения" }
            }
            div { class: "section-body",
                for kind in ResearchKind::ALL {
                    if snapshot.research_available(kind) {
                        ResearchCard { key: "{kind.name()}", kind, state, view: snapshot.clone() }
                    } else {
                        div { class: "locked-note", key: "{kind.name()}",
                            "{kind.name()}: сначала изучи {kind.requirement().unwrap_or(\"предыдущее\")}."
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ResearchCard(kind: ResearchKind, state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let snapshot = view;
    let done = snapshot.research_done(kind);
    let ratio = snapshot.research_ratio(kind);
    let percent = format!("{:.0}%", ratio * 100.0);
    let fill_style = format!("width: {:.1}%", ratio * 100.0);
    let speed = snapshot.research_speed(kind);
    let assigned = snapshot.researchers_for(kind);
    let can_minus = assigned > 0;
    let can_plus = !done && snapshot.researchers_free() > 0;
    let can_warp = !done && snapshot.time >= 1.0;
    let progress = snapshot.research_progress(kind);
    let cost = snapshot.research_cost(kind);

    rsx! {
        div { class: "research",
            div { class: "research-head",
                span { class: "research-name", "{kind.name()}" }
                span { class: if done { "research-status done" } else { "research-status" },
                    if done { "изучено" } else { "{percent}" }
                }
            }
            div { class: "research-desc",
                if done {
                    "Изучено. Система открыта."
                } else {
                    "{kind.description()}"
                }
            }
            div { class: "research-track",
                div { class: "research-fill", style: "{fill_style}" }
            }
            div { class: "research-progress-text",
                "{format_num(progress)} / {format_num(cost)} опыта"
            }
            div { class: "research-foot",
                span { class: "research-speed", "+{speed:.1} опыта/с" }
                div { class: "research-actions",
                    if !done {
                        button {
                            class: if can_warp { "warp-btn" } else { "warp-btn disabled" },
                            disabled: !can_warp,
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    actions::research_time_warp(&mut guard, kind);
                                }
                                refresh_view(state);
                            },
                            "⏩ 1 время"
                        }
                    }
                    div { class: "stepper",
                        button {
                            class: "step-btn",
                            disabled: !can_minus,
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    actions::adjust_researchers(&mut guard, kind, -1);
                                }
                                refresh_view(state);
                            },
                            "−"
                        }
                        span { class: "step-value", "{assigned}" }
                        button {
                            class: "step-btn",
                            disabled: !can_plus,
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    actions::adjust_researchers(&mut guard, kind, 1);
                                }
                                refresh_view(state);
                            },
                            "+"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn WorkScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let snapshot = view;
    let slots = snapshot.worker_slots();
    let free = snapshot.workers_free();
    let sawmill_workers = snapshot.sawmill_workers;
    let sawmill_slots = snapshot.sawmill_slots();
    let sawmill_rate = snapshot.sawmill_wood_per_sec();
    let mine_workers = snapshot.mine_workers;
    let mine_slots = snapshot.mine_slots();
    let mine_rate = snapshot.mine_iron_per_sec();
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
                    "Назначай работников на лесопилку, шахту и угольную яму."
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
                        {
                            let mut guard = state.write();
                            actions::adjust_sawmill_workers(&mut guard, -1);
                        }
                        refresh_view(state);
                    },
                    on_plus: move |_| {
                        {
                            let mut guard = state.write();
                            actions::adjust_sawmill_workers(&mut guard, 1);
                        }
                        refresh_view(state);
                    },
                }

                if snapshot.coal_pit_researched {
                    WorkerRow {
                        title: "Шахта".to_string(),
                        detail: format!("+{}/с железа", format_num(mine_rate)),
                        assigned: mine_workers,
                        capacity: mine_slots,
                        free,
                        on_minus: move |_| {
                            {
                                let mut guard = state.write();
                                actions::adjust_mine_workers(&mut guard, -1);
                            }
                            refresh_view(state);
                        },
                        on_plus: move |_| {
                            {
                                let mut guard = state.write();
                                actions::adjust_mine_workers(&mut guard, 1);
                            }
                            refresh_view(state);
                        },
                    }
                    WorkerRow {
                        title: "Угольная яма".to_string(),
                        detail: format!("+{}/с угля · −{}/с дерева", format_num(coal_rate), format_num(coal_wood)),
                        assigned: coal_workers,
                        capacity: coal_slots,
                        free,
                        on_minus: move |_| {
                            {
                                let mut guard = state.write();
                                actions::adjust_coal_pit_workers(&mut guard, -1);
                            }
                            refresh_view(state);
                        },
                        on_plus: move |_| {
                            {
                                let mut guard = state.write();
                                actions::adjust_coal_pit_workers(&mut guard, 1);
                            }
                            refresh_view(state);
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
fn CycleScreen(state: Signal<GameState>, view: GameState) -> Element {
    let snapshot = view;
    let mut armed = use_signal(|| false);
    let cycles = snapshot.cycles;
    let bonus = cycles as f64 * balance::CYCLE_BONUS_PER_CYCLE * 100.0;
    let ready = snapshot.forge_level > 0 && snapshot.iron >= balance::CYCLE_IRON_REQUIREMENT;
    let requirement = if snapshot.forge_level == 0 {
        "Построй кузницу.".to_string()
    } else if snapshot.iron < balance::CYCLE_IRON_REQUIREMENT {
        format!(
            "Нужно {} железа (сейчас {}).",
            format_num(balance::CYCLE_IRON_REQUIREMENT),
            format_num(snapshot.iron)
        )
    } else {
        "Всё готово для нового цикла.".to_string()
    };

    rsx! {
        section { class: "cycle-hero",
            div { class: "cycle-count", "{cycles}" }
            div { class: "cycle-label", "циклов пройдено" }
            div { class: "cycle-bonus", "Постоянный бонус: +{bonus:.0}%" }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Новый цикл" }
            p { class: "settings-note",
                "Сбросит ресурсы, постройки, изучения, работников и сумку. Останутся циклы, время, статистика и достижения."
            }
            p { class: "settings-note", "{requirement}" }
            button {
                class: if armed() { "danger-btn armed" } else { "danger-btn" },
                disabled: !ready,
                onclick: move |_| {
                    if !ready {
                        return;
                    }
                    if *armed.peek() {
                        do_prestige(state, armed);
                    } else {
                        armed.set(true);
                        spawn(async move {
                            tokio::time::sleep(Duration::from_secs(3)).await;
                            if *armed.peek() {
                                armed.set(false);
                            }
                        });
                    }
                },
                if armed() {
                    "Нажми ещё раз, чтобы начать цикл"
                } else {
                    "Начать новый цикл"
                }
            }
        }
    }
}

#[component]
fn StatsScreen(view: GameState) -> Element {
    let snapshot = view;
    let stats = &snapshot.stats;
    let achievements = snapshot.achievements();
    let done_count = achievements
        .iter()
        .filter(|achievement| achievement.done)
        .count();
    let total = achievements.len();

    rsx! {
        section { class: "settings",
            h2 { class: "settings-title", "Статистика" }
            StatRow { label: "Время в игре".to_string(), value: format_duration(stats.play_seconds) }
            StatRow { label: "Кликов добычи".to_string(), value: format_num(stats.clicks as f64) }
            StatRow { label: "Отдыхов".to_string(), value: format_num(stats.rests as f64) }
            StatRow { label: "Добыто дерева".to_string(), value: format_num(stats.wood_gathered) }
            StatRow { label: "Добыто камня".to_string(), value: format_num(stats.stone_gathered) }
            StatRow { label: "Получено угля".to_string(), value: format_num(stats.coal_produced) }
            StatRow { label: "Добыто железа".to_string(), value: format_num(stats.iron_mined) }
            StatRow { label: "Заработано золота".to_string(), value: format_num(stats.gold_earned) }
            StatRow { label: "Пройдено циклов".to_string(), value: format_num(snapshot.cycles as f64) }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Достижения ({done_count}/{total})" }
            for achievement in achievements {
                AchievementRow { key: "{achievement.name}", achievement }
            }
        }
    }
}

#[component]
fn AchievementRow(achievement: AchievementStatus) -> Element {
    let fill_style = format!("width: {:.1}%", achievement.progress * 100.0);
    rsx! {
        div { class: if achievement.done { "ach done" } else { "ach" },
            div { class: "ach-head",
                span { "{achievement.name}" }
                span { class: "ach-value", "{achievement.value_text}" }
            }
            div { class: "ach-desc", "{achievement.description}" }
            div { class: "ach-track",
                div { class: "ach-fill", style: "{fill_style}" }
            }
        }
    }
}

#[component]
fn StatRow(label: String, value: String) -> Element {
    rsx! {
        div { class: "stat-row",
            span { class: "stat-label", "{label}" }
            span { class: "stat-value", "{value}" }
        }
    }
}

#[component]
fn SettingsScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let reset_armed = use_signal(|| false);
    let theme = view.theme;

    rsx! {
        section { class: "settings",
            h2 { class: "settings-title", "Внешний вид" }
            div { class: "theme-list",
                ThemeOption {
                    name: "Классический".to_string(),
                    color: "#004487".to_string(),
                    active: theme == Theme::Classic,
                    on_pick: move |_| {
                        {
                            let mut guard = state.write();
                            guard.theme = Theme::Classic;
                        }
                        refresh_view(state);
                    },
                }
                ThemeOption {
                    name: "Тёмный".to_string(),
                    color: "#444444".to_string(),
                    active: theme == Theme::Dark,
                    on_pick: move |_| {
                        {
                            let mut guard = state.write();
                            guard.theme = Theme::Dark;
                        }
                        refresh_view(state);
                    },
                }
            }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Прогресс" }
            p { class: "settings-note", "Игра сохраняется автоматически каждые 2 секунды." }
            button {
                class: if reset_armed() { "danger-btn armed" } else { "danger-btn" },
                onclick: move |_| reset_progress(state, reset_armed),
                if reset_armed() {
                    "Нажми ещё раз, чтобы сбросить"
                } else {
                    "Сбросить прогресс"
                }
            }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Об игре" }
            p { class: "settings-note", "«Поле» — версия v0.5. Rust + Dioxus 0.7." }
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
fn TopBar(
    title: String,
    bell_open: bool,
    trader_open: bool,
    on_bell: EventHandler<()>,
    on_trader: EventHandler<()>,
    on_dev: EventHandler<()>,
) -> Element {
    let bell_class = if bell_open {
        "icon-btn bell-btn active"
    } else {
        "icon-btn bell-btn"
    };
    let trader_class = if trader_open {
        "icon-btn trader-btn active"
    } else {
        "icon-btn trader-btn"
    };

    rsx! {
        header { class: "topbar",
            button {
                class: "{trader_class}",
                aria_label: "Торговец",
                title: "Торговец",
                onclick: move |_| on_trader.call(()),
                span { class: "trader-icon", "🛒" }
            }
            h1 { class: "topbar-title", "{title}" }
            button {
                class: "{bell_class}",
                aria_label: "Текущая цель",
                onclick: move |_| on_bell.call(()),
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
fn GoalPanel(view: GameState, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "notif-overlay",
            onclick: move |_| on_close.call(()),
            div {
                class: "notif-panel",
                onclick: move |event| event.stop_propagation(),
                div { class: "notif-head",
                    h2 { class: "notif-title", "Текущая цель" }
                    button {
                        class: "dev-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div { class: "goal-panel-body",
                    CurrentGoal { state: view }
                }
            }
        }
    }
}

#[component]
fn DevPanel(state: Signal<GameState>, on_close: EventHandler<()>) -> Element {
    let mut state = state;
    // Читаем через peek, чтобы тик не перерисовывал панель.
    let snapshot = state.peek().clone();
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
                        label: "Шахта, уровень".to_string(),
                        value: snapshot.mine_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().mine_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Кузница, уровень".to_string(),
                        value: snapshot.forge_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().forge_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Банк, уровень".to_string(),
                        value: snapshot.bank_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().bank_level = value.max(0.0) as u32;
                        },
                    }
                    DevNumberRow {
                        label: "Огонь кузницы, с".to_string(),
                        value: snapshot.forge_fire,
                        step: "5".to_string(),
                        on_change: move |value: f64| {
                            let max = state.peek().fire_max();
                            state.write().forge_fire = value.clamp(0.0, max);
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
                        label: "Работников: шахта".to_string(),
                        value: snapshot.mine_workers as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().mine_workers = value.max(0.0) as u32;
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
                    button {
                        class: "dev-btn dev-wide",
                        onclick: move |_| {
                            let mut guard = state.write();
                            guard.coal_pit_researched = true;
                            guard.coal_pit_level = guard.coal_pit_level.max(1);
                            guard.forge_researched = !guard.forge_researched;
                            if guard.forge_researched {
                                guard.forge_level = guard.forge_level.max(1);
                            }
                        },
                        if snapshot.forge_researched {
                            "Сбросить изучение кузницы"
                        } else {
                            "Изучить кузницу сразу"
                        }
                    }
                    button {
                        class: "dev-btn dev-wide",
                        onclick: move |_| {
                            let mut guard = state.write();
                            guard.coal_pit_researched = true;
                            guard.forge_researched = true;
                            guard.trader_researched = !guard.trader_researched;
                        },
                        if snapshot.trader_researched {
                            "Сбросить изучение торговца"
                        } else {
                            "Изучить торговца сразу"
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
                    DevNumberRow {
                        label: "Время".to_string(),
                        value: snapshot.time,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().time = value.clamp(0.0, balance::TIME_CAP);
                        },
                    }
                    DevNumberRow {
                        label: "Циклов пройдено".to_string(),
                        value: snapshot.cycles as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().cycles = value.max(0.0) as u32;
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
fn DevNumberRow(label: String, value: f64, step: String, on_change: EventHandler<f64>) -> Element {
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
fn CostLabel(cost: balance::Cost) -> Element {
    rsx! {
        span { class: "cost",
            img { class: "cost-img", src: WOOD_ICON, alt: "дерево" }
            span { class: "cost-num", "{format_num(cost.wood)}" }
            img { class: "cost-img", src: STONE_ICON, alt: "камень" }
            span { class: "cost-num", "{format_num(cost.stone)}" }
        }
    }
}

#[component]
fn BuildRow(
    title: String,
    level: u32,
    description: String,
    cost: balance::Cost,
    affordable: bool,
    on_build: EventHandler<()>,
) -> Element {
    let mut open = use_signal(|| false);
    let is_open = open();
    let chev_class = if is_open { "chev open" } else { "chev" };
    let button_label = if level == 0 {
        title
    } else {
        format!("{title} ({level})")
    };

    rsx! {
        div { class: "build",
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
                div { class: "build-info",
                    div { class: "build-desc", "{description}" }
                    div { class: "build-cost-line",
                        span { class: "build-cost-label", "Стоимость" }
                        span {
                            class: if affordable { "build-cost" } else { "build-cost poor" },
                            CostLabel { cost }
                        }
                    }
                }
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
            None,
            Some((width, count)),
        )
    } else if state.house_level == 0 {
        (
            "Построй свой дом — он даст энергию и усилит отдых.".to_string(),
            Some(balance::house_cost(0)),
            None,
        )
    } else if state.worker_house_level == 0 {
        (
            "Построй дом для работников, чтобы получить первого работника.".to_string(),
            Some(balance::worker_house_cost(0)),
            None,
        )
    } else if state.research_house_level == 0 {
        (
            "Построй дом для изучения, чтобы начать исследования.".to_string(),
            Some(balance::research_house_cost(0)),
            None,
        )
    } else if state.sawmill_level == 0 {
        (
            "Построй лесопилку и назначь на неё работника.".to_string(),
            Some(balance::sawmill_cost(0)),
            None,
        )
    } else if state.sawmill_workers == 0 {
        (
            "Назначь работника на лесопилку во вкладке «Труд».".to_string(),
            None,
            None,
        )
    } else if !state.coal_pit_researched {
        (
            "Назначь исследователя на угольную яму во вкладке «Изучения».".to_string(),
            None,
            None,
        )
    } else if state.wood >= state.cap(ResourceKind::Wood)
        || state.stone >= state.cap(ResourceKind::Stone)
    {
        (
            "Склад полон. Улучши склад, чтобы добывать дальше.".to_string(),
            Some(balance::storage_cost(state.storage_level)),
            None,
        )
    } else {
        (
            "Развивай дом: больше энергии за отдых и в секунду.".to_string(),
            Some(balance::house_cost(state.house_level)),
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
                    if let Some(cost) = goal_hint {
                        div { class: "goal-cost", CostLabel { cost } }
                    }
                },
            }
        }
    }
}

#[component]
fn ResourceBar(view: GameState) -> Element {
    let state = view;
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
    if state.iron > 0.0 || state.forge_researched || state.mine_level > 0 {
        items.push((
            ResourceKind::Iron,
            "Железо",
            state.iron,
            state.cap(ResourceKind::Iron),
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
    if state.gold > 0.0 || state.trader_researched {
        items.push((
            ResourceKind::Gold,
            "Золото",
            state.gold,
            state.cap(ResourceKind::Gold),
        ));
    }

    rsx! {
        div { class: "resources",
            if items.is_empty() && state.time <= 0.0 {
                span { class: "resources-empty", "Ресурсов пока нет — отдохни и начни добывать" }
            } else {
                for (kind, name, value, cap) in items {
                    div { class: "resource", key: "{name}",
                        if let Some(src) = resource_icon(kind) {
                            img {
                                class: if kind == ResourceKind::Coal { "resource-img dark" } else { "resource-img" },
                                src,
                                alt: "{name}",
                            }
                        } else {
                            span { class: "resource-icon", "💰" }
                        }
                        span { class: "resource-name", "{name}" }
                        span {
                            class: if value >= cap { "resource-value full" } else { "resource-value" },
                            "{format_num(value)} / {format_num(cap)}"
                        }
                    }
                }
                if state.time > 0.0 {
                    div { class: "resource",
                        span { class: "resource-icon", "⏳" }
                        span { class: "resource-name", "Время" }
                        span { class: "resource-value", "{format_num(state.time)}" }
                    }
                }
            }
        }
    }
}

#[component]
fn TabBar(active: Signal<usize>) -> Element {
    // На десктопе колесо мыши над вкладками крутит их по горизонтали.
    use_effect(move || {
        spawn(async move {
            let script = r#"
                (() => {
                    const nav = document.querySelector('.tabs');
                    if (!nav || nav.dataset.wheelBound) return true;
                    nav.dataset.wheelBound = '1';
                    nav.addEventListener('wheel', (event) => {
                        if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
                        if (nav.scrollWidth <= nav.clientWidth) return;
                        nav.scrollLeft += event.deltaY;
                        event.preventDefault();
                    }, { passive: false });
                    return true;
                })();
            "#;
            if let Err(error) = document::eval(script).await {
                eprintln!("Не удалось включить прокрутку вкладок колесом: {error}");
            }
        });
    });

    rsx! {
        nav { class: "tabs",
            for (i, (icon, label)) in TABS.iter().enumerate() {
                button {
                    key: "{i}",
                    class: if active() == i { "tab active" } else { "tab" },
                    onclick: move |_| active.set(i),
                    if let Some(src) = tab_image(i) {
                        img {
                            class: "tab-img",
                            src,
                            alt: "{label}",
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
fn EnergyBar(view: GameState) -> Element {
    let state = view;
    let fill_style = format!("width: {:.1}%", state.energy_ratio() * 100.0);
    let label = format!(
        "{} / {} (+{:.2}/с)",
        format_num(state.energy),
        format_num(state.max_energy()),
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

/// Немедленно обновляет снимок UI после действия игрока.
pub(crate) fn refresh_view(state: Signal<GameState>) {
    *VIEW.write() = state.peek().clone();
}

fn do_rest(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::rest(&mut guard);
    }
    refresh_view(state);
}

fn do_gather(mut state: Signal<GameState>, kind: ResourceKind) {
    {
        let mut guard = state.write();
        actions::gather(&mut guard, kind);
    }
    refresh_view(state);
}

fn do_build_with(mut state: Signal<GameState>, action: fn(&mut GameState) -> BuildResult) {
    {
        let mut guard = state.write();
        action(&mut guard);
    }
    refresh_view(state);
}

fn do_build_house(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::build_house(&mut guard);
    }
    refresh_view(state);
}

fn do_build_storage(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::build_storage(&mut guard);
    }
    refresh_view(state);
}

fn do_build_comfort(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::build_comfort(&mut guard);
    }
    refresh_view(state);
}

fn do_forge_feed(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::add_coal_to_fire(&mut guard);
    }
    refresh_view(state);
}

fn do_time_warp(mut state: Signal<GameState>) {
    {
        let mut guard = state.write();
        actions::time_warp(&mut guard);
    }
    refresh_view(state);
}

fn do_prestige(mut state: Signal<GameState>, mut armed: Signal<bool>) {
    {
        let mut guard = state.write();
        actions::prestige(&mut guard);
    }
    armed.set(false);
    refresh_view(state);
}

fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours} ч {minutes} мин")
    } else if minutes > 0 {
        format!("{minutes} мин {secs} с")
    } else {
        format!("{secs} с")
    }
}

fn reset_progress(mut state: Signal<GameState>, mut reset_armed: Signal<bool>) {
    if *reset_armed.peek() {
        let theme = state.peek().theme;
        let fresh = GameState {
            theme,
            ..GameState::default()
        };
        state.set(fresh);
        reset_armed.set(false);
        refresh_view(state);

        // Сначала стираем файл, потом ставим свежее состояние: иначе автосейв
        // мог бы успеть записать старый прогресс обратно.
        spawn(async move {
            if let Err(error) = save::clear().await {
                eprintln!("Не удалось удалить сохранение: {error}");
            }
            let mut snapshot = state.peek().clone();
            snapshot.last_seen = actions::unix_now();
            if let Err(error) = save::save(&snapshot).await {
                eprintln!("Не удалось сохранить сброс: {error}");
            }
        });
    } else {
        reset_armed.set(true);

        spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if *reset_armed.peek() {
                reset_armed.set(false);
            }
        });
    }
}

fn theme_css_class(theme: Theme) -> &'static str {
    match theme {
        Theme::Classic => "theme-classic",
        Theme::Dark => "theme-dark",
    }
}

pub(crate) fn format_num(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
