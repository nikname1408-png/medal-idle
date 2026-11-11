use std::time::Duration;

use dioxus::prelude::*;

use crate::game::actions::{self, BuildResult};
use crate::game::expedition;
use crate::game::state::{AchievementStatus, ResourceKind, Theme};
use crate::game::{balance, GameState};

use super::expedition::ExpeditionScreen;
use super::inventory::InventoryScreen;
use super::save;
use super::trader::TradeScreen;

/// Снимок состояния для UI: обновляется после каждого действия и раз в тик.
static VIEW: GlobalSignal<GameState> = Signal::global(GameState::default);

const MAIN_CSS: Asset = asset!("/assets/main.css");

const ICON_BAG: Asset = asset!("/assets/icons/bag.png");
const ICON_COAL: Asset = asset!("/assets/icons/coal.png");
const ICON_FIELD: Asset = asset!("/assets/icons/field.png");
const ICON_GEAR: Asset = asset!("/assets/icons/gear.png");
const ICON_GOLD: Asset = asset!("/assets/icons/gold.png");
const ICON_SCROLL: Asset = asset!("/assets/icons/scroll.png");
const ICON_STONE: Asset = asset!("/assets/icons/stone.png");
const ICON_WOOD: Asset = asset!("/assets/icons/wood.png");

const TAB_FIELD: usize = 0;
const TAB_WORK: usize = 1;
const TAB_BAG: usize = 2;
const TAB_EXPEDITION: usize = 3;
const TAB_CYCLE: usize = 4;
const TAB_STATS: usize = 5;
const TAB_SETTINGS: usize = 6;

const TABS: [(&str, &str); 7] = [
    ("🌾", "Поле"),
    ("👷", "Труд"),
    ("🎒", "Сумка"),
    ("🧭", "Походы"),
    ("🔄", "Цикл"),
    ("📊", "Статы"),
    ("⚙️", "Настр."),
];

/// Текстовая заглушка иконки ресурса.
#[allow(dead_code)]
fn resource_text_icon(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Wood => "🪵",
        ResourceKind::Stone => "🪨",
        ResourceKind::Coal => "⚫",
        ResourceKind::Fire => "🔥",
        ResourceKind::Gold => "🪙",
    }
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(GameState::default);
    let mut active_tab = use_signal(|| TAB_FIELD);
    let mut bell_open = use_signal(|| false);
    let mut trader_open = use_signal(|| false);
    let mut menu_open = use_signal(|| false);
    let mut loaded = use_signal(|| false);
    let mut dev_open = use_signal(|| false);
    let mut back = use_signal(|| 0u32);
    let mut menu_back = use_signal(|| *back.peek());

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
                expedition::tick_active_expedition(&mut guard, 1.0);
                let mut snapshot = guard.clone();
                // Панель разработчика показывает живые числа без сброса ввода.
                if *dev_open.peek() {
                    snapshot.dev = guard.dev.clone();
                }
                snapshot
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

    // Назад закрывает бургер-меню, если оно открыто.
    use_effect(move || {
        let count = back();
        if count <= *menu_back.peek() {
            return;
        }
        menu_back.set(count);
        if menu_open() {
            menu_open.set(false);
        }
    });

    // Экраны перерисовываются по снимку `VIEW`; сами действия пишут в `state`.
    let snapshot = VIEW();
    let theme_class = theme_css_class(snapshot.theme);
    let title = match active_tab() {
        TAB_SETTINGS => "Настройки",
        TAB_WORK => "Труд",
        TAB_BAG => "Инвентарь",
        TAB_EXPEDITION => "Походы",
        TAB_CYCLE => "Цикл",
        TAB_STATS => "Статы",
        _ => "Поле",
    };

    let screen = if trader_open() || menu_open() {
        rsx! {}
    } else {
        match active_tab() {
            TAB_FIELD => rsx! {
                FieldScreen { state, view: snapshot.clone() }
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
                }
            },
            TAB_EXPEDITION => rsx! {
                ExpeditionScreen { state, view: snapshot.clone() }
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
                menu_open: menu_open(),
                unread: snapshot.unread_letters(),
                on_menu: move |_| menu_open.set(!menu_open()),
                on_bell: move |_| bell_open.set(!bell_open()),
            }
            main { class: "content", {screen} }
            div { class: "bottom",
                ResourceBar { view: snapshot.clone() }
                TabBar { active: active_tab }
                EnergyBar { view: snapshot.clone() }
            }
            if bell_open() {
                BellPanel {
                    view: snapshot.clone(),
                    on_close: move |_| bell_open.set(false),
                    on_open_letter: move |letter_id: String| {
                        state.write().open_letter(&letter_id);
                        refresh_view(state);
                        bell_open.set(false);
                    },
                }
            }
            if menu_open() {
                BurgerMenu {
                    on_close: move |_| menu_open.set(false),
                    on_trader: move |_| {
                        menu_open.set(false);
                        trader_open.set(true);
                    },
                    on_settings: move |_| {
                        menu_open.set(false);
                        active_tab.set(TAB_SETTINGS);
                    },
                    on_dev: move |_| {
                        menu_open.set(false);
                        dev_open.set(true);
                    },
                }
            }
            if trader_open() {
                TradeScreen {
                    state,
                    view: snapshot.clone(),
                    back,
                    on_close: move |_| trader_open.set(false),
                }
            }
            if let Some(offer_id) = snapshot.pending_offer.clone() {
                if !trader_open() && !menu_open() {
                    OfferModal { state, view: snapshot.clone(), offer_id }
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
    let sawmill_cost = balance::sawmill_cost(snapshot.sawmill_level);
    let mine_cost = balance::mine_cost(snapshot.mine_level);
    let coal_pit_cost = balance::coal_pit_cost(snapshot.coal_pit_level + 1);
    let forge_cost = balance::forge_cost(snapshot.forge_level + 1);
    let bank_cost = balance::bank_cost(snapshot.bank_level + 1);
    let house_cost = balance::house_cost(snapshot.house_level);
    let comfort_cost = balance::comfort_cost(snapshot.comfort_level);
    let barn_cost = balance::barn_cost(snapshot.barn_level);

    let affordable = |cost: balance::Cost| actions::can_afford(&snapshot, cost);
    let storage_affordable = affordable(storage_cost);
    let worker_house_affordable = affordable(worker_house_cost);
    let sawmill_affordable = affordable(sawmill_cost);
    let mine_affordable = affordable(mine_cost);
    let coal_pit_affordable = affordable(coal_pit_cost);
    let forge_affordable = affordable(forge_cost);
    let bank_affordable = affordable(bank_cost);
    let house_affordable = affordable(house_cost);
    let comfort_affordable = affordable(comfort_cost);
    let barn_affordable = affordable(barn_cost);

    let gather_yield =
        (balance::CLICK_YIELD + snapshot.dev.gather_yield) * (1.0 + snapshot.gather_bonus as f64);
    let rest_available = snapshot.energy < snapshot.max_energy();
    let wood_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && (snapshot.active_amount("wood") as f64) < snapshot.cap(ResourceKind::Wood);
    let stone_available = snapshot.energy >= balance::CLICK_ENERGY_COST
        && (snapshot.active_amount("stone") as f64) < snapshot.cap(ResourceKind::Stone);

    let fire_max = snapshot.fire_max();
    let fire_ratio = if fire_max > 0 {
        (snapshot.current_fire as f64 / fire_max as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fire_fill = format!("width: {:.1}%", fire_ratio * 100.0);
    let can_feed =
        snapshot.active_amount("coal") >= 1 && (snapshot.current_fire as f64) < fire_max as f64;
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

    let barn_description = if snapshot.barn_level == 0 {
        format!(
            "Амбар даёт +{} к лимиту склада за уровень. Открывается на {} уровне склада.",
            format_num(balance::BARN_CAP_PER_LEVEL),
            balance::BARN_UNLOCK_STORAGE_LEVEL
        )
    } else {
        format!(
            "Сейчас амбар даёт +{}. Улучшение добавит ещё +{}.",
            format_num(snapshot.barn_level as f64 * balance::BARN_CAP_PER_LEVEL),
            format_num(balance::BARN_CAP_PER_LEVEL)
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
        "Работники добывают камень: +{}/с с каждого. Железо — только из походов и у торговца. Уровень даёт одно рабочее место.",
        format_num(balance::MINE_STONE_PER_WORKER_PER_SEC)
    );

    let forge_description = format!(
        "Огонь из угля нужен для крафта предметов. Один уголь — {} с огня. Уровень повышает запас огня.",
        format_num(balance::FIRE_SECONDS_PER_COAL as f64)
    );

    let bank_description = format!(
        "Сейчас лимит золота: {}. Улучшение добавит +{}.",
        format_num(snapshot.gold_cap()),
        format_num(balance::GOLD_CAP_PER_LEVEL)
    );

    let house_description = if snapshot.house_level == 0 {
        format!(
            "После постройки: +{} энергии за отдых и +{}/с.",
            format_num(snapshot.rest_energy() + balance::REST_ENERGY_PER_HOUSE_LEVEL),
            format_num(balance::HOUSE_ENERGY_PER_LEVEL)
        )
    } else {
        format!(
            "Сейчас: +{} за отдых, +{}/с. Улучшение даст ещё +{} за отдых и +{}/с.",
            format_num(snapshot.rest_energy()),
            format_num(snapshot.energy_regen()),
            format_num(balance::REST_ENERGY_PER_HOUSE_LEVEL),
            format_num(balance::HOUSE_ENERGY_PER_LEVEL)
        )
    };

    let comfort_description = format!(
        "Сейчас максимум энергии: {}. Улучшение добавит +{}.",
        format_num(snapshot.max_energy()),
        format_num(balance::ENERGY_CAP_PER_COMFORT)
    );

    rsx! {
        CollapsibleSection { title: "Отдых".to_string(),
            ActionRow {
                label: "Отдых".to_string(),
                info: rest_info,
                available: rest_available,
                on_press: move |_| do_rest(state),
            }
        }

        CollapsibleSection { title: "Энергия".to_string(),
            BuildRow {
                title: "Дом".to_string(),
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
                if snapshot.barn_unlocked() {
                    BuildRow {
                        title: "Амбар".to_string(),
                        level: snapshot.barn_level,
                        description: barn_description,
                        cost: barn_cost,
                        affordable: barn_affordable,
                        on_build: move |_| do_build_with(state, actions::build_barn),
                    }
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
            }
        }

        if snapshot.house_level > 0 {
            CollapsibleSection { title: "Постройки".to_string(),
                if snapshot.house_level > 0 {
                    BuildRow {
                        title: "Лесопилка".to_string(),
                        level: snapshot.sawmill_level,
                        description: sawmill_description,
                        cost: sawmill_cost,
                        affordable: sawmill_affordable,
                        on_build: move |_| do_build_with(state, actions::build_sawmill),
                    }
                }
                if snapshot.coal_pit_unlocked {
                    BuildRow {
                        title: "Угольная яма".to_string(),
                        level: snapshot.coal_pit_level,
                        description: coal_pit_description,
                        cost: coal_pit_cost,
                        affordable: coal_pit_affordable,
                        on_build: move |_| do_build_with(state, actions::build_coal_pit),
                    }
                }
                if snapshot.coal_pit_level > 0 {
                    BuildRow {
                        title: "Шахта".to_string(),
                        level: snapshot.mine_level,
                        description: mine_description,
                        cost: mine_cost,
                        affordable: mine_affordable,
                        on_build: move |_| do_build_with(state, actions::build_mine),
                    }
                }
            }
        }

        if snapshot.forge_unlocked {
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
                                "{format_num(snapshot.current_fire as f64)} / {format_num(fire_max as f64)} с"
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
                                "Подбросить уголь (+{snapshot.fire_per_coal()} с)"
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
                            "Крафт предметов тратит огонь. Сейчас: {format_num(snapshot.current_fire as f64)} с."
                        }
                    }
                }
            }
        }

        if snapshot.bank_unlocked {
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
                    div { class: "time-note",
                        "Время копится, пока игра закрыта. Тратится на тайм-варп."
                    }
                }
                button {
                    class: if time_available { "forge-btn" } else { "forge-btn disabled" },
                    disabled: !time_available,
                    onclick: move |_| do_time_warp(state),
                    "{time_warp_label} ▸"
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
    let mine_rate = snapshot.mine_stone_per_sec();
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
                        detail: format!("+{}/с камня", format_num(mine_rate)),
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
                        detail: format!(
                            "+{}/с угля · −{}/с дерева",
                            format_num(coal_rate),
                            format_num(coal_wood),
                        ),
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
                        "Угольная яма не открыта. Построй свой дом — придёт предложение."
                    }
                }
            }
        }

        div { class: "count-note", "Работники: {free} / {slots}" }
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
    let multiplier = snapshot.income_multiplier();
    let next_multiplier = 1.0 + (cycles + 1) as f64 * balance::CYCLE_INCOME_PER_LEVEL;
    let threshold = snapshot.cycle_threshold();
    let gathered = snapshot.cycle_gathered();
    let progress = snapshot.cycle_progress();
    let progress_fill = format!("width: {:.1}%", progress * 100.0);
    let threshold_met = gathered + 1e-9 >= threshold;

    let wall_mine_active = cycles >= balance::CYCLE_WALL_LEVEL;
    let wall_mine_met = snapshot.cycle_wall_mine_met();
    let wall_stone_active = cycles >= balance::CYCLE_WALL_STONE_LEVEL;
    let wall_stone_met = snapshot.cycle_wall_stone_met();
    let stone_need = balance::CYCLE_WALL_EXTRA_STONE;
    let stone_have = snapshot.cycle_stone;

    let ready = threshold_met && wall_mine_met && wall_stone_met;
    let requirement = if !threshold_met {
        format!(
            "Нужно добыть {} дерева и камня за цикл (сейчас {}).",
            format_num(threshold),
            format_num(gathered)
        )
    } else if !wall_mine_met {
        format!(
            "Стена R={}: построй шахту до уровня {}.",
            balance::CYCLE_WALL_LEVEL,
            balance::CYCLE_WALL_MINE_LEVEL
        )
    } else if !wall_stone_met {
        format!(
            "Стена R={}: нужно {} камня за цикл сверх порога.",
            balance::CYCLE_WALL_STONE_LEVEL,
            format_num(balance::CYCLE_WALL_EXTRA_STONE)
        )
    } else {
        "Всё готово для нового цикла.".to_string()
    };

    rsx! {
        section { class: "cycle-hero",
            div { class: "cycle-count", "{cycles}" }
            div { class: "cycle-label", "циклов пройдено" }
            div { class: "cycle-bonus", "Множитель дохода: ×{multiplier:.2}" }
        }

        section { class: "settings",
            h2 { class: "settings-title", "Новый цикл" }
            p { class: "settings-note",
                "Сбросит ресурсы, постройки, работников и сумку. Останутся циклы, время в игре, статистика и достижения."
            }

            div { class: "cycle-progress-head",
                span { "Добыто за цикл" }
                span { class: "cycle-progress-value", "{format_num(gathered)} / {format_num(threshold)}" }
            }
            div { class: "cycle-progress-track",
                div { class: "cycle-progress-fill", style: "{progress_fill}" }
            }

            div { class: "cycle-forecast",
                div { class: "cycle-forecast-item",
                    span { class: "cycle-forecast-label", "Сейчас" }
                    span { class: "cycle-forecast-value", "×{multiplier:.2}" }
                }
                div { class: "cycle-forecast-item",
                    span { class: "cycle-forecast-label", "После цикла" }
                    span { class: "cycle-forecast-value", "×{next_multiplier:.2}" }
                }
                div { class: "cycle-forecast-item",
                    span { class: "cycle-forecast-label", "При 2× порога" }
                    span { class: "cycle-forecast-value", "×{next_multiplier:.2}" }
                }
            }
            p { class: "settings-note",
                "Множитель зависит только от номера цикла: перебор сверх порога X не увеличивает. «Отдых» и «Труд» умножаются на X, ручная добыча — нет."
            }

            if wall_mine_active {
                div { class: if wall_mine_met { "cycle-wall done" } else { "cycle-wall" },
                    span { class: "cycle-wall-mark",
                        if wall_mine_met {
                            "✓"
                        } else {
                            "•"
                        }
                    }
                    span { class: "cycle-wall-text",
                        "Стена R={balance::CYCLE_WALL_LEVEL}: шахта до уровня {balance::CYCLE_WALL_MINE_LEVEL} (сейчас {snapshot.mine_level})"
                    }
                }
            }
            if wall_stone_active {
                div { class: if wall_stone_met { "cycle-wall done" } else { "cycle-wall" },
                    span { class: "cycle-wall-mark",
                        if wall_stone_met {
                            "✓"
                        } else {
                            "•"
                        }
                    }
                    span { class: "cycle-wall-text",
                        "Стена R={balance::CYCLE_WALL_STONE_LEVEL}: {format_num(stone_have)} / {format_num(stone_need)} камня за цикл"
                    }
                }
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
    let time_played = snapshot.total_play_seconds.max(stats.play_seconds);
    let achievements = snapshot.achievements();
    let done_count = achievements
        .iter()
        .filter(|achievement| achievement.done)
        .count();
    let total = achievements.len();

    rsx! {
        section { class: "settings",
            h2 { class: "settings-title", "Статистика" }
            StatRow {
                label: "Время в игре".to_string(),
                value: format_duration(time_played),
            }
            StatRow {
                label: "Кликов добычи".to_string(),
                value: format_num(stats.clicks as f64),
            }
            StatRow {
                label: "Отдыхов".to_string(),
                value: format_num(stats.rests as f64),
            }
            StatRow {
                label: "Добыто дерева".to_string(),
                value: format_num(stats.wood_gathered),
            }
            StatRow {
                label: "Добыто камня".to_string(),
                value: format_num(stats.stone_gathered),
            }
            StatRow {
                label: "Получено угля".to_string(),
                value: format_num(stats.coal_produced),
            }
            StatRow {
                label: "Добыто железной руды".to_string(),
                value: format_num(stats.iron_mined),
            }
            StatRow {
                label: "Заработано золота".to_string(),
                value: format_num(stats.gold_earned),
            }
            StatRow {
                label: "Пройдено циклов".to_string(),
                value: format_num(snapshot.cycles as f64),
            }
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
            p { class: "settings-note",
                "Игра сохраняется автоматически каждые 2 секунды."
            }
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
            p { class: "placeholder-text",
                "Этот раздел появится в следующих версиях."
            }
        }
    }
}

#[component]
fn TopBar(
    title: String,
    bell_open: bool,
    menu_open: bool,
    unread: u32,
    on_menu: EventHandler<()>,
    on_bell: EventHandler<()>,
) -> Element {
    let bell_class = if bell_open {
        "icon-btn bell-btn active"
    } else {
        "icon-btn bell-btn"
    };
    let menu_class = if menu_open {
        "icon-btn menu-btn active"
    } else {
        "icon-btn menu-btn"
    };
    let bell_label = if unread > 0 {
        format!("Почта: {unread} новых")
    } else {
        "Текущая цель и письма".to_string()
    };

    rsx! {
        header { class: "topbar",
            button {
                class: "{menu_class}",
                aria_label: "Меню",
                title: "Меню",
                onclick: move |_| on_menu.call(()),
                span { class: "burger" }
            }
            h1 { class: "topbar-title", "{title}" }
            button {
                class: "{bell_class}",
                aria_label: "{bell_label}",
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
                if unread > 0 {
                    span { class: "bell-badge", "{unread}" }
                }
            }
        }
    }
}

#[component]
fn BurgerMenu(
    on_close: EventHandler<()>,
    on_trader: EventHandler<()>,
    on_settings: EventHandler<()>,
    on_dev: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "menu-overlay", onclick: move |_| on_close.call(()),
            div {
                class: "menu-panel",
                onclick: move |event| event.stop_propagation(),
                div { class: "menu-head",
                    span { class: "menu-title", "Меню" }
                    button {
                        class: "menu-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                button { class: "menu-item", onclick: move |_| on_trader.call(()),
                    img {
                        class: "menu-icon icon",
                        src: "{ICON_SCROLL}",
                        alt: "",
                    }
                    span { "Торговец" }
                }
                button {
                    class: "menu-item",
                    onclick: move |_| on_settings.call(()),
                    span { class: "menu-icon", "⚙" }
                    span { "Настройки" }
                }
                button { class: "menu-item", onclick: move |_| on_dev.call(()),
                    span { class: "menu-icon", "🛠" }
                    span { "Панель разработчика" }
                }
            }
        }
    }
}

#[component]
fn BellPanel(
    view: GameState,
    on_close: EventHandler<()>,
    on_open_letter: EventHandler<String>,
) -> Element {
    let letters = view.letters.clone();
    rsx! {
        div { class: "notif-overlay", onclick: move |_| on_close.call(()),
            div {
                class: "notif-panel",
                onclick: move |event| event.stop_propagation(),
                div { class: "notif-head",
                    h2 { class: "notif-title", "Почта и цель" }
                    button {
                        class: "dev-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                if !letters.is_empty() {
                    div { class: "letters",
                        for letter in letters {
                            button {
                                key: "{letter.id}",
                                class: if letter.read { "letter" } else { "letter unread" },
                                onclick: {
                                    let id = letter.id.clone();
                                    move |_| on_open_letter.call(id.clone())
                                },
                                span { class: "letter-title", "{letter.title}" }
                                span { class: "letter-body", "{letter.body}" }
                            }
                        }
                    }
                }
                div { class: "goal-panel-body",
                    CurrentGoal { state: view }
                }
            }
        }
    }
}

/// Квадратное уведомление о новой постройке.
#[component]
fn OfferModal(state: Signal<GameState>, view: GameState, offer_id: String) -> Element {
    let Some(def) = actions::offer_def(&offer_id) else {
        return rsx! {};
    };
    let affordable = actions::can_afford(&view, def.cost);
    let price_class = if affordable {
        "offer-price"
    } else {
        "offer-price missing"
    };

    rsx! {
        div { class: "offer-overlay",
            div { class: "offer-card",
                div { class: "offer-title", "{def.title}" }
                div { class: "offer-body", "{def.body}" }
                div { class: "offer-actions",
                    button {
                        class: "{price_class}",
                        disabled: !affordable,
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                actions::build_pending_offer(&mut guard);
                            }
                            refresh_view(state);
                        },
                        CostLabel { cost: def.cost }
                    }
                    button {
                        class: "offer-later",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                actions::defer_pending_offer(&mut guard);
                            }
                            refresh_view(state);
                        },
                        "Позже"
                    }
                }
            }
        }
    }
}

#[component]
fn DevPanel(state: Signal<GameState>, on_close: EventHandler<()>) -> Element {
    let mut state = state;
    // Снимок берём через peek и только один раз при открытии панели: тик игры
    // не должен перерисовывать панель и сбрасывать введённые числа.
    let snapshot = use_hook(|| state.peek().clone());
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
                p { class: "dev-note",
                    "Временная панель для тестов. В релизе её не будет."
                }

                section { class: "dev-group",
                    h3 { class: "dev-group-title", "Энергия" }
                    DevNumberRow {
                        label: "Сейчас".to_string(),
                        value: snapshot.energy,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().energy = value.max(0.0);
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
                        label: "Уют, уровень".to_string(),
                        value: snapshot.comfort_level as f64,
                        step: "1".to_string(),
                        on_change: move |value: f64| {
                            state.write().comfort_level = value.max(0.0) as u32;
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
                            let mut guard = state.write();
                            guard.bank_level = value.max(0.0) as u32;
                            guard.gold = guard.gold.min(guard.gold_cap());
                        },
                    }
                    DevNumberRow {
                        label: "Огонь кузницы, с".to_string(),
                        value: snapshot.current_fire as f64,
                        step: "5".to_string(),
                        on_change: move |value: f64| {
                            state.write().current_fire = value.max(0.0) as u32;
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
                                guard.coal_pit_unlocked = true;
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
                            guard.coal_pit_unlocked = true;
                            guard.coal_pit_level = guard.coal_pit_level.max(1);
                            guard.forge_researched = !guard.forge_researched;
                            if guard.forge_researched {
                                guard.forge_unlocked = true;
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
                            guard.coal_pit_unlocked = true;
                            guard.coal_pit_level = guard.coal_pit_level.max(1);
                            guard.forge_researched = true;
                            guard.forge_unlocked = true;
                            guard.forge_level = guard.forge_level.max(1);
                            guard.trader_researched = !guard.trader_researched;
                            if guard.trader_researched {
                                guard.bank_unlocked = true;
                            }
                        },
                        if snapshot.trader_researched {
                            "Сбросить изучение банка"
                        } else {
                            "Изучить банк сразу"
                        }
                    }
                }

                section { class: "dev-group",
                    h3 { class: "dev-group-title", "Ресурсы" }
                    DevResourceRow {
                        icon: "🌲".to_string(),
                        name: "Дерево".to_string(),
                        value: snapshot.active_amount("wood") as f64,
                        speed: dev.wood_per_sec,
                        on_value: move |value: f64| {
                            let mut guard = state.write();
                            guard
                                .active_resources
                                .insert("wood".to_string(), value.max(0.0) as u32);
                            guard.refresh_unlocks();
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.wood_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "🪨".to_string(),
                        name: "Камень".to_string(),
                        value: snapshot.active_amount("stone") as f64,
                        speed: dev.stone_per_sec,
                        on_value: move |value: f64| {
                            let mut guard = state.write();
                            guard
                                .active_resources
                                .insert("stone".to_string(), value.max(0.0) as u32);
                            guard.refresh_unlocks();
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.stone_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "⛏️".to_string(),
                        name: "Железная руда".to_string(),
                        value: snapshot.item_count("iron") as f64,
                        speed: dev.iron_per_sec,
                        on_value: move |value: f64| {
                            let mut guard = state.write();
                            let _ = guard.add_item("iron", value.max(0.0) as u32);
                        },
                        on_speed: move |value: f64| {
                            state.write().dev.iron_per_sec = value.max(0.0);
                        },
                    }
                    DevResourceRow {
                        icon: "🔥".to_string(),
                        name: "Уголь".to_string(),
                        value: snapshot.active_amount("coal") as f64,
                        speed: dev.coal_per_sec,
                        on_value: move |value: f64| {
                            let mut guard = state.write();
                            guard
                                .active_resources
                                .insert("coal".to_string(), value.max(0.0) as u32);
                            guard.refresh_unlocks();
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
                            // То же золото, что видно в баре ресурсов и у торговца.
                            let mut guard = state.write();
                            guard.gold = value.max(0.0);
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

/// Поле ввода числа. Текст живёт в локальном сигнале, поэтому перерисовки
/// игры не стирают то, что набирает пользователь.
#[component]
fn DevNumberRow(label: String, value: f64, step: String, on_change: EventHandler<f64>) -> Element {
    let mut text = use_signal(|| format_input(value));
    let draft = text();
    let field_class = if draft.trim() == format_input(value) {
        "dev-input"
    } else {
        "dev-input dirty"
    };
    let shown = draft.clone();

    rsx! {
        div { class: "dev-row",
            span { class: "dev-label", "{label}" }
            input {
                class: "{field_class}",
                r#type: "number",
                step: "{step}",
                value: "{shown}",
                oninput: move |event| text.set(event.value()),
                onchange: move |event| {
                    if let Ok(parsed) = event.value().parse::<f64>() {
                        on_change.call(parsed);
                        text.set(format_input(parsed));
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
    // Локальные черновики полей не зависят от тика игры.
    let mut amount_text = use_signal(|| format_input(value));
    let mut speed_text = use_signal(|| format_input(speed));
    let amount = amount_text();
    let speed_draft = speed_text();
    let current_amount = format_input(value);
    let current_speed = format_input(speed);
    let amount_class = if amount.trim() == current_amount {
        "dev-input"
    } else {
        "dev-input dirty"
    };
    let speed_class = if speed_draft.trim() == current_speed {
        "dev-input"
    } else {
        "dev-input dirty"
    };
    let amount_shown = amount.clone();
    let speed_shown = speed_draft.clone();

    rsx! {
        div { class: "dev-resource",
            div { class: "dev-resource-head",
                span { class: "dev-resource-name", "{icon} {name}" }
                span { class: "dev-resource-value", "{format_num(value)}" }
            }
            div { class: "dev-row",
                span { class: "dev-label", "Количество" }
                input {
                    class: "{amount_class}",
                    r#type: "number",
                    step: "1",
                    value: "{amount_shown}",
                    oninput: move |event| amount_text.set(event.value()),
                    onchange: move |event| {
                        if let Ok(parsed) = event.value().parse::<f64>() {
                            on_value.call(parsed);
                            amount_text.set(format_input(parsed));
                        }
                    },
                }
                button {
                    class: "dev-btn",
                    onclick: move |_| on_value.call(value + 10.0),
                    "+10"
                }
                button {
                    class: "dev-btn",
                    onclick: move |_| on_value.call(value + 100.0),
                    "+100"
                }
                button { class: "dev-btn", onclick: move |_| on_value.call(0.0), "0" }
            }
            div { class: "dev-row",
                span { class: "dev-label", "В секунду" }
                input {
                    class: "{speed_class}",
                    r#type: "number",
                    step: "0.1",
                    value: "{speed_shown}",
                    oninput: move |event| speed_text.set(event.value()),
                    onchange: move |event| {
                        if let Ok(parsed) = event.value().parse::<f64>() {
                            on_speed.call(parsed);
                            speed_text.set(format_input(parsed));
                        }
                    },
                }
            }
        }
    }
}

/// Число для поля ввода без лишних нулей.
fn format_input(value: f64) -> String {
    if (value.fract()).abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

#[component]
fn CollapsibleSection(title: String, children: Element) -> Element {
    let mut open = use_signal(|| true);
    let is_open = open();
    let chev_class = if is_open { "chev open" } else { "chev" };

    rsx! {
        section { class: "section",
            div { class: "section-head",
                span { class: "section-title", "{title}" }
                button {
                    class: "chev-btn",
                    aria_label: "Свернуть или развернуть раздел",
                    onclick: move |_| open.set(!open()),
                    span { class: chev_class }
                }
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
            if cost.wood > 0.0 {
                span { class: "cost-icon", "🪵" }
                span { class: "cost-num", "{format_num(cost.wood)}" }
            }
            if cost.stone > 0.0 {
                span { class: "cost-icon", "🪨" }
                span { class: "cost-num", "{format_num(cost.stone)}" }
            }
            if cost.gold > 0.0 {
                span { class: "cost-icon", "💰" }
                span { class: "cost-num", "{format_num(cost.gold)}" }
            }
            if cost.energy > 0.0 {
                span { class: "cost-icon", "⚡" }
                span { class: "cost-num", "{format_num(cost.energy)}" }
            }
            if cost.boards > 0 {
                span { class: "cost-icon", "🪚" }
                span { class: "cost-num", "{cost.boards}" }
            }
            if cost.iron_ingots > 0 {
                span { class: "cost-icon", "🔩" }
                span { class: "cost-num", "{cost.iron_ingots}" }
            }
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
                        span { class: if affordable { "build-cost" } else { "build-cost poor" },
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
    } else if !state.coal_pit_unlocked {
        (
            "Открой угольную яму — письмо в колокольчике.".to_string(),
            Some(balance::coal_pit_cost(0)),
            None,
        )
    } else if state.coal_pit_level == 0 {
        (
            "Построй угольную яму — она даст уголь для кузницы.".to_string(),
            Some(balance::coal_pit_cost(1)),
            None,
        )
    } else if !state.forge_unlocked {
        (
            "Открой кузницу — письмо в колокольчике.".to_string(),
            Some(balance::forge_cost(0)),
            None,
        )
    } else if state.forge_level == 0 {
        (
            "Построй кузницу, чтобы плавить руду и крафтить снаряжение.".to_string(),
            Some(balance::forge_cost(1)),
            None,
        )
    } else if state.bank_seen && !state.bank_unlocked {
        (
            "Открой банк — письмо в колокольчике.".to_string(),
            Some(balance::bank_cost(0)),
            None,
        )
    } else if state.bank_unlocked && state.bank_level == 0 {
        (
            "Построй банк, чтобы поднять лимит золота.".to_string(),
            Some(balance::bank_cost(1)),
            None,
        )
    } else if state.active_amount("wood") as f64 >= state.cap(ResourceKind::Wood)
        || state.active_amount("stone") as f64 >= state.cap(ResourceKind::Stone)
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
                        div { class: "goal-cost",
                            CostLabel { cost }
                        }
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

    if state.wood_seen || state.active_amount("wood") > 0 {
        items.push((
            ResourceKind::Wood,
            "Дерево",
            state.active_amount("wood") as f64,
            state.cap(ResourceKind::Wood),
        ));
    }
    if state.stone_seen || state.active_amount("stone") > 0 {
        items.push((
            ResourceKind::Stone,
            "Камень",
            state.active_amount("stone") as f64,
            state.cap(ResourceKind::Stone),
        ));
    }
    if state.coal_seen || state.active_amount("coal") > 0 {
        items.push((
            ResourceKind::Coal,
            "Уголь",
            state.active_amount("coal") as f64,
            state.cap(ResourceKind::Coal),
        ));
    }
    if state.forge_researched || state.forge_level > 0 {
        items.push((
            ResourceKind::Fire,
            "Огонь",
            state.current_fire as f64,
            state.cap(ResourceKind::Fire),
        ));
    }
    if state.gold > 0.0 || state.bank_seen || state.bank_level > 0 {
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
                span { class: "resources-empty",
                    "Ресурсов пока нет — отдохни и начни добывать"
                }
            } else {
                for (kind , name , value , cap) in items {
                    div { class: "resource", key: "{name}",
                        if let Some(icon) = resource_icon(kind) {
                            img {
                                class: "resource-icon icon",
                                src: "{icon}",
                                alt: "",
                            }
                        } else {
                            span { class: "resource-icon", "{resource_text_icon(kind)}" }
                        }
                        div { class: "resource-text",
                            span { class: "resource-name", "{name}" }
                            span { class: if value >= cap { "resource-value full" } else { "resource-value" },
                                "{format_num(value)} / {format_num(cap)}"
                            }
                        }
                    }
                }
                if state.time > 0.0 {
                    div { class: "resource",
                        span { class: "resource-icon", "⏳" }
                        div { class: "resource-text",
                            span { class: "resource-name", "Время" }
                            span { class: "resource-value", "{format_num(state.time)}" }
                        }
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
        div { class: "tabbar",
            button {
                class: "tab-arrow",
                aria_label: "Прокрутить вкладки влево",
                onclick: move |_| scroll_tabs(-1),
                "<"
            }
            nav { class: "tabs",
                for (i , (_icon , label)) in TABS.iter().enumerate() {
                    button {
                        key: "{i}",
                        class: if active() == i { "tab active" } else { "tab" },
                        onclick: move |_| active.set(i),
                        if let Some(icon) = tab_icon(i) {
                            img {
                                class: "tab-icon icon",
                                src: "{icon}",
                                alt: "",
                            }
                        } else {
                            span { class: "tab-icon", "{_icon}" }
                        }
                        span { class: "tab-label", "{label}" }
                    }
                }
            }
            button {
                class: "tab-arrow",
                aria_label: "Прокрутить вкладки вправо",
                onclick: move |_| scroll_tabs(1),
                ">"
            }
        }
    }
}

/// Прокручивает нижние вкладки на один экран в заданную сторону.
fn scroll_tabs(direction: i32) {
    let delta = direction * 180;
    spawn(async move {
        let script = format!(
            "(() => {{ const nav = document.querySelector('.tabs'); if (nav) nav.scrollBy({{ left: {delta}, behavior: 'smooth' }}); }})();"
        );
        if let Err(error) = document::eval(&script).await {
            eprintln!("Не удалось прокрутить вкладки: {error}");
        }
    });
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
        // Наигранное время — не прогресс, его сброс не трогает.
        let total_play_seconds = state.peek().total_play_seconds;
        let fresh = GameState {
            theme,
            total_play_seconds,
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

/// Загруженная PNG-иконка вкладки (`None` — рисуем эмодзи).
fn tab_icon(index: usize) -> Option<Asset> {
    match index {
        TAB_FIELD => Some(ICON_FIELD),
        TAB_WORK => None,
        TAB_BAG => Some(ICON_BAG),
        TAB_EXPEDITION => None,
        TAB_SETTINGS => Some(ICON_GEAR),
        _ => None,
    }
}

/// Загруженная PNG-иконка ресурса. У огня своего спрайта нет — эмодзи.
fn resource_icon(kind: ResourceKind) -> Option<Asset> {
    match kind {
        ResourceKind::Wood => Some(ICON_WOOD),
        ResourceKind::Stone => Some(ICON_STONE),
        ResourceKind::Coal => Some(ICON_COAL),
        ResourceKind::Fire => None,
        ResourceKind::Gold => Some(ICON_GOLD),
    }
}
