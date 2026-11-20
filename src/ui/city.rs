//! Город: карта зданий и подэкраны лавки, рынка, банка, гильдии, ратуши
//! и таверны.

use dioxus::prelude::*;

use crate::game::actions;
use crate::game::balance;
use crate::game::city::{self, CityEvent, DiceResult};
use crate::game::i18n;
use crate::game::items;
use crate::game::state::GameState;

use super::app::{format_num, refresh_view, CostLabel, TravelerFocus};
use super::inventory::{ingredient_name, slot_short, ItemCell, ItemDetails, ItemIcon};

const ICON_CITY_BANK: Asset = asset!("/assets/icons/city_bank.png");
const ICON_CITY_CHURCH: Asset = asset!("/assets/icons/city_church.png");
const ICON_CITY_GUILD: Asset = asset!("/assets/icons/city_guild.png");
const ICON_CITY_MARKET: Asset = asset!("/assets/icons/city_market.png");
const ICON_CITY_TAVERN: Asset = asset!("/assets/icons/city_tavern.png");
const ICON_CITY_TOWN_HALL: Asset = asset!("/assets/icons/city_town_hall.png");
const ICON_CITY_TRAVELERS: Asset = asset!("/assets/icons/city_travelers.png");

/// Какой подэкран города открыт. Карта — стартовый вид.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CityView {
    Map,
    Market,
    Shop,
    Bank,
    Guild,
    TownHall,
    Tavern,
    Church,
    /// Дом странников: стройка, а после постройки — сами путешественники.
    TravelersHouse,
}

/// Золото с подписью ресурса.
fn money(value: f64) -> String {
    format!("{} {}", format_num(value), i18n::t("res.gold"))
}

/// Длительность в часах, минутах или секундах.
fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    if total >= 3600 {
        i18n::tf(
            "time.hours_minutes",
            &[
                &(total / 3600).to_string(),
                &((total % 3600) / 60).to_string(),
            ],
        )
    } else if total >= 60 {
        i18n::tf(
            "time.minutes_seconds",
            &[&(total / 60).to_string(), &(total % 60).to_string()],
        )
    } else {
        i18n::tf("time.seconds", &[&total.to_string()])
    }
}

/// Цвет полосы цены: скидка — зелёный, наценка — красный.
fn price_band(mult: f32) -> &'static str {
    if mult < 0.7 {
        "price-green"
    } else if mult < 1.0 {
        "price-yellow"
    } else if mult <= 1.2 {
        "price-orange"
    } else {
        "price-red"
    }
}

fn view_title(view: CityView) -> &'static str {
    match view {
        CityView::Map => i18n::t("tab.city"),
        CityView::Market => i18n::t("market.title"),
        CityView::Shop => i18n::t("city.shop"),
        CityView::Bank => i18n::t("city.bank"),
        CityView::Guild => i18n::t("city.guild"),
        CityView::TownHall => i18n::t("city.town_hall"),
        CityView::Tavern => i18n::t("city.tavern"),
        CityView::Church => i18n::t("city.church"),
        CityView::TravelersHouse => i18n::t("traveler_house.title"),
    }
}

/// NPC, который «встречает» игрока в здании (лавка — без NPC).
fn view_npc(view: CityView) -> &'static str {
    match view {
        CityView::Bank => i18n::t("npc.banker"),
        CityView::Guild => i18n::t("npc.guild"),
        CityView::TownHall => i18n::t("npc.guard"),
        CityView::Tavern => i18n::t("npc.tavern"),
        CityView::Church => i18n::t("npc.priest"),
        _ => "",
    }
}

/// Описание активного события. У слуха подставляются названия товаров.
fn event_desc(event: &CityEvent) -> String {
    match event {
        CityEvent::Rumor {
            item_id, dead_item, ..
        } => {
            if dead_item.is_empty() {
                i18n::tf("event.rumor.hot", &[ingredient_name(item_id)])
            } else {
                i18n::tf(
                    "event.rumor.desc",
                    &[ingredient_name(dead_item), ingredient_name(item_id)],
                )
            }
        }
        CityEvent::Competitor { items, .. } if !items.is_empty() => {
            let names = items
                .iter()
                .map(|id| ingredient_name(id))
                .collect::<Vec<_>>()
                .join(", ");
            i18n::tf("event.competitor.items", &[&names])
        }
        CityEvent::Competitor { .. } => i18n::t("event.competitor.desc").to_string(),
        CityEvent::Epidemic { .. } => i18n::t("event.epidemic.desc").to_string(),
        CityEvent::Festival { .. } => i18n::t("event.festival.desc").to_string(),
        CityEvent::Fair { .. } => i18n::t("event.fair.desc").to_string(),
    }
}

/// Полный экран города: карта зданий и открытый подэкран.
#[component]
pub fn CityScreen(
    state: Signal<GameState>,
    view: GameState,
    back: Signal<u32>,
    subview: Signal<CityView>,
    /// Окно предложения Дома странников рисуется прямо в городе, а не
    /// отдельным оверлеем сверху: тогда под ним нечего двигать.
    #[props(default = true)]
    house_modal: bool,
) -> Element {
    let mut subview = subview;
    use_context_provider(move || subview);

    // Системная «Назад» из подэкрана возвращает на карту города.
    // `peek` обязателен: обычное чтение подписало бы эффект на `subview`,
    // и он сбрасывал бы открытый подэкран обратно на карту сразу после клика.
    use_effect(move || {
        let _ = back();
        if *subview.peek() != CityView::Map {
            subview.set(CityView::Map);
        }
    });

    if !view.city.unlocked {
        return rsx! {
            section { class: "city-screen",
                p { class: "city-hint", "{i18n::t(\"city.not_unlocked\")}" }
            }
        };
    }

    let now = view.total_play_seconds;
    // Карточка Дома странников: видна на карте сразу после аренды земли,
    // дальше только показывает прогресс.
    let house_hint = if city::traveler_house_built(&view) {
        i18n::t("traveler_house.built").to_string()
    } else if city::traveler_house_building(&view) {
        i18n::tf(
            "traveler_house.building",
            &[&format_duration(
                (view.city.traveler_house_ends_at - now).max(0.0),
            )],
        )
    } else {
        i18n::t("traveler_house.rented").to_string()
    };
    let fire_pending = view.city.fire_pending;
    let active_event = city::active_event(&view).cloned();
    let event_title = active_event
        .as_ref()
        .map(|event| event.name().to_string())
        .unwrap_or_default();
    let event_body = active_event.as_ref().map(event_desc).unwrap_or_default();
    let event_left = active_event
        .as_ref()
        .map(|event| format_duration(event.ends_at() - now))
        .unwrap_or_default();
    let fire_repair = i18n::tf(
        "event.repair",
        &[&money(balance::CITY_EVENT_FIRE_REPAIR_GOLD)],
    );
    let bribe = i18n::tf("event.bribe", &[&money(balance::CITY_EVENT_BRIBE_GOLD)]);
    let shop_open = view.city.shop.is_some();
    let citizen_line = i18n::t(&format!(
        "npc.line.{}",
        ((view.total_play_seconds / 45.0) as u64) % 5
    ));
    let shop_hint = if shop_open {
        i18n::t("city.shop_hint").to_string()
    } else {
        i18n::t("shop.closed").to_string()
    };
    let guild_hint = if view.city.guild_unlocked {
        i18n::tf(
            "guild.rank",
            &[
                &city::guild_rank(&view).to_string(),
                &balance::GUILD_RANK_MAX.to_string(),
            ],
        )
    } else {
        i18n::tf(
            "city.guild_hint",
            &[
                &balance::GUILD_SALES_REQUIRED.to_string(),
                &money(balance::GUILD_ENTRY_GOLD),
            ],
        )
    };
    let town_hall_hint = if view.city.town_hall_unlocked {
        i18n::t("town_hall.title").to_string()
    } else {
        i18n::tf(
            "city.town_hall_hint",
            &[
                &balance::TOWN_HALL_SALES_REQUIRED.to_string(),
                &format_num(balance::TOWN_HALL_MIN_RATING as f64),
            ],
        )
    };

    let house_offer = house_modal
        && view.city.traveler_house_offered
        && view.city.traveler_house_answer.is_none()
        && !city::traveler_house_built(&view);

    rsx! {
        section { class: "city-screen",
            if subview() != CityView::Map {
                div { class: "city-head",
                    button {
                        class: "city-back",
                        onclick: move |_| subview.set(CityView::Map),
                        "{i18n::t(\"city.back\")}"
                    }
                    h2 { class: "city-title", "{view_title(subview())}" }
                    if !view_npc(subview()).is_empty() {
                        span { class: "city-npc", "{view_npc(subview())}" }
                    }
                }
            }

            if fire_pending {
                div { class: "city-event fire",
                    div { class: "city-event-head",
                        span { class: "city-event-name", "{i18n::t(\"event.fire.name\")}" }
                    }
                    p { class: "city-event-body",
                        "{i18n::tf(\"event.fire.desc\", &[&money(balance::CITY_EVENT_FIRE_REPAIR_GOLD)])}"
                    }
                    div { class: "city-event-actions",
                        button {
                            class: "city-btn",
                            onclick: move |_| {
                                let paid = {
                                    let mut guard = state.write();
                                    city::resolve_fire(&mut guard, true)
                                };
                                let _ = paid;
                                refresh_view(state);
                            },
                            "{fire_repair}"
                        }
                        button {
                            class: "city-btn ghost",
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    city::resolve_fire(&mut guard, false);
                                }
                                refresh_view(state);
                            },
                            "{i18n::t(\"event.ignore\")}"
                        }
                    }
                }
            } else if active_event.is_some() {
                div { class: "city-event",
                    div { class: "city-event-head",
                        span { class: "city-event-name", "{event_title}" }
                        span { class: "city-event-time", "{i18n::tf(\"event.active\", &[&event_left])}" }
                    }
                    p { class: "city-event-body", "{event_body}" }
                    if matches!(active_event, Some(CityEvent::Competitor { .. })) {
                        button {
                            class: "city-btn",
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    city::bribe_competitor(&mut guard);
                                }
                                refresh_view(state);
                            },
                            "{bribe}"
                        }
                    }
                }
            }

            if subview() == CityView::Map {
                div { class: "city-topbar",
                    button {
                        class: "city-btn market-open",
                        onclick: move |_| subview.set(CityView::Market),
                        "{i18n::t(\"market.title\")}"
                    }
                }
            }

            match subview() {
                CityView::Church => rsx! {
                    ChurchScreen { state, view: view.clone() }
                },
                CityView::Map => rsx! {
                    div { class: "city-map",
                        CityBuilding {
                            icon: ICON_CITY_MARKET,
                            title: i18n::t("city.shop").to_string(),
                            hint: shop_hint,
                            unlocked: shop_open,
                            on_open: move |_| subview.set(CityView::Shop),
                        }
                        CityBuilding {
                            icon: ICON_CITY_BANK,
                            title: i18n::t("city.bank").to_string(),
                            hint: i18n::t("city.bank_hint").to_string(),
                            unlocked: true,
                            on_open: move |_| subview.set(CityView::Bank),
                        }
                        CityBuilding {
                            icon: ICON_CITY_GUILD,
                            title: i18n::t("city.guild").to_string(),
                            hint: guild_hint,
                            unlocked: true,
                            on_open: move |_| subview.set(CityView::Guild),
                        }

                        CityBuilding {
                            icon: ICON_CITY_TOWN_HALL,
                            title: i18n::t("city.town_hall").to_string(),
                            hint: town_hall_hint,
                            unlocked: view.city.town_hall_unlocked,
                            on_open: move |_| subview.set(CityView::TownHall),
                        }
                        CityBuilding {
                            icon: ICON_CITY_TAVERN,
                            title: i18n::t("city.tavern").to_string(),
                            hint: i18n::t("city.tavern_hint").to_string(),
                            unlocked: true,
                            on_open: move |_| subview.set(CityView::Tavern),
                        }
                        if view.city.traveler_house_rented {
                            CityBuilding {
                                icon: ICON_CITY_TRAVELERS,
                                title: i18n::t("traveler_house.title").to_string(),
                                hint: house_hint.clone(),
                                unlocked: true,
                                on_open: move |_| subview.set(CityView::TravelersHouse),
                            }
                        }
                        CityBuilding {
                            icon: ICON_CITY_CHURCH,
                            title: i18n::t("city.church").to_string(),
                            hint: i18n::t("city.church_hint").to_string(),
                            unlocked: true,
                            on_open: move |_| subview.set(CityView::Church),
                        }
                    }
                    div { class: "city-citizen",
                        span { class: "city-citizen-name", "{i18n::t(\"npc.citizen\")}" }
                        span { class: "city-citizen-line", "{citizen_line}" }
                    }
                },
                CityView::Market => rsx! {
                    MarketScreen { state, view: view.clone() }
                },
                CityView::Shop => rsx! {
                    ShopScreen { state, view: view.clone() }
                },
                CityView::Bank => rsx! {
                    BankScreen { state, view: view.clone() }
                },
                CityView::Guild => rsx! {
                    GuildScreen { state, view: view.clone() }
                },

                CityView::TownHall => rsx! {
                    TownHallScreen { state, view: view.clone() }
                },
                CityView::Tavern => rsx! {
                    TavernScreen { state, view: view.clone() }
                },
                CityView::TravelersHouse => rsx! {
                    TravelersHouseScreen { state, view: view.clone() }
                },
            }

            if house_offer {
                TravelerHouseModal { state, city_view: subview, overlay: false }
            }
        }
    }
}

/// Квадрат здания на карте города.
#[component]
fn CityBuilding(
    icon: Asset,
    title: String,
    hint: String,
    unlocked: bool,
    on_open: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: if unlocked { "city-building" } else { "city-building locked" },
            disabled: !unlocked,
            onclick: move |_| on_open.call(()),
            img { class: "city-building-icon", src: "{icon}", alt: "" }
            span { class: "city-building-name", "{title}" }
            span { class: "city-building-hint", "{hint}" }
        }
    }
}

/// Диалог Мартина при входе в город: «Ты мастер?».
#[component]
pub fn MartinDialog(state: Signal<GameState>) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);

    rsx! {
        div { class: "offer-overlay",
            div { class: "offer-card",
                div { class: "offer-title", "{i18n::t(\"martin.name\")}" }
                div { class: "offer-body", "{i18n::t(\"martin.greeting\")}" }
                div { class: "offer-body", "{i18n::t(\"martin.ask\")}" }
                div { class: "offer-actions",
                    button {
                        class: "steam-cta",
                        onclick: move |_| {
                            let ok = {
                                let mut guard = state.write();
                                city::martin_answer(&mut guard, true)
                            };
                            if !ok {
                                message.set(i18n::t("err.shop_no_space").to_string());
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"martin.yes\")}"
                    }
                    button {
                        class: "offer-later",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::martin_answer(&mut guard, false);
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"martin.no\")}"
                    }
                }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
            }
        }
    }
}

/// Строка списка путешественников в таверне.
#[derive(Clone, PartialEq)]
struct TravelerView {
    index: usize,
    name: String,
    specialism: String,
    status: String,
    ready: bool,
    advance_text: String,
    info: Option<TravelerCardView>,
}

/// Список путешественников: общий для таверны и Дома странников.
fn travelers_view(state: &GameState, open: Option<usize>, with_gear: bool) -> Vec<TravelerView> {
    let now = state.total_play_seconds;
    state
        .city
        .travelers
        .iter()
        .enumerate()
        .map(|(index, traveler)| {
            let ready = city::traveler_ready(traveler);
            // Лут и доплату забирают отдельной кнопкой: пока игрок не нажал,
            // человек занят, и скрывать это за «свободен» нельзя.
            let waiting = !traveler.carried.is_empty() || traveler.payout > 1e-9;
            let status = if waiting {
                i18n::t("traveler.waiting").to_string()
            } else if city::traveler_waits_for_compass(traveler) {
                i18n::t("traveler.lost_compass").to_string()
            } else if ready {
                i18n::t("traveler.free").to_string()
            } else {
                i18n::tf(
                    "traveler.back_in",
                    &[&format_duration((traveler.returns_at - now).max(0.0))],
                )
            };
            let info = if open == Some(index) {
                Some(traveler_card(state, traveler, index, with_gear))
            } else {
                None
            };
            TravelerView {
                index,
                name: city::traveler_name(traveler.profile_id).to_string(),
                specialism: city::traveler_specialism(traveler.profile_id).to_string(),
                status,
                ready,
                advance_text: i18n::tf(
                    "traveler.advance",
                    &[&money(balance::TRAVELER_LOCATIONS[0].advance)],
                ),
                info,
            }
        })
        .collect()
}

/// Содержимое раскрытой карточки путешественника.
#[derive(Clone, PartialEq)]
struct TravelerCardView {
    specialism_text: String,
    advance_text: String,
    /// Строка о том, сколько походов осталось до потери компаса,
    /// или просьба принести компас.
    stage_text: String,
    /// Странник ждёт компас: показываем кнопку передачи.
    compass_wanted: bool,
    /// Компас лежит в сумке: кнопку можно нажать.
    compass_ready: bool,
    locations: Vec<TravelerTripView>,
    /// Что путешественник принёс и ждёт, пока это заберут, — квадратами.
    loot: Vec<TravelerLootView>,
    payout_text: String,
    claim_ready: bool,
    empty_trip: bool,
    /// Снаряжение: семь слотов в порядке `EquipSlot::ALL`.
    gear: Vec<TravelerGearView>,
    /// Что можно надеть в каждый слот (индекс — номер слота).
    gear_picks: Vec<Vec<TravelerGearPickView>>,
    /// Сколько вещей сета странника надето и что это даёт.
    set_text: String,
    set_active: bool,
    /// Сколько вещей сета надето — для карточки предмета.
    set_pieces: u32,
    gear_hint: String,
}

/// Один слот снаряжения путешественника.
#[derive(Clone, PartialEq)]
struct TravelerGearView {
    slot: usize,
    label: String,
    short: String,
    item: Option<items::Item>,
    /// Сколько таких вещей лежит в сумке — для карточки предмета.
    bag_count: u32,
}

/// Вещь из сумки, которую можно надеть на путешественника.
#[derive(Clone, PartialEq)]
struct TravelerGearPickView {
    item_id: String,
    name: String,
    slot: items::EquipSlot,
    count: u32,
}

/// Добыча странника: предмет и количество для квадрата с бейджем.
#[derive(Clone, PartialEq)]
struct TravelerLootView {
    item_id: String,
    count: u32,
}

/// Локация, куда можно отправить путешественника.
#[derive(Clone, PartialEq)]
struct TravelerTripView {
    id: String,
    name: String,
    time: String,
    advance: String,
    closed: bool,
    can_send: bool,
    /// Что может выпасть и с каким шансом.
    loot_lines: Vec<String>,
}

#[derive(Clone, PartialEq)]
struct SlotView {
    index: usize,
    item_id: String,
    name: String,
    quantity: u32,
    mult: f32,
    price_text: String,
    mult_text: String,
    band: &'static str,
    demand_percent: u32,
    demand_text: String,
    demand_dead: bool,
    status: String,
    status_class: &'static str,
}

/// Товар из сумки для сетки «что выставить».
#[derive(Clone, PartialEq)]
struct GoodView {
    id: String,
    item: items::Item,
    count: u32,
}

/// Строка рынка города: товар с иконкой, цена, спрос и стрелка тренда.
#[derive(Clone, PartialEq)]
struct MarketRow {
    id: String,
    name: String,
    price: String,
    /// Сколько спроса налито сейчас; предложение в таблице не показываем.
    demand: String,
    fill_class: &'static str,
    /// Слово «баланс/избыток/дефицит» показываем подсказкой к спросу.
    status: String,
    trend: &'static str,
    trend_class: &'static str,
    /// Числовое значение тренда: по нему сортируется рынок.
    trend_value: i32,
}

/// Рынок города: та же пара «спрос / предложение», что у торговцев,
/// только масштаб крупнее — на полке надо держать больше товара.
#[component]
fn MarketScreen(state: Signal<GameState>, view: GameState) -> Element {
    let _ = state;
    let mut sort_by_trend = use_signal(|| false);
    let mut rows: Vec<MarketRow> = items::catalog()
        .iter()
        .filter(|item| city::is_shop_sellable(&view, &item.id))
        .map(|item| {
            let entry = city::demand_of(&view, &item.id);
            let trend = city::market_trend(&view, &item.id);
            // В узкой колонке — только стрелка: слова «растёт/падает»
            // растягивали строку и мешали читать.
            let (trend_label, trend_class) = if trend > 0 {
                ("↑", "up")
            } else if trend < 0 {
                ("↓", "down")
            } else {
                ("→", "flat")
            };
            let market_status = city::demand_status(&view, &item.id);
            MarketRow {
                id: item.id.clone(),
                name: item.name().to_string(),
                price: format_num(city::shop_price_for(&view, &item.id, 1.0)),
                // В таблице оставляем только спрос: предложение игроку не нужно.
                demand: entry.demand.to_string(),
                fill_class: if entry.demand >= 67 {
                    "high"
                } else if entry.demand >= 34 {
                    "mid"
                } else {
                    "low"
                },
                status: market_status.label().to_string(),
                trend: trend_label,
                trend_class,
                trend_value: trend,
            }
        })
        .collect();
    if sort_by_trend() {
        rows.sort_by(|left, right| {
            right
                .trend_value
                .cmp(&left.trend_value)
                .then_with(|| left.name.cmp(&right.name))
        });
    } else {
        rows.sort_by(|left, right| left.name.cmp(&right.name));
    }
    let count_text = rows.len().to_string();

    rsx! {
        p { class: "city-hint", "{i18n::t(\"market.hint\")}" }
        div { class: "city-topbar market-sort",
            button {
                class: if sort_by_trend() { "city-btn small" } else { "city-btn small active" },
                onclick: move |_| sort_by_trend.set(false),
                "{i18n::t(\"market.sort_name\")}"
            }
            button {
                class: if sort_by_trend() { "city-btn small active" } else { "city-btn small" },
                onclick: move |_| sort_by_trend.set(true),
                "{i18n::t(\"market.sort_trend\")}"
            }
        }
        div { class: "city-card market-card",
            div { class: "city-row market-row market-head",
                span { "{i18n::t(\"market.col_item\")}" }
                span { "{i18n::t(\"market.col_price\")}" }
                span { "{i18n::t(\"market.col_pair\")}" }
                span { "{i18n::t(\"market.col_trend\")}" }
            }
            for row in rows {
                div { class: "city-row market-row", key: "{row.id}",
                    span { class: "market-name",
                        if let Some(item) = items::item(&row.id) {
                            ItemIcon {
                                item: item.clone(),
                                class: "market-icon".to_string(),
                            }
                        }
                        span { class: "market-name-text", "{row.name}" }
                    }
                    span { class: "market-price", "{row.price}" }
                    span { class: "market-pair {row.fill_class}", title: "{row.status}", "{row.demand}" }
                    span { class: "market-trend market-arrow {row.trend_class}", "{row.trend}" }
                }
            }
        }
        p { class: "city-hint", "{count_text}" }
    }
}

/// Лавка: касса, полки и выставление товара.
#[component]
fn ShopScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut subview = use_context::<Signal<CityView>>();
    let mut message = use_signal(String::new);
    let mut add_open = use_signal(|| false);
    let mut selected_goods = use_signal(|| None::<String>);
    let mut pick_amount = use_signal(|| 1u32);
    let mut stock_open = use_signal(|| false);

    let shop = view.city.shop.clone().unwrap_or_default();
    let shop_open = view.city.shop.is_some();
    let rating_text = i18n::tf("shop.rating", &[&format!("{:.2}", shop.rating)]);
    let slots_text = i18n::tf(
        "shop.slots",
        &[&shop.slots.len().to_string(), &shop.max_slots.to_string()],
    );
    let cash_text = i18n::tf(
        "shop.cash",
        &[&format_num(shop.cash), &format_num(shop.cash_cap)],
    );
    let tax_left = if view.city.next_tax_at > 0.0 {
        format_duration(view.city.next_tax_at - view.total_play_seconds)
    } else {
        "—".to_string()
    };
    let tax_text = i18n::tf("shop.tax_in", &[&tax_left]);
    let tax_due = view.city.tax_due;
    let tax_due_text = i18n::tf("tax.due", &[&money(tax_due)]);
    let can_pay_tax = tax_due > 1e-6 && (shop.cash > 0.0 || view.gold > 0.0);
    let reopen_text = i18n::tf("shop.reopen", &[&money(balance::CITY_SHOP_REOPEN_GOLD)]);

    let slots: Vec<SlotView> = shop
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let entry = city::demand_of(&view, &slot.item_id);
            let ratio = city::demand_ratio(&view, &slot.item_id);
            let percent = (ratio * 100.0).round() as u32;
            let demand_text = if percent == 0 {
                i18n::t("demand.dead").to_string()
            } else {
                i18n::tf(
                    "demand.pair",
                    &[&entry.demand.to_string(), &entry.supply.to_string()],
                )
            };
            let status = city::demand_status(&view, &slot.item_id);
            let status_class = match status {
                super::super::game::trader::MarketStatus::Deficit => "deficit",
                super::super::game::trader::MarketStatus::Surplus => "surplus",
                super::super::game::trader::MarketStatus::Balance => "balance",
            };
            SlotView {
                index,
                item_id: slot.item_id.clone(),
                name: ingredient_name(&slot.item_id).to_string(),
                quantity: slot.quantity,
                mult: slot.price_mult,
                price_text: i18n::tf("shop.per_unit", &[&money(city::shop_price(&view, slot))]),
                mult_text: format!("{:.1}", slot.price_mult),
                band: price_band(slot.price_mult),
                demand_percent: percent,
                demand_text,
                demand_dead: percent == 0,
                status: status.label().to_string(),
                status_class,
            }
        })
        .collect();

    let mut goods: Vec<GoodView> = view
        .inventory
        .iter()
        .filter(|(id, count)| **count > 0 && city::is_shop_sellable(&view, id))
        .filter_map(|(id, count)| {
            items::item(id).cloned().map(|item| GoodView {
                id: id.clone(),
                item,
                count: *count,
            })
        })
        .collect();
    goods.sort_by(|left, right| left.item.name().cmp(right.item.name()));

    // Карточка выбранного товара.
    let selected_now = selected_goods();
    let picked = selected_now
        .as_deref()
        .and_then(|id| items::item(id).cloned());
    let picked_name = picked
        .as_ref()
        .map(|item| item.name().to_string())
        .unwrap_or_default();
    let picked_price = picked
        .as_ref()
        .map(|item| i18n::tf("bag.price", &[&format_num(item.base_price as f64)]))
        .unwrap_or_default();
    let picked_have = selected_now
        .as_deref()
        .map(|id| view.amount_of(id))
        .unwrap_or(0);
    let picked_amount = pick_amount().clamp(1, picked_have.max(1));
    let put_id = selected_now.clone().unwrap_or_default();
    let stock_id = put_id.clone();

    let clerk_hired = view.city.clerk_hired;
    let clerk_mult_value = format!("{:.1}", view.city.clerk_mult);
    let clerk_mult_text = i18n::tf("clerk.mult", &[&clerk_mult_value]);
    let clerk_salary_text = i18n::tf("clerk.salary", &[&money(city::clerk_salary(&view))]);
    let clerk_hire_text = i18n::tf("clerk.hire", &[&money(balance::CLERK_PRICE)]);
    let clerk_can_hire = !clerk_hired && view.gold + 1e-9 >= balance::CLERK_PRICE;

    rsx! {
        if shop_open {
            div { class: "city-card",
                div { class: "city-row",
                    span { "{rating_text}" }
                    span { class: "city-row-muted", "{slots_text}" }
                }
                div { class: "city-row",
                    span { "{cash_text}" }
                    button {
                        class: "city-btn small",
                        disabled: shop.cash <= 0.0,
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::withdraw_cash(&mut guard);
                            }
                            // Без текста: касса и кошелёк сами показывают
                            // новую сумму.
                            message.set(String::new());
                            refresh_view(state);
                        },
                        "{i18n::t(\"shop.withdraw\")}"
                    }
                }
                div { class: "city-row muted",
                    span { "{tax_text}" }
                }
                if tax_due > 1e-6 {
                    div { class: "city-row",
                        span { class: "city-row-muted", "{tax_due_text}" }
                        button {
                            class: "city-btn small",
                            disabled: !can_pay_tax,
                            onclick: move |_| {
                                let outcome = {
                                    let mut guard = state.write();
                                    city::pay_tax_due(&mut guard)
                                };
                                let left = state.read().city.tax_due;
                                message
                                    .set(
                                        match outcome {
                                            city::TaxOutcome::Paid => i18n::t("tax.paid").to_string(),
                                            city::TaxOutcome::Short => i18n::tf("tax.short", &[&money(left)]),
                                            _ => i18n::t("tax.nothing").to_string(),
                                        },
                                    );
                                refresh_view(state);
                            },
                            "{i18n::t(\"tax.pay\")}"
                        }
                    }
                }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
                button {
                    class: "city-btn wide market-open",
                    onclick: move |_| subview.set(CityView::Market),
                    "{i18n::t(\"market.title\")}"
                }
            }

            if slots.is_empty() {
                p { class: "city-hint", "{i18n::t(\"shop.slot_empty\")}" }
            } else {
                for slot in slots {
                    ShopSlotCard { key: "{slot.index}", state, slot }
                }
            }

            button {
                class: "city-btn wide",
                onclick: move |_| add_open.set(!add_open()),
                "{i18n::t(\"shop.add\")}"
            }
            if add_open() {
                div { class: "city-card",
                    div { class: "city-row muted",
                        span { "{i18n::t(\"shop.add_title\")}" }
                    }
                    if goods.is_empty() {
                        p { class: "city-hint", "{i18n::t(\"shop.no_goods\")}" }
                    } else {
                        div { class: "inv-grid shop-goods-grid",
                            for good in goods.clone() {
                                GoodCell {
                                    key: "{good.id}",
                                    item: good.item,
                                    count: good.count,
                                    selected: selected_now.as_deref() == Some(good.id.as_str()),
                                    on_select: move |item_id: String| {
                                        selected_goods.set(Some(item_id));
                                        pick_amount.set(1);
                                    },
                                }
                            }
                        }
                    }

                    if let Some(picked_item) = picked {
                        div { class: "city-goods-card",
                            div { class: "city-goods-main",
                                div { class: "city-goods-name", "{picked_name}" }
                                div { class: "city-goods-price", "{picked_price}" }
                                div { class: "city-goods-controls",
                                    button {
                                        class: "city-stepper",
                                        onclick: move |_| { pick_amount.set(picked_amount.saturating_sub(1).max(1)) },
                                        "-"
                                    }
                                    span { class: "city-stepper-value", "{picked_amount}" }
                                    button {
                                        class: "city-stepper",
                                        onclick: move |_| { pick_amount.set((picked_amount + 1).min(picked_have)) },
                                        "+"
                                    }
                                    button {
                                        class: "city-stepper",
                                        onclick: move |_| { pick_amount.set((picked_amount + 10).min(picked_have)) },
                                        "+10"
                                    }
                                    button {
                                        class: "city-stepper",
                                        onclick: move |_| pick_amount.set(picked_have),
                                        "{i18n::t(\"shop.max\")}"
                                    }
                                }
                            }
                            div { class: "city-goods-icon",
                                ItemIcon {
                                    item: picked_item,
                                    class: "inv-item-icon".to_string(),
                                }
                            }
                        }
                        button {
                            class: "city-btn wide",
                            disabled: picked_have == 0,
                            onclick: move |_| {
                                let qty = pick_amount().clamp(1, picked_have.max(1));
                                let outcome = {
                                    let mut guard = state.write();
                                    city::add_slot(&mut guard, &put_id, qty, 1.0)
                                };
                                match outcome {
                                    Ok(()) => {
                                        // Без текста: полка и сумка сами
                                        // показывают, что вещь переехала.
                                        message.set(String::new());
                                        selected_goods.set(None);
                                        pick_amount.set(1);
                                    }
                                    Err(error) => message.set(error.text().to_string()),
                                }
                                refresh_view(state);
                            },
                            "{i18n::t(\"shop.put\")}"
                        }
                        button {
                            class: "city-btn wide",
                            disabled: picked_have == 0,
                            onclick: move |_| {
                                let qty = pick_amount().clamp(1, picked_have.max(1));
                                let outcome = {
                                    let mut guard = state.write();
                                    city::add_stock(&mut guard, &stock_id, qty)
                                };
                                match outcome {
                                    Ok(()) => {
                                        message.set(String::new());
                                        selected_goods.set(None);
                                        pick_amount.set(1);
                                    }
                                    Err(error) => message.set(error.text().to_string()),
                                }
                                refresh_view(state);
                            },
                            "{i18n::t(\"stock.add\")}"
                        }
                    }
                }
            }

            div { class: "city-card",
                div { class: "city-row",
                    span { class: "city-upgrade-name", "{i18n::t(\"clerk.title\")}" }
                }
                div { class: "city-row muted",
                    span { "{i18n::t(\"clerk.hint\")}" }
                }
                if clerk_hired {
                    div { class: "city-row muted",
                        span { "{clerk_mult_text}" }
                    }
                    div { class: "city-row price-stepper",
                        button {
                            class: "city-stepper",
                            disabled: view.city.clerk_mult <= balance::CLERK_MULT_MIN + 1e-6,
                            onclick: move |_| {
                                let current = state.read().city.clerk_mult;
                                let next = ((current - 0.1) * 10.0).round() / 10.0;
                                {
                                    let mut guard = state.write();
                                    city::set_clerk_mult(&mut guard, next);
                                }
                                refresh_view(state);
                            },
                            "−"
                        }
                        span { class: "city-stepper-value", "×{clerk_mult_value}" }
                        button {
                            class: "city-stepper",
                            disabled: view.city.clerk_mult >= balance::CLERK_MULT_MAX - 1e-6,
                            onclick: move |_| {
                                let current = state.read().city.clerk_mult;
                                let next = ((current + 0.1) * 10.0).round() / 10.0;
                                {
                                    let mut guard = state.write();
                                    city::set_clerk_mult(&mut guard, next);
                                }
                                refresh_view(state);
                            },
                            "+"
                        }
                    }
                    div { class: "city-row muted",
                        span { "{clerk_salary_text}" }
                    }
                    // Пауза приказчика: он перестаёт трогать полки и склад.
                    button {
                        class: if view.city.clerk_paused { "city-btn paused" } else { "city-btn" },
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::toggle_clerk_pause(&mut guard);
                            }
                            refresh_view(state);
                        },
                        if view.city.clerk_paused {
                            "{i18n::t(\"clerk.resume\")}"
                        } else {
                            "{i18n::t(\"clerk.pause\")}"
                        }
                    }
                    button {
                        class: "city-btn",
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::clerk_clear_shelves(&mut guard)
                            };
                            match outcome {
                                Ok(moved) => message
                                    .set(i18n::tf("clerk.cleared", &[&moved.to_string()])),
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"clerk.clear_shelves\")}"
                    }
                } else {
                    button {
                        class: "city-btn",
                        disabled: !clerk_can_hire,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::hire_clerk(&mut guard)
                            };
                            match outcome {
                                Ok(()) => message.set(i18n::t("clerk.hired").to_string()),
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{clerk_hire_text}"
                    }
                }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
            }

            div { class: "city-card",
                div { class: "city-row",
                    span { class: "city-upgrade-name", "{i18n::t(\"stock.title\")}" }
                }
                div { class: "city-row muted",
                    span { "{i18n::t(\"stock.hint\")}" }
                }
                button {
                    class: "city-btn wide",
                    onclick: move |_| stock_open.set(!stock_open()),
                    "{i18n::t(\"stock.open\")}"
                }
            }
            if stock_open() {
                StockPanel { state, view: view.clone(), message }
            }
        } else {
            div { class: "city-card",
                p { class: "city-hint", "{i18n::t(\"shop.closed\")}" }
                button {
                    class: "city-btn",
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::reopen_shop(&mut guard)
                        };
                        match outcome {
                            Ok(()) => message.set(i18n::t("shop.reopened").to_string()),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{reopen_text}"
                }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
            }
        }
    }
}

/// Запись склада для отрисовки.
#[derive(Clone, PartialEq)]
struct StockRow {
    index: usize,
    item_id: String,
    name: String,
    quantity: u32,
}

/// Склад приказчика: откуда он берёт товар для полок.
#[component]
fn StockPanel(state: Signal<GameState>, view: GameState, message: Signal<String>) -> Element {
    let mut state = state;
    let mut message = message;
    let mut picked = use_signal(|| None::<usize>);
    let mut amount = use_signal(|| 1u32);

    let rows: Vec<StockRow> = view
        .city
        .stock
        .iter()
        .enumerate()
        .map(|(index, row)| StockRow {
            index,
            item_id: row.item_id.clone(),
            name: ingredient_name(&row.item_id).to_string(),
            quantity: row.quantity,
        })
        .collect();
    let cells = city::clerk_stock_cells(&view);
    let cells_text = i18n::tf(
        "stock.cells",
        &[&rows.len().to_string(), &cells.to_string()],
    );
    // Что приказчик возьмёт на полки при следующем обходе.
    let plan = city::stock_plan(&view, &[], cells as usize, None, false);
    let plan_text = if plan.is_empty() {
        String::new()
    } else {
        let parts: Vec<String> = plan
            .iter()
            .map(|(id, quantity)| format!("{} ×{quantity}", ingredient_name(id)))
            .collect();
        i18n::tf("stock.plan", &[&parts.join(", ")])
    };

    let picked_row = picked().and_then(|index| rows.get(index).cloned());
    let item_name = picked_row
        .as_ref()
        .map(|row| row.name.clone())
        .unwrap_or_default();
    let item_have = picked_row.as_ref().map(|row| row.quantity).unwrap_or(0);
    let amount_now = amount().clamp(1, item_have.max(1));

    rsx! {
        div { class: "city-card",
            div { class: "city-row",
                span { "{cells_text}" }
            }
            if !plan_text.is_empty() {
                div { class: "city-row muted",
                    span { "{plan_text}" }
                }
            }
            if rows.is_empty() {
                p { class: "city-hint", "{i18n::t(\"stock.empty\")}" }
            } else {
                div { class: "inv-grid shop-goods-grid",
                    for row in rows.clone() {
                        {
                            let row_id = row.item_id.clone();
                            let row_index = row.index;
                            let quantity = row.quantity;
                            let row_name = row.name.clone();
                            let active = picked() == Some(row_index);
                            rsx! {
                                button {
                                    key: "{row_index}",
                                    class: if active { "inv-item selected" } else { "inv-item" },
                                    title: "{row_name}",
                                    onclick: move |_| {
                                        if picked() == Some(row_index) {
                                            picked.set(None);
                                        } else {
                                            picked.set(Some(row_index));
                                            amount.set(1);
                                        }
                                    },
                                    if let Some(item) = items::item(&row_id) {
                                        ItemIcon { item: item.clone(), class: "inv-item-icon".to_string() }
                                    }
                                    if quantity > 1 {
                                        span { class: "inv-badge", "{quantity}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if picked_row.is_some() {
                div { class: "city-card",
                    div { class: "city-row",
                        span { class: "city-upgrade-name", "{item_name}" }
                        span { class: "city-row-muted", "×{item_have}" }
                    }
                    div { class: "city-goods-controls",
                        button {
                            class: "city-stepper",
                            onclick: move |_| {
                                let next = amount_now.saturating_sub(1).max(1);
                                amount.set(next);
                            },
                            "-"
                        }
                        span { class: "city-stepper-value", "{amount_now}" }
                        button {
                            class: "city-stepper",
                            onclick: move |_| {
                                let next = (amount_now + 1).min(item_have.max(1));
                                amount.set(next);
                            },
                            "+"
                        }
                        button {
                            class: "city-stepper",
                            onclick: move |_| {
                                let next = (amount_now + 10).min(item_have.max(1));
                                amount.set(next);
                            },
                            "+10"
                        }
                        button {
                            class: "city-stepper",
                            onclick: move |_| amount.set(item_have.max(1)),
                            "{i18n::t(\"shop.max\")}"
                        }
                    }
                    button {
                        class: "city-btn wide",
                        onclick: move |_| {
                            let Some(index) = picked() else {
                                return;
                            };
                            let quantity = amount_now.max(1);
                            let outcome = {
                                let mut guard = state.write();
                                city::stock_to_shelf(&mut guard, index, quantity)
                            };
                            match outcome {
                                Ok(()) => {
                                    message.set(String::new());
                                    picked.set(None);
                                    amount.set(1);
                                }
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"shop.put\")}"
                    }
                    button {
                        class: "city-btn wide",
                        onclick: move |_| {
                            let Some(index) = picked() else {
                                return;
                            };
                            let quantity = amount_now.max(1);
                            let outcome = {
                                let mut guard = state.write();
                                city::take_stock_amount(&mut guard, index, quantity)
                            };
                            match outcome {
                                Ok(()) => {
                                    message.set(String::new());
                                    picked.set(None);
                                    amount.set(1);
                                }
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"stock.take\")}"
                    }
                }
            }
        }
    }
}

/// Одна полка: товар, количество, цена и ползунок наценки.
#[component]
fn ShopSlotCard(state: Signal<GameState>, slot: SlotView) -> Element {
    let mut state = state;
    let index = slot.index;
    let item = items::item(&slot.item_id).cloned();

    rsx! {
        div { class: "city-slot",
            div { class: "city-slot-head",
                if let Some(item) = item {
                    ItemIcon {
                        item: item.clone(),
                        class: "city-item-icon".to_string(),
                    }
                }
                span { class: "city-slot-name", "{slot.name}" }
                span { class: "city-slot-qty", "×{slot.quantity}" }
                button {
                    class: "city-slot-remove",
                    onclick: move |_| {
                        {
                            let mut guard = state.write();
                            let _ = city::remove_slot(&mut guard, index);
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"shop.remove\")}"
                }
            }
            div { class: "city-row",
                span { class: "city-slot-price", "{slot.price_text}" }
                span { class: "city-price-mult {slot.band}", "×{slot.mult_text}" }
            }
            div { class: "demand-row",
                span { class: if slot.demand_dead { "demand-label dead" } else { "demand-label" },
                    "{slot.demand_text}"
                }
                span { class: "market-short {slot.status_class}", "{slot.status}" }
                div { class: "demand-bar",
                    div {
                        class: if slot.demand_dead { "demand-fill dead" } else { "demand-fill" },
                        style: "width: {slot.demand_percent}%",
                    }
                }
            }
            div { class: "city-row price-stepper",
                button {
                    class: "city-stepper",
                    disabled: slot.mult <= balance::SHOP_PRICE_MIN + 1e-6,
                    onclick: move |_| {
                        let next = ((slot.mult - 0.1) * 10.0).round() / 10.0;
                        {
                            let mut guard = state.write();
                            let _ = city::set_price(&mut guard, index, next);
                        }
                        refresh_view(state);
                    },
                    "−"
                }
                span { class: "city-stepper-value", "×{slot.mult_text}" }
                button {
                    class: "city-stepper",
                    disabled: slot.mult >= balance::SHOP_PRICE_MAX - 1e-6,
                    onclick: move |_| {
                        let next = ((slot.mult + 0.1) * 10.0).round() / 10.0;
                        {
                            let mut guard = state.write();
                            let _ = city::set_price(&mut guard, index, next);
                        }
                        refresh_view(state);
                    },
                    "+"
                }
            }
        }
    }
}

/// Кубик товара в сетке «что выставить».
#[component]
fn GoodCell(
    item: items::Item,
    count: u32,
    selected: bool,
    on_select: EventHandler<String>,
) -> Element {
    let click_id = item.id.clone();

    rsx! {
        button {
            class: if selected { "inv-item selected" } else { "inv-item" },
            onclick: move |_| on_select.call(click_id.clone()),
            ItemIcon { item: item.clone(), class: "inv-item-icon".to_string() }
            if count > 1 {
                span { class: "inv-badge", "{count}" }
            }
        }
    }
}

/// Банк: ячейки золотого лимита и задания банкира.
#[component]
fn BankScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);

    let cells = view.city.bank_cells;
    let maxed = cells >= balance::BANK_CELLS_MAX;
    // Цена следующей ячейки считается только по купленным: награды банкира
    // ячейки дают, но покупку не удорожают.
    let cells_text = i18n::tf(
        "bank.cells",
        &[&cells.to_string(), &balance::BANK_CELLS_MAX.to_string()],
    );
    let cap_text = i18n::tf("bank.cap", &[&money(view.gold_cap())]);
    let price_text = i18n::tf(
        "bank.next_price",
        &[&money(city::bank_cell_price(view.city.bank_paid_cells))],
    );
    let done = view.city.bank_quests_done;
    let quest = city::bank_quests().get(done as usize).copied();
    let quest_text = quest
        .map(|quest| {
            i18n::tf(
                "bank.quest_line",
                &[
                    &quest.quantity.to_string(),
                    ingredient_name(quest.item_id),
                    &quest.cells.to_string(),
                ],
            )
        })
        .unwrap_or_default();
    let owned = quest
        .map(|quest| view.amount_of(quest.item_id))
        .unwrap_or(0);
    let can_turn_in = quest.map(|quest| owned >= quest.quantity).unwrap_or(false);
    let quest_owned = quest
        .map(|quest| format!("{} / {}", owned, quest.quantity))
        .unwrap_or_default();

    rsx! {
        div { class: "city-card",
            div { class: "city-row",
                span { "{cells_text}" }
                span { "{cap_text}" }
            }
            div { class: "city-row muted",
                span { "{price_text}" }
            }
            button {
                class: "city-btn",
                disabled: maxed,
                onclick: move |_| {
                    let outcome = {
                        let mut guard = state.write();
                        city::buy_bank_cell(&mut guard)
                    };
                    match outcome {
                        Ok(()) => {
                            message
                                .set(
                                    i18n::tf("bank.bought", &[&format_num(balance::BANK_CELL_BONUS)]),
                                )
                        }
                        Err(error) => message.set(error.text().to_string()),
                    }
                    refresh_view(state);
                },
                "{i18n::t(\"bank.buy\")}"
            }
            if !message().is_empty() {
                div { class: "city-msg", "{message()}" }
            }
        }

        div { class: "city-card",
            div { class: "city-row muted",
                span { "{i18n::t(\"bank.quests\")}" }
            }
            if quest.is_some() {
                div { class: "city-row",
                    span { "{quest_text}" }
                    span { class: "city-row-muted", "{quest_owned}" }
                }
                button {
                    class: "city-btn",
                    disabled: !can_turn_in,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::complete_bank_quest(&mut guard)
                        };
                        match outcome {
                            Ok(cells) => message.set(i18n::tf("bank.bought", &[&cells.to_string()])),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"bank.quest_done\")}"
                }
            } else {
                p { class: "city-hint", "{i18n::t(\"bank.no_more\")}" }
            }
        }
    }
}

/// Храм: благословение за пожертвование и исповедь.
#[component]
fn ChurchScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    let blessing_left = (view.city.blessing_until - view.total_play_seconds).max(0.0);
    let blessing_text = if blessing_left > 0.0 {
        i18n::tf("church.blessing", &[&format_duration(blessing_left)])
    } else {
        i18n::t("church.no_blessing").to_string()
    };
    let confession_left = (view.city.next_confession_at - view.total_play_seconds).max(0.0);
    let confession_ready = confession_left <= 0.0;
    let confession_price = money(balance::CHURCH_CONFESSION_PRICE);
    let confess_text = if confession_ready {
        i18n::tf("church.confess", &[&confession_price])
    } else {
        i18n::tf("church.cooldown", &[&format_duration(confession_left)])
    };
    let missed_text = i18n::tf("church.missed", &[&view.city.taxes_missed.to_string()]);
    let message_text = message();

    rsx! {
        div { class: "city-card",
            div { class: "city-row muted",
                span { "{i18n::t(\"church.bless_hint\")}" }
            }
            div { class: "city-row",
                span { "{blessing_text}" }
            }
            div { class: "church-donations",
                for amount in balance::CHURCH_DONATIONS {
                    {
                        let label = i18n::tf("church.donate", &[&money(amount)]);
                        let affordable = view.gold + 1e-9 >= amount;
                        rsx! {
                            button {
                                class: "city-btn",
                                disabled: !affordable,
                                onclick: move |_| {
                                    let outcome = {
                                        let mut guard = state.write();
                                        city::donate(&mut guard, amount)
                                    };
                                    match outcome {
                                        Ok(()) => message.set(i18n::t("church.donated").to_string()),
                                        Err(error) => message.set(error.text().to_string()),
                                    }
                                    refresh_view(state);
                                },
                                "{label}"
                            }
                        }
                    }
                }
            }
        }
        div { class: "city-card",
            div { class: "city-row muted",
                span { "{missed_text}" }
            }
            button {
                class: "city-btn wide",
                disabled: !confession_ready
                    || view.gold + 1e-9 < balance::CHURCH_CONFESSION_PRICE,
                onclick: move |_| {
                    let outcome = {
                        let mut guard = state.write();
                        city::confess(&mut guard)
                    };
                    match outcome {
                        Ok(()) => message.set(i18n::t("church.confessed").to_string()),
                        Err(error) => message.set(error.text().to_string()),
                    }
                    refresh_view(state);
                },
                "{confess_text}"
            }
        }
        if !message_text.is_empty() {
            div { class: "city-msg", "{message_text}" }
        }
    }
}

/// Гильдия: вступление и улучшения лавки.
#[component]
fn GuildScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);

    let sales = view
        .city
        .shop
        .as_ref()
        .map(|shop| shop.total_sales)
        .unwrap_or(0);
    let member = view.city.guild_unlocked;
    let can_join =
        sales >= balance::GUILD_SALES_REQUIRED && view.gold + 1e-9 >= balance::GUILD_ENTRY_GOLD;
    let need_text = i18n::tf(
        "guild.need",
        &[
            &balance::GUILD_SALES_REQUIRED.to_string(),
            &money(balance::GUILD_ENTRY_GOLD),
        ],
    );
    let sales_text = i18n::tf("guild.sales", &[&sales.to_string()]);
    let rank_text = i18n::tf(
        "guild.rank",
        &[
            &city::guild_rank(&view).to_string(),
            &balance::GUILD_RANK_MAX.to_string(),
        ],
    );
    let title_text = i18n::tf("guild.title_label", &[city::title_name(sales)]);

    let upgrades: Vec<(&'static str, String, String, f64, bool)> = city::available_upgrades(&view)
        .into_iter()
        .map(|def| {
            (
                def.id,
                i18n::t(&format!("guild.upgrade.{}.name", def.id)).to_string(),
                i18n::t(&format!("guild.upgrade.{}.desc", def.id)).to_string(),
                def.price,
                city::has_upgrade(&view, def.id),
            )
        })
        .collect();

    // Купленные улучшения больше не показываются: каждая строка — следующее
    // недокупленное звено цепочки. Когда покупать нечего, показываем заметку.
    let upgrades_done = upgrades.is_empty();

    rsx! {
        if member {
            div { class: "city-card",
                div { class: "city-row",
                    span { "{rank_text}" }
                    span { "{title_text}" }
                }
                div { class: "city-row muted",
                    span { "{sales_text}" }
                }
            }
            div { class: "city-card",
                div { class: "city-row muted",
                    span { "{i18n::t(\"guild.upgrades\")}" }
                }
                if upgrades_done {
                    div { class: "city-row muted",
                        span { "{i18n::t(\"guild.upgrades_done\")}" }
                    }
                }
                for (id , name , desc , price , bought) in upgrades {
                    GuildUpgradeRow {
                        key: "{id}",
                        state,
                        id: id.to_string(),
                        name,
                        desc,
                        price,
                        bought,
                    }
                }
            }
        } else {
            div { class: "city-card",
                div { class: "city-row",
                    span { "{sales_text}" }
                    span { "{need_text}" }
                }
                button {
                    class: "city-btn",
                    disabled: !can_join,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::join_guild(&mut guard)
                        };
                        match outcome {
                            Ok(()) => message.set(i18n::t("guild.joined").to_string()),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"guild.join\")}"
                }
                if !message().is_empty() {
                    div { class: "city-msg", "{message()}" }
                }
            }
        }
    }
}

/// Одно улучшение гильдии.
#[component]
fn GuildUpgradeRow(
    state: Signal<GameState>,
    id: String,
    name: String,
    desc: String,
    price: f64,
    bought: bool,
) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    let price_text = money(price);

    rsx! {
        div { class: "city-upgrade",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{name}" }
                span { class: "city-row-muted", "{price_text}" }
            }
            div { class: "city-row muted",
                span { "{desc}" }
            }
            if !bought {
                button {
                    class: "city-btn small",
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::buy_upgrade(&mut guard, &id)
                        };
                        match outcome {
                            Ok(()) => message.set(String::new()),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"guild.buy\")}"
                }
            }
            if !message().is_empty() {
                div { class: "city-msg", "{message()}" }
            }
        }
    }
}

/// Дом странников: стройка до постройки, потом сами путешественники.
#[component]
fn TravelersHouseScreen(state: Signal<GameState>, view: GameState) -> Element {
    let built = city::traveler_house_built(&view);

    if built {
        return rsx! {
            TravelerList { state, view: view.clone(), with_gear: true }
        };
    }

    // Земля арендуется только по приглашению друзей: без аренды эта
    // дверь ещё закрыта, и показывать стройку нечего.
    if !city::traveler_house_land(&view) {
        return rsx! {
            div { class: "city-card",
                div { class: "city-row muted",
                    span { "{i18n::t(\"err.traveler_house_not_offered\")}" }
                }
            }
        };
    }

    let mut state = state;
    let mut message = use_signal(String::new);
    let stage = view.city.traveler_house_stage;
    let building = city::traveler_house_building(&view);
    let left = (view.city.traveler_house_ends_at - view.total_play_seconds).max(0.0);
    let left_text = i18n::tf("traveler_house.building", &[&format_duration(left)]);
    let cost = balance::traveler_house_cost(stage.min(balance::TRAVELER_HOUSE_STAGES));
    let can_build = !building && actions::can_afford(&view, cost);
    let stage_names = [
        "traveler_house.foundation",
        "traveler_house.walls",
        "traveler_house.roof",
    ];
    let stage_name = stage_names
        .get(stage as usize)
        .map(|key| i18n::t(key).to_string())
        .unwrap_or_else(|| i18n::t("traveler_house.roof").to_string());
    let stage_text = i18n::tf(
        "traveler_house.stage",
        &[
            &(stage + 1).min(balance::TRAVELER_HOUSE_STAGES).to_string(),
            &balance::TRAVELER_HOUSE_STAGES.to_string(),
        ],
    );

    rsx! {
        div { class: "city-card",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{stage_name}" }
                span { class: "city-row-muted", "{stage_text}" }
            }
            CostLabel { view: view.clone(), cost }
            if building {
                div { class: "city-msg", "{left_text}" }
            } else {
                button {
                    class: "city-btn",
                    disabled: !can_build,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::start_traveler_house_stage(&mut guard)
                        };
                        if let Err(error) = outcome {
                            message.set(error.text().to_string());
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"traveler_house.build\")}"
                }
            }
            if !message().is_empty() {
                div { class: "city-msg", "{message()}" }
            }
        }
    }
}
/// Ратуша: статистика лавки, топ товаров и история продаж.
#[component]
fn TownHallScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    // Две вкладки: обзор лавки и налог с зарплатой рабочим.
    let mut tab = use_signal(|| 0usize);
    let shop = view.city.shop.clone().unwrap_or_default();
    let sales = shop.total_sales;
    let sales_text = i18n::tf("town_hall.sales", &[&sales.to_string()]);
    let rating_text = i18n::tf("shop.rating", &[&format!("{:.2}", shop.rating)]);

    if !view.city.town_hall_unlocked {
        let locked_text = i18n::tf(
            "town_hall.locked",
            &[
                &balance::TOWN_HALL_SALES_REQUIRED.to_string(),
                &format_num(balance::TOWN_HALL_MIN_RATING as f64),
            ],
        );
        return rsx! {
            div { class: "city-card",
                p { class: "city-hint", "{locked_text}" }
                div { class: "city-row",
                    span { "{sales_text}" }
                    span { "{rating_text}" }
                }
            }
        };
    }

    let profit_text = i18n::tf("town_hall.profit", &[&money(shop.total_profit)]);
    let avg_text = i18n::tf("town_hall.avg_check", &[&money(city::average_check(&shop))]);
    let taxes_text = i18n::tf(
        "town_hall.taxes",
        &[
            &view.city.taxes_paid.to_string(),
            &view.city.taxes_missed.to_string(),
        ],
    );
    let title_text = i18n::tf("town_hall.title_label", &[city::title_name(sales)]);

    let top: Vec<(String, u32)> = city::top_goods(&shop, 10)
        .into_iter()
        .map(|(item_id, quantity)| (ingredient_name(&item_id).to_string(), quantity))
        .collect();
    let history: Vec<String> = shop
        .sales
        .iter()
        .rev()
        .take(10)
        .map(|sale| {
            let name = ingredient_name(&sale.item_id);
            let star = if sale.vip { "★ " } else { "" };
            format!("{star}{name} ×{} — {}", sale.quantity, money(sale.total))
        })
        .collect();

    let charter_cost = money(balance::CHARTER_COST);
    let charter_buy = i18n::tf("charter.buy", &[&charter_cost]);
    let can_buy_charter = view.gold + 1e-9 >= balance::CHARTER_COST;
    let first_permit_buy = i18n::tf(
        "smith.first_permit.buy",
        &[&money(balance::SMITH_FIRST_PERMIT_GOLD)],
    );
    let can_buy_first_permit = view.gold + 1e-9 >= balance::SMITH_FIRST_PERMIT_GOLD;
    let permit_buy = i18n::tf(
        "smith.permit.buy",
        &[&money(balance::SMITH_SECOND_PERMIT_GOLD)],
    );
    let can_buy_permit = view.gold + 1e-9 >= balance::SMITH_SECOND_PERMIT_GOLD;
    let rent_text = i18n::tf(
        "traveler_house.rent",
        &[&money(balance::TRAVELER_HOUSE_RENT)],
    );
    let house_built = city::traveler_house_built(&view);
    let house_stage = view.city.traveler_house_stage;
    let house_progress_text = format!("{}/{}", house_stage, balance::TRAVELER_HOUSE_STAGES);
    let house_hint = if house_built {
        i18n::t("traveler_house.built").to_string()
    } else if city::traveler_house_building(&view) {
        i18n::tf(
            "traveler_house.building",
            &[&format_duration(
                (view.city.traveler_house_ends_at - view.total_play_seconds).max(0.0),
            )],
        )
    } else {
        i18n::t("traveler_house.rented").to_string()
    };

    // Вкладка «Налог»: налог магазина и зарплата рабочим отдельными строками.
    let tax_shop_percent = i18n::tf(
        "town_hall.tax_shop",
        &[&format_num(balance::CITY_TAX_PERCENT * 100.0)],
    );
    let cycle_revenue: f64 = shop
        .sales
        .iter()
        .filter(|sale| sale.timestamp >= view.total_play_seconds - balance::CITY_TAX_INTERVAL)
        .map(|sale| sale.total)
        .sum();
    let shop_tax_line = i18n::tf(
        "town_hall.tax_shop_line",
        &[&money(cycle_revenue * balance::CITY_TAX_PERCENT)],
    );
    let workers_line = i18n::tf(
        "town_hall.salary_workers",
        &[&money(city::production_salary(&view))],
    );
    let salary_line = i18n::tf(
        "town_hall.salary_total",
        &[&money(city::salary_total(&view))],
    );
    let salary_next = match city::salary_due_in(&view) {
        Some(left) => i18n::tf("town_hall.salary_next", &[&format_duration(left)]),
        None => String::new(),
    };

    rsx! {
        div { class: "tavern-tabs",
            button {
                class: if tab() == 0 { "city-tab active" } else { "city-btn small" },
                onclick: move |_| tab.set(0),
                "{i18n::t(\"town_hall.tab_overview\")}"
            }
            button {
                class: if tab() == 1 { "city-tab active" } else { "city-btn small" },
                onclick: move |_| tab.set(1),
                "{i18n::t(\"town_hall.tab_tax\")}"
            }
        }

        if tab() == 1 {
            div { class: "city-card",
                div { class: "city-row",
                    span { "{tax_shop_percent}" }
                }
                div { class: "city-row",
                    span { "{shop_tax_line}" }
                }
                div { class: "city-row muted",
                    span { "{taxes_text}" }
                }
            }
            div { class: "city-card",
                div { class: "city-row",
                    span { "{workers_line}" }
                }
                div { class: "city-row",
                    span { "{salary_line}" }
                }
                if !salary_next.is_empty() {
                    div { class: "city-row muted",
                        span { "{salary_next}" }
                    }
                }
                div { class: "city-row muted",
                    span { "{i18n::t(\"town_hall.salary_hint\")}" }
                }
            }
        } else {
        div { class: "city-card",
            div { class: "city-row",
                span { "{sales_text}" }
                span { "{profit_text}" }
            }
            div { class: "city-row",
                span { "{avg_text}" }
                span { "{title_text}" }
            }
            div { class: "city-row muted",
                span { "{taxes_text}" }
            }
        }

        div { class: "city-card",
            div { class: "city-row muted",
                span { "{i18n::t(\"town_hall.top\")}" }
            }
            if top.is_empty() {
                p { class: "city-hint", "{i18n::t(\"town_hall.no_sales\")}" }
            } else {
                for (index , (name , quantity)) in top.into_iter().enumerate() {
                    div { class: "city-row", key: "{index}",
                        span { "{name}" }
                        span { class: "city-row-muted", "×{quantity}" }
                    }
                }
            }
        }

        div { class: "city-card",
            div { class: "city-row muted",
                span { "{i18n::t(\"town_hall.history\")}" }
            }
            if history.is_empty() {
                p { class: "city-hint", "{i18n::t(\"town_hall.nothing\")}" }
            } else {
                // Ключ — номер строки: одинаковые продажи дают одинаковый
                // ключ, а повторяющиеся ключи ломают перерисовку списка.
                for (index , line) in history.into_iter().enumerate() {
                    div { class: "city-row muted", key: "{index}",
                        span { "{line}" }
                    }
                }
            }
        }

        div { class: "city-card",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{i18n::t(\"charter.name\")}" }
            }
            div { class: "city-row muted",
                span { "{i18n::t(\"charter.desc\")}" }
            }
            if view.city.hire_charter {
                div { class: "city-msg", "{i18n::t(\"charter.owned\")}" }
            } else {
                button {
                    class: "city-btn",
                    disabled: !can_buy_charter,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::buy_charter(&mut guard)
                        };
                        match outcome {
                            Ok(()) => message.set(i18n::t("charter.owned").to_string()),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{charter_buy}"
                }
            }
        }

        div { class: "city-card",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{i18n::t(\"smith.first_permit.name\")}" }
            }
            div { class: "city-row muted",
                span { "{i18n::t(\"smith.first_permit.desc\")}" }
            }
            if view.city.smith_first_permit {
                div { class: "city-msg", "{i18n::t(\"smith.first_permit.owned\")}" }
            } else {
                button {
                    class: "city-btn",
                    disabled: !can_buy_first_permit,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::buy_first_smith_permit(&mut guard)
                        };
                        match outcome {
                            Ok(()) => {
                                message.set(i18n::t("smith.first_permit.owned").to_string())
                            }
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{first_permit_buy}"
                }
            }
        }

        div { class: "city-card",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{i18n::t(\"smith.permit.name\")}" }
            }
            div { class: "city-row muted",
                span { "{i18n::t(\"smith.permit.desc\")}" }
            }
            if view.city.smith_second_permit {
                div { class: "city-msg", "{i18n::t(\"smith.permit.owned\")}" }
            } else {
                button {
                    class: "city-btn",
                    disabled: !can_buy_permit,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::buy_smith_permit(&mut guard)
                        };
                        match outcome {
                            Ok(()) => message.set(i18n::t("smith.permit.owned").to_string()),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{permit_buy}"
                }
            }
        }

        div { class: "city-card",
            div { class: "city-row",
                span { class: "city-upgrade-name", "{i18n::t(\"traveler_house.title\")}" }
            }
            div { class: "city-row muted",
                span { "{i18n::t(\"traveler_house.hint\")}" }
            }
            if view.city.traveler_house_rented && !house_built {
                div { class: "city-row traveler-loc closed",
                    span { class: "city-upgrade-name", "{i18n::t(\"traveler_house.title\")}" }
                    span { class: "city-row-muted", "{house_progress_text}" }
                    span { class: "city-row-muted", "{house_hint}" }
                }
            }
            if house_built {
                div { class: "city-msg", "{i18n::t(\"traveler_house.built\")}" }
            } else if !view.city.traveler_house_rented {
                if city::traveler_house_offer_made(&view) {
                    button {
                        class: "city-btn",
                        disabled: view.gold + 1e-9 < balance::TRAVELER_HOUSE_RENT,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::rent_traveler_house(&mut guard)
                            };
                            match outcome {
                                Ok(()) => message.set(i18n::t("traveler_house.rented").to_string()),
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        "{rent_text}"
                    }
                } else {
                    // Без согласия друзей Ратуша землю не выделяет.
                    div { class: "city-row muted",
                        span { "{i18n::t(\"err.traveler_house_not_offered\")}" }
                    }
                }
            }
        }
        // Одно сообщение на всю ратушу: раньше одинаковый текст висел
        // под каждым блоком (грамота, разрешения, дом).
        if !message().is_empty() {
            div { class: "city-msg", "{message()}" }
        }
        }
    }
}

/// Таверна: слухи, путешественники и игра в кости.
#[component]
fn TavernScreen(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    let mut bet = use_signal(|| 100.0);
    let mut tab = use_signal(|| 0usize);
    let now = view.total_play_seconds;
    let cooldown = (view.city.next_rumor_at - now).max(0.0);
    let rumor_text = i18n::tf("tavern.rumor", &[&money(balance::TAVERN_RUMOR_PRICE)]);
    let cooldown_text = if cooldown > 0.0 {
        i18n::tf("tavern.cooldown", &[&format_duration(cooldown)])
    } else {
        String::new()
    };
    let active_rumor = match city::active_event(&view) {
        Some(CityEvent::Rumor {
            item_id, ends_at, ..
        }) => Some(i18n::tf(
            "tavern.rumor_active",
            &[ingredient_name(item_id), &format_duration(ends_at - now)],
        )),
        _ => None,
    };
    let active_rumor_text = active_rumor.unwrap_or_else(|| i18n::t("tavern.no_rumor").to_string());
    let bet_value = bet();
    let bet_text = i18n::tf("tavern.dice_bet", &[&money(bet_value)]);
    let house_built = city::traveler_house_built(&view);
    let free_count = view
        .city
        .travelers
        .iter()
        .filter(|traveler| city::traveler_ready(traveler))
        .count();
    if house_built && tab() == 1 {
        // Дом построен — вкладка путешественников уехала в Дом.
        tab.set(0);
    }
    let message_text = message();

    rsx! {
        div { class: "tavern-tabs",
            button {
                class: if tab() == 0 { "city-tab active" } else { "city-btn small" },
                onclick: move |_| tab.set(0),
                "{i18n::t(\"tavern.tab_rumors\")}"
            }
            if !view.city.travelers.is_empty() && !house_built {
                button {
                    class: if tab() == 1 { "city-tab active" } else { "city-btn small" },
                    onclick: move |_| tab.set(1),
                    "{i18n::tf(\"tavern.tab_travelers\", &[&free_count.to_string(), &balance::TRAVELER_TARGET_COUNT.to_string()])}"
                }
            }
        }

        if tab() == 0 {
            div { class: "city-card",
                button {
                    class: "city-btn",
                    disabled: cooldown > 0.0,
                    onclick: move |_| {
                        let outcome = {
                            let mut guard = state.write();
                            city::buy_rumor(&mut guard)
                        };
                        match outcome {
                            Ok(pair) => {
                                message
                                    .set(
                                        i18n::tf(
                                            "event.rumor.desc",
                                            &[ingredient_name(&pair.dead), ingredient_name(&pair.hot)],
                                        ),
                                    )
                            }
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{rumor_text}"
                }
                if !cooldown_text.is_empty() {
                    div { class: "city-row muted",
                        span { "{cooldown_text}" }
                    }
                }
                div { class: "city-row muted",
                    span { "{active_rumor_text}" }
                }
                if !message_text.is_empty() {
                    div { class: "city-msg", "{message_text}" }
                }
            }

            div { class: "city-card",
                div { class: "city-row muted",
                    span { "{i18n::t(\"tavern.dice\")}" }
                }
                div { class: "city-row",
                    span { "{bet_text}" }
                }
                div { class: "city-bet-row",
                    for amount in [100.0, 500.0, 1000.0] {
                        button {
                            class: if (bet_value - amount).abs() < 0.5 { "city-tab active" } else { "city-btn small" },
                            onclick: move |_| bet.set(amount),
                            "{format_num(amount)}"
                        }
                    }
                }
                button {
                    class: "city-btn",
                    disabled: view.gold + 1e-9 < bet_value,
                    onclick: move |_| {
                        let current = bet();
                        let outcome = {
                            let mut guard = state.write();
                            city::play_dice(&mut guard, current)
                        };
                        match outcome {
                            Ok(result) => message.set(dice_text(&result)),
                            Err(error) => message.set(error.text().to_string()),
                        }
                        refresh_view(state);
                    },
                    "{i18n::t(\"tavern.roll\")}"
                }
            }
        } else {
            TravelerList { state, view: view.clone(), with_gear: false }
        }
    }
}

/// Список путешественников с раскрывающимися карточками.
///
/// `with_gear` включается только в Доме странников: там жители стоят
/// квадратами, как в сумке, а снаряжение можно надевать лишь там.
#[component]
fn TravelerList(state: Signal<GameState>, view: GameState, with_gear: bool) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    let mut open = use_signal(|| Option::<usize>::None);
    // Письмо о возвращении раскрывает нужную карточку: фокус приходит
    // из письма в колокольчике и срабатывает один раз.
    let focus = use_context::<TravelerFocus>().0;
    use_effect(move || {
        if let Some(index) = focus() {
            open.set(Some(index));
            focus.clone().set(None);
        }
    });
    let house_built = city::traveler_house_built(&view);
    let travelers = travelers_view(&view, open(), with_gear);
    let free_count = view
        .city
        .travelers
        .iter()
        .filter(|traveler| city::traveler_ready(traveler))
        .count();
    let counter_text = i18n::tf(
        "traveler.counter",
        &[
            &free_count.to_string(),
            &view.city.travelers.len().to_string(),
        ],
    );
    let away_text = i18n::t("traveler.all_away").to_string();
    // Пустой список — «все ушли» только когда они вообще завелись.
    let away_lines: Vec<String> = if !travelers.is_empty() && travelers.iter().all(|t| !t.ready) {
        view.city
            .travelers
            .iter()
            .map(|traveler| {
                i18n::tf(
                    "traveler.will_back",
                    &[
                        city::traveler_name(traveler.profile_id),
                        &format_duration((traveler.returns_at - view.total_play_seconds).max(0.0)),
                    ],
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let house_hint_text = if house_built {
        i18n::t("traveler.house_built").to_string()
    } else {
        i18n::t("traveler.house_hint").to_string()
    };
    // В Доме мест всего пять: когда они кончились, квадрат «+» гаснет.
    let can_hire = with_gear && view.city.travelers.len() < city::TRAVELER_PROFILES.len();
    let hire_price = city::traveler_hire_price(&view);
    let hire_text = i18n::tf("traveler.hire_price", &[&money(hire_price)]);
    let full_text = i18n::t("traveler.house_full").to_string();
    let message_text = message();

    rsx! {
        div { class: "city-card",
            if !with_gear {
                div { class: "city-topbar",
                    span { class: "city-upgrade-name", "{counter_text}" }
                    span { class: "city-row-muted", "{i18n::t(\"traveler.house_ask\")}" }
                }
                div { class: "city-row muted",
                    span { "{house_hint_text}" }
                }
            } else {
                div { class: "city-row",
                    span { class: "city-upgrade-name", "{counter_text}" }
                }
            }
            if travelers.is_empty() {
                p { class: "city-hint", "{i18n::t(\"traveler.none\")}" }
            } else if with_gear {
                // Дом странников: квадраты жителей и «+» для найма.
                div { class: "traveler-grid",
                    for traveler in travelers.iter() {
                        {
                            let index = traveler.index;
                            let name = traveler.name.clone();
                            let status = traveler.status.clone();
                            let ready = traveler.ready;
                            let open_now = traveler.info.is_some();
                            rsx! {
                                button {
                                    class: if open_now { "traveler-tile active" } else { "traveler-tile" },
                                    key: "{index}",
                                    onclick: move |_| {
                                        let current = open();
                                        open.set(if current == Some(index) { None } else { Some(index) });
                                    },
                                    span { class: "traveler-face", "👤" }
                                    span { class: "traveler-tile-name", "{name}" }
                                    span {
                                        class: if ready { "traveler-tile-status" } else { "traveler-tile-status away" },
                                        "{status}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "traveler-hire",
                        button {
                            class: if can_hire { "traveler-tile hire" } else { "traveler-tile hire full" },
                            disabled: !can_hire,
                            onclick: move |_| {
                                let outcome = {
                                    let mut guard = state.write();
                                    city::hire_traveler(&mut guard)
                                };
                                match outcome {
                                    Ok(()) => message.set(String::new()),
                                    Err(error) => message.set(error.text().to_string()),
                                }
                                refresh_view(state);
                            },
                            "+"
                        }
                        span { class: "traveler-hire-price",
                            if can_hire {
                                "{hire_text}"
                            } else {
                                "{full_text}"
                            }
                        }
                    }
                }
                for traveler in travelers {
                    if let Some(card) = traveler.info {
                        TravelerCard { state, index: traveler.index, card }
                    }
                }
            } else {
                for traveler in travelers {
                    {
                        let index = traveler.index;
                        let name = traveler.name;
                        let specialism = traveler.specialism;
                        let status = traveler.status;
                        let ready = traveler.ready;
                        let advance = traveler.advance_text;
                        let info = traveler.info;
                        let open_now = info.is_some();
                        rsx! {
                            div { class: "traveler-block", key: "{index}",
                                button {
                                    class: "traveler-main traveler-name-btn",
                                    onclick: move |_| {
                                        let current = open();
                                        open.set(if current == Some(index) { None } else { Some(index) });
                                    },
                                    span { class: "traveler-face", "👤" }
                                    span { class: "traveler-names",
                                        span { class: "city-upgrade-name", "{name}" }
                                        span { class: "city-row-muted", "{specialism}" }
                                    }
                                    span { class: if ready { "traveler-chip" } else { "traveler-chip away" }, "{status}" }
                                }
                                div { class: "traveler-main traveler-actions",
                                    span { class: "city-row-muted traveler-advance", "{advance}" }
                                }
                                if open_now {
                                    if let Some(card) = info {
                                        TravelerCard { state, index, card }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !away_lines.is_empty() {
                div { class: "city-row muted",
                    span { "{away_text}" }
                }
                for line in away_lines {
                    div { class: "city-row muted",
                        span { "{line}" }
                    }
                }
            }
            if !message_text.is_empty() {
                div { class: "city-msg", "{message_text}" }
            }
        }
    }
}

/// Окно-приглашение друга-путешественника. Кнопка «Да» перебрасывает
/// в Ратушу, где живёт аренда земли под Дом странников.
///
/// `overlay: false` — окно стоит внутри экрана города и не закрывает всё
/// приложение: тогда под ним ничего не прокручивается.
#[component]
pub fn TravelerHouseModal(
    state: Signal<GameState>,
    city_view: Signal<CityView>,
    #[props(default = true)] overlay: bool,
) -> Element {
    let mut state = state;
    let mut city_view = city_view;
    let blocker = if overlay {
        "notif-overlay"
    } else {
        "notif-overlay city-modal-inline"
    };
    rsx! {
        div { class: "{blocker}",
            div { class: "notif-panel offer-modal",
                div { class: "notif-head",
                    h2 { class: "notif-title", "{i18n::t(\"traveler.house_offer_title\")}" }
                }
                p { class: "letter-body", "{i18n::t(\"traveler.house_offer\")}" }
                div { class: "offer-actions",
                    button {
                        class: "offer-accept",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::traveler_house_decision(&mut guard, true);
                            }
                            refresh_view(state);
                            city_view.set(CityView::TownHall);
                        },
                        "{i18n::t(\"traveler.house_yes\")}"
                    }
                    button {
                        class: "offer-later",
                        onclick: move |_| {
                            {
                                let mut guard = state.write();
                                city::traveler_house_decision(&mut guard, false);
                            }
                            refresh_view(state);
                            city_view.set(CityView::TownHall);
                        },
                        "{i18n::t(\"traveler.house_later\")}"
                    }
                }
            }
        }
    }
}

/// Результат броска костей.
fn dice_text(result: &DiceResult) -> String {
    match result {
        DiceResult::Win => i18n::t("tavern.dice_win").to_string(),
        DiceResult::Draw => i18n::t("tavern.dice_draw").to_string(),
        DiceResult::Lose => i18n::t("tavern.dice_lose").to_string(),
    }
}

/// Что может выпасть в локации и с каким шансом.
///
/// Игрок просил показать это прямо в блоке направления, чтобы не гадать,
/// зачем платить аванс.
fn traveler_loot_lines(location: &balance::TravelerLocationDef) -> Vec<String> {
    let total: u64 = location.loot.iter().map(|row| row.weight as u64).sum();
    if total == 0 {
        return Vec::new();
    }
    location
        .loot
        .iter()
        .map(|row| {
            let percent = row.weight as f64 / total as f64 * 100.0;
            let name = ingredient_name(row.item_id);
            let quantity = if row.min_quantity == row.max_quantity {
                row.min_quantity.to_string()
            } else {
                format!("{}–{}", row.min_quantity, row.max_quantity)
            };
            i18n::tf(
                "traveler.loot_line",
                &[name, &quantity, &format!("{percent:.0}")],
            )
        })
        .collect()
}

/// Содержимое раскрытой карточки путешественника.
fn traveler_card(
    view: &GameState,
    traveler: &city::Traveler,
    _index: usize,
    with_gear: bool,
) -> TravelerCardView {
    // Этап странника: походы до потери компаса, просьба о компасе или уже
    // предложенный Дом.
    let (stage_text, compass_wanted, compass_ready) = match city::TravelerStage::of(view, traveler)
    {
        city::TravelerStage::Trips { left } => (
            i18n::tf("traveler.stage_trips", &[&left.to_string()]),
            false,
            false,
        ),
        city::TravelerStage::Compass => (
            i18n::t("traveler.stage_compass").to_string(),
            true,
            view.item_count(city::TRAVELER_COMPASS_ITEM) > 0,
        ),
        city::TravelerStage::HouseOffered => {
            (i18n::t("traveler.stage_house").to_string(), false, false)
        }
    };
    let can_go = city::traveler_ready(traveler)
        && !compass_wanted
        && traveler.carried.is_empty()
        && traveler.payout <= 1e-9;
    let locations = balance::TRAVELER_LOCATIONS
        .iter()
        .map(|location| {
            let closed = location.needs_house && !city::traveler_house_built(view);
            TravelerTripView {
                id: location.id.to_string(),
                name: i18n::t(&format!("traveler.loc.{}", location.id)).to_string(),
                time: i18n::tf("traveler.minutes", &[&format_num(location.minutes)]),
                advance: i18n::tf("traveler.advance", &[&money(location.advance)]),
                closed,
                can_send: !closed && can_go && view.gold + 1e-9 >= location.advance,
                loot_lines: traveler_loot_lines(location),
            }
        })
        .collect();
    let loot: Vec<TravelerLootView> = traveler
        .carried
        .iter()
        .map(|(item_id, count)| TravelerLootView {
            item_id: item_id.clone(),
            count: *count,
        })
        .collect();
    let payout_text = if traveler.payout > 0.5 {
        let share = city::traveler_loot_value(&traveler.carried);
        let percent = if share > 0.0 {
            (traveler.payout / share * 100.0).round() as i64
        } else {
            15
        };
        i18n::tf(
            "traveler.payout",
            &[&money(traveler.payout), &percent.to_string()],
        )
    } else {
        String::new()
    };
    let claim_ready = !traveler.carried.is_empty() || traveler.payout > 1e-9;
    let gear = if with_gear {
        items::EquipSlot::ALL
            .iter()
            .enumerate()
            .map(|(slot, equip_slot)| {
                let item_id = traveler.gear_at(slot);
                TravelerGearView {
                    slot,
                    label: equip_slot.label().to_string(),
                    short: super::inventory::slot_short(*equip_slot).to_string(),
                    item: item_id.and_then(items::item).cloned(),
                    bag_count: item_id.map(|id| view.item_count(id)).unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    // Для каждого квадрата — только те вещи из сумки, что в него встают.
    let mut gear_picks: Vec<Vec<TravelerGearPickView>> =
        vec![Vec::new(); items::EquipSlot::ALL.len()];
    if with_gear {
        let mut pool: Vec<TravelerGearPickView> = view
            .inventory
            .iter()
            .filter(|(_, count)| **count > 0)
            .filter_map(|(id, count)| {
                items::item(id).and_then(|item| {
                    item.slot.map(|slot| TravelerGearPickView {
                        item_id: id.clone(),
                        name: item.name().to_string(),
                        slot,
                        count: *count,
                    })
                })
            })
            .collect();
        pool.sort_by(|left, right| left.name.cmp(&right.name));
        for (slot, equip_slot) in items::EquipSlot::ALL.iter().enumerate() {
            gear_picks[slot] = pool
                .iter()
                .filter(|pick| traveler_gear_fits(pick.slot, *equip_slot))
                .cloned()
                .collect();
        }
    }
    let set_pieces = traveler
        .gear
        .iter()
        .flatten()
        .filter(|item_id| {
            items::item(item_id)
                .and_then(|item| item.stats.set_id.as_deref())
                .map(|set_id| set_id == balance::TRAVELER_SET_ID)
                .unwrap_or(false)
        })
        .count();
    let set_active = set_pieces >= balance::TRAVELER_SET_PIECES;
    let set_text = if set_active {
        i18n::t("set.traveler.active").to_string()
    } else {
        i18n::tf(
            "set.traveler.partial",
            &[
                &set_pieces.to_string(),
                &balance::TRAVELER_SET_PIECES.to_string(),
            ],
        )
    };
    let gear_hint = if city::traveler_house_built(view) {
        i18n::t("traveler.gear_hint").to_string()
    } else {
        i18n::t("traveler.house_hint").to_string()
    };
    TravelerCardView {
        specialism_text: city::traveler_specialism(traveler.profile_id).to_string(),
        advance_text: i18n::tf(
            "traveler.advance_mid",
            &[&money(balance::TRAVELER_LOCATIONS[0].advance)],
        ),
        stage_text,
        compass_wanted,
        compass_ready,
        locations,
        loot,
        payout_text,
        claim_ready,
        empty_trip: traveler.last_empty && !claim_ready,
        gear,
        gear_picks,
        set_text,
        set_active,
        set_pieces: set_pieces as u32,
        gear_hint,
    }
}

/// Встаёт ли вещь в квадрат снаряжения: аксессуары — в любой из двух.
fn traveler_gear_fits(item_slot: items::EquipSlot, target: items::EquipSlot) -> bool {
    if item_slot == target {
        return true;
    }
    let accessory = matches!(
        target,
        items::EquipSlot::Accessory1 | items::EquipSlot::Accessory2
    );
    accessory && item_slot == items::EquipSlot::Accessory1
}

/// Снаряжение в заданном квадрате; пустой слот — с подписью.
fn gear_at(card: &TravelerCardView, slot: items::EquipSlot) -> TravelerGearView {
    card.gear
        .iter()
        .find(|gear| gear.slot == slot.index())
        .cloned()
        .unwrap_or(TravelerGearView {
            slot: slot.index(),
            label: slot.label().to_string(),
            short: slot_short(slot).to_string(),
            item: None,
            bag_count: 0,
        })
}

/// Надевает вещь из сумки в выбранный квадрат снаряжения странника.
fn equip_traveler_item(
    state: Signal<GameState>,
    index: usize,
    slot: usize,
    item_id: String,
    message: Signal<String>,
    gear_mode: Signal<u8>,
) {
    let mut state = state;
    let mut message = message;
    let mut gear_mode = gear_mode;
    let outcome = {
        let mut guard = state.write();
        city::equip_traveler_gear(&mut guard, index, slot, &item_id)
    };
    match outcome {
        Ok(()) => gear_mode.set(0),
        Err(error) => message.set(error.text().to_string()),
    }
    refresh_view(state);
}

/// Квадрат снаряжения странника — как `EquipmentSlot` в сумке героя.
#[component]
fn TravelerGearCell(
    gear: TravelerGearView,
    active: bool,
    slot_signal: Signal<usize>,
    mode_signal: Signal<u8>,
    pick_signal: Signal<Option<String>>,
) -> Element {
    let mut slot_signal = slot_signal;
    let mut mode_signal = mode_signal;
    let mut pick_signal = pick_signal;
    let slot = gear.slot;
    let worn = gear.item.is_some();
    let class = if active {
        "equip-cell selected"
    } else {
        "equip-cell"
    };
    let label = gear.label.clone();
    let short = gear.short.clone();
    let item = gear.item.clone();

    rsx! {
        button {
            class: "{class}",
            onclick: move |_| {
                // Повторный тап по открытому квадрату закрывает карточку.
                if slot_signal() == slot && mode_signal() != 0 {
                    mode_signal.set(0);
                    return;
                }
                slot_signal.set(slot);
                mode_signal.set(if worn { 2 } else { 1 });
                pick_signal.set(None);
            },
            div { class: "equip-cell-icon",
                if let Some(item) = item.clone() {
                    ItemIcon { item, class: "equip-icon".to_string() }
                } else {
                    "{short}"
                }
            }
            div { class: "equip-cell-label", "{label}" }
            if let Some(item) = item.clone() {
                div { class: "equip-cell-name", "{item.name()}" }
            } else {
                div { class: "equip-cell-name empty", "{i18n::t(\"bag.empty_slot\")}" }
            }
        }
    }
}

/// Карточка путешественника под его строкой.
#[component]
fn TravelerCard(state: Signal<GameState>, index: usize, card: TravelerCardView) -> Element {
    let mut state = state;
    let mut message = use_signal(String::new);
    // Какой квадрат снаряжения открыт: 0 — ничего, 1 — сумка, 2 — надетое.
    let gear_slot = use_signal(|| 0usize);
    let mut gear_mode = use_signal(|| 0u8);
    let mut pick_item = use_signal(|| None::<String>);
    let mut loot_open = use_signal(|| None::<String>);
    let message_text = message();
    // Вещи из сумки, которые встают в выбранный квадрат снаряжения.
    let bag_picks: Vec<TravelerGearPickView> = card
        .gear_picks
        .get(gear_slot())
        .cloned()
        .unwrap_or_default();
    rsx! {
        div { class: "traveler-card",
            div { class: "city-row",
                span { class: "city-row-muted", "{card.specialism_text}" }
                span { class: "city-row-muted", "{card.advance_text}" }
            }
            div { class: "traveler-stage",
                span { class: "city-row-muted", "{card.stage_text}" }
                if card.compass_wanted {
                    button {
                        class: "city-btn",
                        disabled: !card.compass_ready,
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::deliver_compass(&mut guard, index)
                            };
                            match outcome {
                                Ok(()) => {
                                    message.set(i18n::t("traveler.compass_given").to_string())
                                }
                                Err(error) => message.set(error.text().to_string()),
                            }
                            refresh_view(state);
                        },
                        if card.compass_ready {
                            "{i18n::t(\"traveler.give_compass\")}"
                        } else {
                            "{i18n::t(\"traveler.need_compass\")}"
                        }
                    }
                }
            }
            if card.claim_ready {
                div { class: "traveler-loot",
                    // Добыча — квадратами, как в сумке: иконка и количество
                    // в правом верхнем углу. Тап открывает описание вещи.
                    if !card.loot.is_empty() {
                        div { class: "inv-grid traveler-loot-grid",
                            for loot in card.loot.iter() {
                                {
                                    let item_id = loot.item_id.clone();
                                    let selected = loot_open() == Some(item_id.clone());
                                    rsx! {
                                        button {
                                            class: if selected { "inv-item selected" } else { "inv-item" },
                                            onclick: move |_| {
                                                let next = if loot_open() == Some(item_id.clone()) {
                                                    None
                                                } else {
                                                    Some(item_id.clone())
                                                };
                                                loot_open.set(next);
                                            },
                                            if let Some(item) = items::item(&loot.item_id) {
                                                ItemIcon {
                                                    item: item.clone(),
                                                    class: "inv-item-icon".to_string(),
                                                }
                                            }
                                            span { class: "inv-badge", "{loot.count}" }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(item_id) = loot_open() {
                            if let Some(item) = items::item(&item_id) {
                                {
                                    let count = card
                                        .loot
                                        .iter()
                                        .find(|loot| loot.item_id == item_id)
                                        .map(|loot| loot.count)
                                        .unwrap_or(1);
                                    rsx! {
                                        ItemDetails {
                                            item: item.clone(),
                                            count,
                                            set_pieces: 0,
                                            on_close: move |_| loot_open.set(None),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !card.payout_text.is_empty() {
                        span { class: "city-row-muted", "{card.payout_text}" }
                    }
                    button {
                        class: "city-btn",
                        onclick: move |_| {
                            let outcome = {
                                let mut guard = state.write();
                                city::claim_traveler_loot(&mut guard, index)
                            };
                            if let Err(error) = outcome {
                                message.set(error.text().to_string());
                            }
                            refresh_view(state);
                        },
                        "{i18n::t(\"traveler.claim\")}"
                    }
                }
            } else if card.empty_trip {
                div { class: "city-row muted",
                    span { "{i18n::t(\"traveler.empty_trip\")}" }
                }
            }
            if !card.gear.is_empty() {
                div { class: "traveler-gear",
                    div { class: "city-row muted",
                        span { "{i18n::t(\"traveler.gear\")}" }
                        span {
                            class: if card.set_active { "equip-set-status active" } else { "equip-set-status" },
                            "{card.set_text}"
                        }
                    }
                    // Снаряжение странника — та же сетка, что у героя в сумке:
                    // голова, аксессуар, тело и спина, руки, ноги, аксессуар.
                    div { class: "equip-grid",
                        TravelerGearCell {
                            gear: gear_at(&card, items::EquipSlot::Head),
                            active: gear_slot() == items::EquipSlot::Head.index(),
                            slot_signal: gear_slot,
                            mode_signal: gear_mode,
                            pick_signal: pick_item,
                        }
                        TravelerGearCell {
                            gear: gear_at(&card, items::EquipSlot::Accessory1),
                            active: gear_slot() == items::EquipSlot::Accessory1.index(),
                            slot_signal: gear_slot,
                            mode_signal: gear_mode,
                            pick_signal: pick_item,
                        }
                        div { class: "equip-pair",
                            TravelerGearCell {
                                gear: gear_at(&card, items::EquipSlot::Body),
                                active: gear_slot() == items::EquipSlot::Body.index(),
                                slot_signal: gear_slot,
                                mode_signal: gear_mode,
                                pick_signal: pick_item,
                            }
                            TravelerGearCell {
                                gear: gear_at(&card, items::EquipSlot::Back),
                                active: gear_slot() == items::EquipSlot::Back.index(),
                                slot_signal: gear_slot,
                                mode_signal: gear_mode,
                                pick_signal: pick_item,
                            }
                        }
                        TravelerGearCell {
                            gear: gear_at(&card, items::EquipSlot::Hand),
                            active: gear_slot() == items::EquipSlot::Hand.index(),
                            slot_signal: gear_slot,
                            mode_signal: gear_mode,
                            pick_signal: pick_item,
                        }
                        TravelerGearCell {
                            gear: gear_at(&card, items::EquipSlot::Feet),
                            active: gear_slot() == items::EquipSlot::Feet.index(),
                            slot_signal: gear_slot,
                            mode_signal: gear_mode,
                            pick_signal: pick_item,
                        }
                        TravelerGearCell {
                            gear: gear_at(&card, items::EquipSlot::Accessory2),
                            active: gear_slot() == items::EquipSlot::Accessory2.index(),
                            slot_signal: gear_slot,
                            mode_signal: gear_mode,
                            pick_signal: pick_item,
                        }
                    }
                    // Сумка игрока квадратиками: тап по вещи показывает её
                    // описание, «Надеть» кладёт вещь в выбранный квадрат.
                    if gear_mode() == 1 {
                        div { class: "traveler-gear-bag",
                            if bag_picks.is_empty() {
                                div { class: "city-row muted",
                                    span { "{i18n::t(\"traveler.gear_none\")}" }
                                }
                            } else {
                                div { class: "inv-grid",
                                    for pick in bag_picks.iter() {
                                        {
                                            let pick_id = pick.item_id.clone();
                                            let selected = pick_item() == Some(pick_id.clone());
                                            rsx! {
                                                if let Some(item) = items::item(&pick.item_id) {
                                                    ItemCell {
                                                        key: "{pick_id}",
                                                        item: item.clone(),
                                                        count: pick.count,
                                                        selected,
                                                        on_select: move |item_id: String| {
                                                            pick_item.set(Some(item_id));
                                                        },
                                                        on_double_click: move |item_id: String| {
                                                            equip_traveler_item(
                                                                state,
                                                                index,
                                                                gear_slot(),
                                                                item_id,
                                                                message,
                                                                gear_mode,
                                                            );
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Карточка выбранной вещи — под сеткой, как в сумке.
                        if let Some(item_id) = pick_item() {
                            if let Some(item) = items::item(&item_id) {
                                {
                                    let count = bag_picks
                                        .iter()
                                        .find(|pick| pick.item_id == item_id)
                                        .map(|pick| pick.count)
                                        .unwrap_or(1);
                                    rsx! {
                                        ItemDetails {
                                            item: item.clone(),
                                            count,
                                            set_pieces: card.set_pieces,
                                            on_close: move |_| pick_item.set(None),
                                        }
                                        button {
                                            class: "city-btn",
                                            onclick: move |_| {
                                                equip_traveler_item(
                                                    state,
                                                    index,
                                                    gear_slot(),
                                                    item_id.clone(),
                                                    message,
                                                    gear_mode,
                                                );
                                                pick_item.set(None);
                                            },
                                            "{i18n::t(\"btn.equip\")}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Надетую вещь показывает карточка с «Снять».
                    if gear_mode() == 2 {
                        if let Some(gear) = card.gear.iter().find(|gear| gear.slot == gear_slot()) {
                            if let Some(item) = gear.item.clone() {
                                ItemDetails {
                                    item,
                                    count: gear.bag_count,
                                    set_pieces: card.set_pieces,
                                    on_close: move |_| gear_mode.set(0),
                                }
                                button {
                                    class: "city-btn",
                                    onclick: move |_| {
                                        let outcome = {
                                            let mut guard = state.write();
                                            city::unequip_traveler_gear(&mut guard, index, gear_slot())
                                        };
                                        if let Err(error) = outcome {
                                            message.set(error.text().to_string());
                                        }
                                        gear_mode.set(0);
                                        refresh_view(state);
                                    },
                                    "{i18n::t(\"btn.unequip\")}"
                                }
                            }
                        }
                    }
                    div { class: "city-row muted",
                        span { "{card.gear_hint}" }
                    }
                }
            }
            div { class: "city-row muted",
                span { "{i18n::t(\"traveler.locations\")}" }
            }
            // Каждая локация — свой блок: дроп с шансами, время и аванс.
            for location in card.locations {
                {
                    let id = location.id.clone();
                    let name = location.name.clone();
                    let time = location.time.clone();
                    let advance = location.advance.clone();
                    let closed = location.closed;
                    let can_send = location.can_send;
                    rsx! {
                        div { class: if closed { "traveler-loc closed" } else { "traveler-loc" },
                            div { class: "traveler-loc-head",
                                span { class: "city-upgrade-name", "{name}" }
                                span { class: "city-row-muted", "{time}" }
                            }
                            div { class: "traveler-loc-loot",
                                for line in location.loot_lines {
                                    span { class: "traveler-loot-line", "{line}" }
                                }
                            }
                            button {
                                class: "city-btn traveler-loc-pay",
                                disabled: !can_send,
                                onclick: move |_| {
                                    let outcome = {
                                        let mut guard = state.write();
                                        city::send_traveler(&mut guard, index, &id)
                                    };
                                    match outcome {
                                        Ok(()) => message.set(String::new()),
                                        Err(error) => message.set(error.text().to_string()),
                                    }
                                    refresh_view(state);
                                },
                                if closed {
                                    "{i18n::t(\"traveler.locked\")}"
                                } else {
                                    "{advance}"
                                }
                            }
                        }
                    }
                }
            }
            if !message_text.is_empty() {
                div { class: "city-msg", "{message_text}" }
            }
        }
    }
}
