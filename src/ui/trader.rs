//! Полноэкранный экран торговцев, живой рынок и карточка товара.

use dioxus::prelude::*;

use crate::game::i18n;
use crate::game::items::{self, Item};
use crate::game::state::GameState;
use crate::game::trader::{self, GoodRarity, GoodState, MarketStatus};

use super::app::refresh_view;
use super::inventory::ItemIcon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Buy,
    Sell,
}

impl Mode {
    fn is_buy(self) -> bool {
        matches!(self, Self::Buy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    All,
    Available,
}

/// Полноэкранное окно торговцев.
#[component]
pub fn TradeScreen(
    state: Signal<GameState>,
    view: GameState,
    back: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let mut trader_index = use_signal(|| 0usize);
    let mut mode = use_signal(|| Mode::Buy);
    let mut show_quests = use_signal(|| false);
    let mut filter = use_signal(|| Filter::All);
    let mut selected = use_signal(|| None::<String>);
    let mut qty = use_signal(|| 1u32);
    let mut message = use_signal(String::new);
    let mut handled_back = use_signal(|| *back.peek());
    let mut offer_open = use_signal(|| false);
    let mut offer_item = use_signal(|| None::<String>);
    // Выбранная партия: количество и цена, согласованные в первом окне.
    let mut offer_qty = use_signal(|| 1u32);
    let mut offer_price = use_signal(|| 0.0f64);
    let mut offer_msg = use_signal(String::new);
    let mut haggle_open = use_signal(|| false);
    let mut target_price = use_signal(|| 0.0f64);
    let mut rounds_left = use_signal(|| 3u32);
    let mut haggle_locked = use_signal(|| false);
    let mut haggle_junk = use_signal(|| false);

    use_effect(move || {
        let count = back();
        if count <= *handled_back.peek() {
            return;
        }
        handled_back.set(count);
        // Системная кнопка «Назад» закрывает верхнее окно: сначала торг,
        // потом выбор товара, потом карточку, и только затем сам экран.
        if *haggle_open.peek() {
            haggle_open.set(false);
        } else if *offer_open.peek() {
            offer_open.set(false);
        } else if selected.peek().is_some() {
            selected.set(None);
        } else {
            on_close.call(());
        }
    });

    let trader_now = trader_index();
    let mode_now = mode();
    let filter_now = filter();
    let selected_now = selected();
    let message_now = message();

    let market = view.traders.get(trader_now);
    let mut visible: Vec<&GoodState> = market
        .map(|market| market.goods.iter().collect())
        .unwrap_or_default();
    if filter_now == Filter::Available {
        visible.retain(|good| is_available(&view, mode_now, good));
    }

    let hint = if visible.is_empty() {
        i18n::t("trade.empty").to_string()
    } else if let Some(item_id) = &selected_now {
        items::item(item_id)
            .map(|item| i18n::tf("trade.selected", &[item.name()]))
            .unwrap_or_default()
    } else {
        i18n::t("trade.hint").to_string()
    };
    let title_text = i18n::tf("trade.title", &[trader::trader_name(trader_now)]);
    let gold_text = i18n::tf("trade.gold", &[&format_money(view.gold)]);
    let column_owned = if mode_now.is_buy() {
        i18n::t("trade.col_stock")
    } else {
        i18n::t("trade.col_owned")
    };
    let wallet_text = i18n::tf(
        "trade.wallet",
        &[
            &format_money(trader::wallet_preview(&view, trader_now)),
            &format_money(trader::TRADER_WALLET),
        ],
    );

    rsx! {
        div { class: "trader-overlay full",
            div { class: "trader-window",
                // Пока открыт оффер или торг — таблица и карточка товара
                // скрыты: модалка занимает всё окно, как отдельная вкладка.
                if !offer_open() && !haggle_open() {
                    div { class: "trade-book",
                        div { class: "trade-head",
                            span { class: "trade-title", "{title_text}" }
                            button {
                                class: "trade-close",
                                onclick: move |_| on_close.call(()),
                                "{i18n::t(\"trade.leave\")}"
                            }
                        }
                        div { class: "trade-subhead",
                            span { "{i18n::t(\"trade.subhead\")}" }
                            span { class: "trade-subhead-gold", "{gold_text}" }
                        }

                        div { class: "trade-trader-picker",
                            for (index , trader_def) in trader::TRADERS.iter().enumerate() {
                                button {
                                    key: "{trader_def.id}",
                                    class: if index == trader_now { "trade-trader active" } else { "trade-trader" },
                                    onclick: move |_| {
                                        trader_index.set(index);
                                        selected.set(None);
                                        qty.set(1);
                                        message.set(String::new());
                                    },
                                    "{trader_def.name()}"
                                }
                            }
                        }

                        div { class: "trade-controls",
                            div { class: "trade-tabs",
                                button {
                                    class: if !show_quests() && mode_now.is_buy() { "trade-tab active" } else { "trade-tab" },
                                    onclick: move |_| {
                                        show_quests.set(false);
                                        mode.set(Mode::Buy);
                                        selected.set(None);
                                        qty.set(1);
                                        message.set(String::new());
                                    },
                                    "{i18n::t(\"trade.buy_tab\")}"
                                }
                                button {
                                    class: if !show_quests() && !mode_now.is_buy() { "trade-tab active" } else { "trade-tab" },
                                    onclick: move |_| {
                                        show_quests.set(false);
                                        mode.set(Mode::Sell);
                                        selected.set(None);
                                        qty.set(1);
                                        message.set(String::new());
                                    },
                                    "{i18n::t(\"trade.sell_tab\")}"
                                }
                                button {
                                    class: if show_quests() { "trade-tab active" } else { "trade-tab" },
                                    onclick: move |_| {
                                        show_quests.set(true);
                                        selected.set(None);
                                        qty.set(1);
                                        message.set(String::new());
                                    },
                                    "{i18n::t(\"quest.tab\")}"
                                }
                            }
                            if !show_quests() {
                                label { class: "trade-field",
                                    span { "{i18n::t(\"trade.filter\")}" }
                                    select {
                                        class: "trade-select",
                                        value: match filter_now {
                                            Filter::All => "all",
                                            Filter::Available => "available",
                                        },
                                        onchange: move |event| {
                                            filter
                                                .set(
                                                    if event.value() == "available" {
                                                        Filter::Available
                                                    } else {
                                                        Filter::All
                                                    },
                                                );
                                        },
                                        option { value: "all", "{i18n::t(\"trade.all\")}" }
                                        option { value: "available", "{i18n::t(\"trade.available\")}" }
                                    }
                                }
                                button {
                                    class: "trade-tab",
                                    onclick: move |_| {
                                        offer_open.set(true);
                                        offer_item.set(None);
                                        offer_msg.set(String::new());
                                    },
                                    "{i18n::t(\"trade.offer_btn\")}"
                                }
                            }
                        }
                        if !show_quests() {
                            div { class: "trade-table",
                                div { class: "trade-row trade-head-row",
                                    span { "{i18n::t(\"trade.col_good\")}" }
                                    span { class: "num", "{column_owned}" }
                                    span { class: "num", "{i18n::t(\"trade.col_buy\")}" }
                                    span { class: "num", "{i18n::t(\"trade.col_sell\")}" }
                                    span { class: "num", "{i18n::t(\"trade.col_demand\")}" }
                                    span { class: "num", "{i18n::t(\"trade.col_market\")}" }
                                }
                                if visible.is_empty() {
                                    div { class: "trade-empty", "{i18n::t(\"trade.empty\")}" }
                                }
                                for good in visible.iter().copied() {
                                    if let Some(item) = items::item(&good.item_id) {
                                        TradeRow {
                                            key: "{good.item_id}",
                                            item: item.clone(),
                                            good: good.clone(),
                                            mode: mode_now,
                                            view: view.clone(),
                                            selected: selected_now.as_deref() == Some(good.item_id.as_str()),
                                            on_select: move |item_id: String| {
                                                selected.set(Some(item_id));
                                                qty.set(1);
                                                message.set(String::new());
                                            },
                                        }
                                    }
                                }
                            }
                        } else {
                            QuestBoard { state, view: view.clone() }
                        }
                        if !show_quests() {
                            div { class: "trade-foot",
                                div { class: "trade-wallet", "{wallet_text}" }
                                div { "{MarketStatus::Balance.legend()}" }
                                div { class: "trade-hint", "{hint}" }
                                if !message_now.is_empty() {
                                    div { class: "trade-error", "{message_now}" }
                                }
                            }
                        }
                    }

                    if let Some(item_id) = selected_now {
                        if let Some(item) = items::item(&item_id) {
                            ItemCard {
                                state,
                                view: view.clone(),
                                trader_index: trader_now,
                                item: item.clone(),
                                mode: mode_now,
                                qty,
                                message,
                                on_close: move |_| selected.set(None),
                            }
                        }
                    }
                }
                if offer_open() {
                    OfferModal {
                        state,
                        view: view.clone(),
                        trader_index: trader_now,
                        on_select: move |(id, q, price, is_junk, max_rounds)| {
                            offer_item.set(Some(id));
                            offer_qty.set(q);
                            offer_price.set(price);
                            target_price.set(price);
                            rounds_left.set(max_rounds);
                            haggle_locked.set(false);
                            haggle_junk.set(is_junk);
                            offer_open.set(false); // Закрываем выбор
                            haggle_open.set(true); // Открываем торг
                        },
                        on_close: move |_| offer_open.set(false),
                    }
                }
                if haggle_open() {
                    HaggleModal {
                        state,
                        view: view.clone(),
                        trader_index: trader_now,
                        offer_item,
                        offer_qty,
                        offer_price,
                        target_price,
                        rounds_left,
                        haggle_locked,
                        haggle_junk,
                        on_back: move |_| {
                            haggle_open.set(false);
                            offer_open.set(true); // Вернуться к выбору
                        },
                        on_close: move |_| haggle_open.set(false),
                    }
                }
            }
        }
    }
}

#[component]
fn HaggleModal(
    state: Signal<GameState>,
    view: GameState,
    trader_index: usize,
    offer_item: Signal<Option<String>>,
    offer_qty: Signal<u32>,
    offer_price: Signal<f64>,
    target_price: Signal<f64>,
    rounds_left: Signal<u32>,
    haggle_locked: Signal<bool>,
    haggle_junk: Signal<bool>,
    on_back: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let mut offer_qty = offer_qty;
    let mut offer_price = offer_price;
    let mut target_price = target_price;
    let mut rounds_left = rounds_left;
    let mut haggle_locked = haggle_locked;
    let mut haggle_junk = haggle_junk;
    let mut offer_msg = use_signal(String::new);

    let selected_id = offer_item();
    let qty = offer_qty();
    let current_price = offer_price();
    let target = target_price();
    let rounds = rounds_left();
    let is_locked = haggle_locked();

    // Ручной ввод цены. Значение живёт в локальном сигнале и обновляется
    // только вместе с ценой (кнопки количества, ползунок, итог торга), поэтому
    // набранное игроком не перетирается на каждом рендере.
    let mut price_text = use_signal(|| format!("{:.0}", target));

    let Some(id) = selected_id else {
        return rsx! {};
    };

    let selected_name = items::item(&id)
        .map(|item| item.name().to_string())
        .unwrap_or_default();
    let selected_max = crate::game::trader::max_offer(&view, trader_index, &id);

    let (min_price, max_price) = if current_price > 0.0 {
        crate::game::trader::haggle_range(current_price)
    } else {
        (0.0, 0.0)
    };

    let chance = if current_price > 0.0 {
        crate::game::trader::haggle_chance(
            &view,
            current_price,
            target,
            min_price,
            max_price,
            haggle_junk(),
        )
    } else {
        0
    };

    rsx! {
        div { class: "offer-overlay",
            div { class: "trade-card",
                div { class: "trade-card-head",
                    span { class: "trade-card-title", "{selected_name}" }
                    span { class: "trade-rarity", "x {qty}" }
                }

                div { class: "trade-qty", style: "margin-top: 8px;",
                    span { class: "trade-qty-label", "{i18n::t(\"trade.qty\")}" }
                    button {
                        class: "trade-step",
                        disabled: is_locked || qty <= 1,
                        onclick: {
                            let id = id.clone();
                            move |_| {
                                let new_q = qty - 1;
                                if let Ok(quote) = crate::game::trader::evaluate_offer(
                                    &mut state.write(),
                                    trader_index,
                                    &id,
                                    new_q,
                                ) {
                                    offer_qty.set(new_q);
                                    offer_price.set(quote.total);
                                    target_price.set(quote.total);
                                    price_text.set(format!("{:.0}", quote.total));
                                    haggle_locked.set(false);
                                    // Партия изменилась — пересчитываем «junk»-флаг:
                                    // от него зависит шанс торга.
                                    haggle_junk.set(quote.parts.iter().all(|p| p.is_junk));
                                    offer_msg.set(String::new());
                                }
                            }
                        },
                        "−"
                    }
                    span { class: "trade-qty-value", "{qty}" }
                    button {
                        class: "trade-step",
                        disabled: is_locked || qty >= selected_max,
                        onclick: {
                            let id = id.clone();
                            move |_| {
                                let new_q = (qty + 1).min(selected_max);
                                if let Ok(quote) = crate::game::trader::evaluate_offer(
                                    &mut state.write(),
                                    trader_index,
                                    &id,
                                    new_q,
                                ) {
                                    offer_qty.set(new_q);
                                    offer_price.set(quote.total);
                                    target_price.set(quote.total);
                                    price_text.set(format!("{:.0}", quote.total));
                                    haggle_locked.set(false);
                                    // Партия изменилась — пересчитываем «junk»-флаг:
                                    // от него зависит шанс торга.
                                    haggle_junk.set(quote.parts.iter().all(|p| p.is_junk));
                                    offer_msg.set(String::new());
                                }
                            }
                        },
                        "+"
                    }
                    button {
                        class: "trade-step",
                        disabled: is_locked || qty >= selected_max,
                        onclick: {
                            let id = id.clone();
                            move |_| {
                                let new_q = (qty + 10).min(selected_max);
                                if let Ok(quote) = crate::game::trader::evaluate_offer(
                                    &mut state.write(),
                                    trader_index,
                                    &id,
                                    new_q,
                                ) {
                                    offer_qty.set(new_q);
                                    offer_price.set(quote.total);
                                    target_price.set(quote.total);
                                    price_text.set(format!("{:.0}", quote.total));
                                    haggle_locked.set(false);
                                    // Партия изменилась — пересчитываем «junk»-флаг:
                                    // от него зависит шанс торга.
                                    haggle_junk.set(quote.parts.iter().all(|p| p.is_junk));
                                    offer_msg.set(String::new());
                                }
                            }
                        },
                        "+10"
                    }
                    button {
                        class: "trade-max",
                        disabled: is_locked || qty >= selected_max,
                        onclick: {
                            let id = id.clone();
                            move |_| {
                                let new_q = selected_max;
                                if let Ok(quote) = crate::game::trader::evaluate_offer(
                                    &mut state.write(),
                                    trader_index,
                                    &id,
                                    new_q,
                                ) {
                                    offer_qty.set(new_q);
                                    offer_price.set(quote.total);
                                    target_price.set(quote.total);
                                    price_text.set(format!("{:.0}", quote.total));
                                    haggle_locked.set(false);
                                    // Партия изменилась — пересчитываем «junk»-флаг:
                                    // от него зависит шанс торга.
                                    haggle_junk.set(quote.parts.iter().all(|p| p.is_junk));
                                    offer_msg.set(String::new());
                                }
                            }
                        },
                        "{i18n::tf(\"btn.max\", &[&selected_max.to_string()])}"
                    }
                }

                div { class: "trade-line",
                    span { "{i18n::t(\"trade.current_offer\")}" }
                    span { class: "trade-line-value", "{format_money(current_price)}" }
                }

                div { class: "haggle-row",
                    div { class: "haggle-info",
                        div { class: "trade-line-muted",
                            "{i18n::tf(\"trade.min_price\", &[&format_money(min_price)])}"
                        }
                        div { class: "trade-line-muted",
                            "{i18n::tf(\"trade.max_price\", &[&format_money(max_price)])}"
                        }
                        div { class: "haggle-chance", "{i18n::t(\"trade.chance\")}: {chance}%" }
                        div { class: "trade-line-muted", "{i18n::t(\"trade.rounds_left\")}: {rounds}" }
                    }
                    div { class: "haggle-slider-wrap",
                        input {
                            class: "haggle-slider",
                            r#type: "range",
                            min: "{min_price}",
                            max: "{max_price}",
                            step: "1",
                            value: "{target}",
                            disabled: is_locked || rounds == 0,
                            oninput: move |evt| {
                                if let Ok(val) = evt.value().parse::<f64>() {
                                    target_price.set(val);
                                    price_text.set(format!("{:.0}", val));
                                }
                            },
                        }
                    }
                }

                div { class: "haggle-manual",
                    span { "{i18n::t(\"trade.your_price\")}" }
                    input {
                        class: "haggle-input",
                        r#type: "number",
                        min: "{min_price}",
                        max: "{max_price}",
                        step: "1",
                        value: "{price_text}",
                        disabled: is_locked,
                        oninput: move |evt| {
                            price_text.set(evt.value());
                            if let Ok(val) = evt.value().parse::<f64>() {
                                target_price.set(val.clamp(min_price, max_price));
                            }
                        },
                    }
                }

                if !offer_msg().is_empty() {
                    div { class: if is_locked { "trade-error ok" } else { "trade-error" }, "{offer_msg()}" }
                }

                div { class: "trade-actions",
                    if !is_locked {
                        button {
                            class: "trade-btn",
                            disabled: rounds == 0 || (target - current_price).abs() < 0.5 || !view.can_haggle(),
                            onclick: move |_| {
                                let target_now = target_price();
                                let current_now = offer_price();
                                let outcome = {
                                    let mut guard = state.write();
                                    crate::game::trader::resolve_haggle(
                                        &mut guard,
                                        current_now,
                                        target_now,
                                        min_price,
                                        max_price,
                                        haggle_junk(),
                                    )
                                };
                                match outcome {
                                    crate::game::trader::HaggleOutcome::Accepted(p) => {
                                        offer_price.set(p);
                                        target_price.set(p);
                                        price_text.set(format!("{:.0}", p));
                                        haggle_locked.set(true);
                                        offer_msg.set(i18n::t("trade.haggle_accepted").to_string());
                                    }
                                    crate::game::trader::HaggleOutcome::Rejected(p) => {
                                        offer_price.set(p);
                                        target_price.set(p);
                                        price_text.set(format!("{:.0}", p));
                                        rounds_left.set(rounds.saturating_sub(1));
                                        offer_msg.set(i18n::t("trade.haggle_rejected").to_string());
                                    }
                                }
                            },
                            "{i18n::t(\"trade.haggle_btn\")}"
                        }
                        if !view.can_haggle() {
                            div { class: "trade-error", "{i18n::t(\"trade.debt_no_haggle\")}" }
                        }
                    }

                    button {
                        class: "trade-btn primary",
                        onclick: {
                            let id = id.clone();
                            move |_| {
                                let current_qty = offer_qty();
                                let current_price = offer_price();
                                let result = {
                                    let mut guard = state.write();
                                    crate::game::trader::execute_offer(
                                        &mut guard,
                                        trader_index,
                                        &id,
                                        current_qty,
                                        current_price,
                                    )
                                };
                                if result.is_ok() {
                                    on_close.call(());
                                } else if let Err(e) = result {
                                    offer_msg.set(e.text().to_string());
                                }
                                super::app::refresh_view(state);
                            }
                        },
                        "{i18n::t(\"trade.sell_btn\")}"
                    }
                }

                div { class: "trade-actions", style: "margin-top: 10px;",
                    button {
                        class: "trade-btn cancel",
                        onclick: move |_| on_back.call(()),
                        "{i18n::t(\"btn.back\")}"
                    }
                    button {
                        class: "trade-btn cancel",
                        onclick: move |_| on_close.call(()),
                        "{i18n::t(\"btn.close\")}"
                    }
                }
            }
        }
    }
}

#[component]
fn TradeRow(
    item: Item,
    good: GoodState,
    mode: Mode,
    view: GameState,
    selected: bool,
    on_select: EventHandler<String>,
) -> Element {
    let locked = view.smith_level() < items::item_unlock_level(&item.id);
    // Цена покупки — ровно та, что спишет `trader::buy`: с наценкой рынка
    // и множителем за неразблокированный товар. Раньше здесь стояла цена
    // витрины (`sell_price_for_good`), и итог в карточке не сходился с кассой.
    let buy = trader::buy_price_for(&item, good.demand, good.supply, view.smith_level());
    let sell = trader::sell_price(&item, good.demand, good.supply, view.trade_bonus)
        * view.debt_sell_multiplier();
    let stock = match mode {
        Mode::Buy => good.stock,
        Mode::Sell => view.item_count(&item.id),
    };
    let market = if mode == Mode::Sell && good.buy_limit == 0 {
        "—".to_string()
    } else {
        good.status().short().to_string()
    };
    let reason = row_reason(&view, mode, &item, &good);
    let unavailable = reason.is_some();
    let item_id = item.id.clone();
    let class = if unavailable {
        "trade-row locked"
    } else if selected {
        "trade-row selected"
    } else {
        "trade-row"
    };

    rsx! {
        button {
            class: "{class}",
            onclick: move |_| on_select.call(item_id.clone()),
            span { class: "trade-name",
                ItemIcon { item: item.clone(), class: "trade-item-icon".to_string() }
                span { "{item.name()}" }
                if let Some(reason) = reason {
                    span { class: "trade-reason", " · {reason}" }
                }
            }
            span { class: "num", "{stock}" }
            span { class: "num",
                if locked {
                    "{format_money(buy)} 🔒"
                } else {
                    "{format_money(buy)}"
                }
            }
            span { class: "num", "{format_money(sell)}" }
            span { class: "num", "{good.demand}/{good.supply}" }
            span { class: "num", "{market}" }
        }
    }
}

/// Карточка выбранного товара. Показывается только после тапа по строке.
#[component]
fn ItemCard(
    state: Signal<GameState>,
    view: GameState,
    trader_index: usize,
    item: Item,
    mode: Mode,
    qty: Signal<u32>,
    message: Signal<String>,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let mut qty = qty;
    let mut message = message;

    let Some(good) = view
        .traders
        .get(trader_index)
        .and_then(|market| market.good(&item.id))
    else {
        return rsx! {
            div {}
        };
    };

    let locked = view.smith_level() < items::item_unlock_level(&item.id);
    let buy = trader::buy_price_for(&item, good.demand, good.supply, view.smith_level());
    let sell = trader::sell_price(&item, good.demand, good.supply, view.trade_bonus)
        * view.debt_sell_multiplier();
    // Маржа считается от той же цены, что видит игрок в карточке.
    let margin = (buy - sell).max(0.0);
    let status = good.status();
    let owned = view.item_count(&item.id);

    let room = if owned > 0 {
        view.item_capacity_left(&item.id)
    } else if view.inventory_free_slots() > 0 {
        items::ITEM_STACK_LIMIT
    } else {
        0
    };
    let wallet = trader::wallet_preview(&view, trader_index);
    let bulk = trader::is_bulk(&item.id);
    let max_qty = match mode {
        Mode::Buy => trader::max_buy(view.gold, good.stock, buy).min(room),
        Mode::Sell => trader::max_sell(
            owned,
            good.buy_limit,
            view.gold,
            view.gold_cap(),
            sell,
            wallet,
            bulk,
        ),
    };

    let quantity = if max_qty == 0 {
        0
    } else {
        qty().clamp(1, max_qty)
    };
    let unit_price = match mode {
        Mode::Buy => buy,
        Mode::Sell => sell,
    };
    let total = unit_price * quantity as f64;
    let gold_after = match mode {
        Mode::Buy => (view.gold - total).max(0.0),
        Mode::Sell => (view.gold + total).min(view.gold_cap()),
    };
    let total_text = if quantity > 0 {
        i18n::tf("trade.total_value", &[&format_money(total)])
    } else {
        "—".to_string()
    };

    let error = match mode {
        Mode::Buy => {
            if good.stock == 0 {
                Some(i18n::t("trade.no_stock").to_string())
            } else if max_qty == 0 && room == 0 {
                Some(i18n::t("trade.no_room").to_string())
            } else if view.gold + 1e-9 < buy {
                Some(i18n::t("trade.no_gold").to_string())
            } else if quantity == 0 {
                Some(i18n::t("trade.nothing_buy").to_string())
            } else {
                None
            }
        }
        Mode::Sell => {
            if owned == 0 {
                Some(i18n::t("trade.nothing_owned").to_string())
            } else if good.buy_limit == 0 {
                Some(i18n::t("trade.limit_reached").to_string())
            } else if view.gold + 1e-9 >= view.gold_cap() {
                Some(i18n::t("trade.gold_cap").to_string())
            } else if quantity == 0 {
                Some(i18n::t("trade.gold_limit_short").to_string())
            } else {
                None
            }
        }
    };

    let advice_text = if mode == Mode::Sell && good.buy_limit == 0 {
        i18n::t("trade.not_buying")
    } else {
        advice(status, mode)
    };
    let can_primary = error.is_none() && quantity > 0;
    let can_quick = max_qty >= 1;
    let buy_item_id = item.id.clone();
    let buy_all_item_id = item.id.clone();
    let sell_item_id = item.id.clone();
    let sell_all_item_id = item.id.clone();
    let item_name = item.name();
    let rarity = item.rarity();
    let appearance = GoodRarity::of(&item).label();
    let message_now = message();

    rsx! {
        div { class: "trade-card",
            div { class: "trade-card-head",
                span { class: "trade-card-title", "{item_name}" }
                span { class: "trade-rarity", "{i18n::tf(\"trade.rarity\", &[rarity])}" }
            }

            div { class: "trade-line",
                span { "{i18n::t(\"trade.appearance\")}" }
                span { class: "trade-line-value", "{appearance}" }
            }

            div { class: "trade-line",
                span { "{i18n::t(\"trade.buy_price\")}" }
                span { class: "trade-line-value",
                    "{i18n::tf(\"trade.total_value\", &[&format_money(buy)])}"
                }
            }
            if locked {
                div { class: "trade-locked", "{i18n::t(\"trade.locked_price\")}" }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.sell_price\")}" }
                span { class: "trade-line-value",
                    "{i18n::tf(\"trade.margin\", &[&format_money(sell), &format_money(margin)])}"
                }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.demand\")}" }
                span { class: "trade-bar", "{bar(good.demand)}" }
                span { class: "num trade-line-value", "{good.demand}" }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.supply\")}" }
                span { class: "trade-bar", "{bar(good.supply)}" }
                span { class: "num trade-line-value", "{good.supply}" }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.market\")}" }
                span { class: "trade-market", "{status.label()} {status.arrow()}" }
                span { class: "trade-line-muted",
                    "{i18n::tf(\"trade.diff\", &[&format_signed(good.diff())])}"
                }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.stock\")}" }
                span { class: "trade-line-value", "{good.stock}" }
            }
            div { class: "trade-line",
                span { "{i18n::t(\"trade.owned\")}" }
                span { class: "trade-line-value", "{owned}" }
            }

            div { class: "trade-qty",
                span { class: "trade-qty-label", "{i18n::t(\"trade.qty\")}" }
                button {
                    class: "trade-step",
                    disabled: max_qty == 0,
                    onclick: move |_| qty.set(qty().saturating_sub(1).max(1)),
                    "−"
                }
                span { class: "trade-qty-value", "{quantity}" }
                button {
                    class: "trade-step",
                    disabled: max_qty == 0,
                    onclick: move |_| qty.set((qty() + 1).min(max_qty)),
                    "+"
                }
                button {
                    class: "trade-max",
                    disabled: max_qty == 0,
                    onclick: move |_| qty.set(max_qty),
                    "{i18n::tf(\"btn.max\", &[&max_qty.to_string()])}"
                }
            }

            div { class: "trade-line trade-total",
                span { "{i18n::t(\"trade.total\")}" }
                span { class: "trade-line-value", "{total_text}" }
                span { class: "trade-line-muted",
                    "{i18n::tf(\"trade.gold_after\", &[&format_money(gold_after)])}"
                }
            }

            if let Some(error) = error.clone() {
                div { class: "trade-error", "{error}" }
            }
            if !message_now.is_empty() {
                div { class: "trade-error", "{message_now}" }
            }

            div { class: "trade-actions",
                if mode == Mode::Buy {
                    button {
                        class: if can_primary { "trade-btn primary" } else if error.is_some() { "trade-btn blocked" } else { "trade-btn" },
                        disabled: !can_primary,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::buy(&mut guard, trader_index, &buy_item_id, quantity)
                            };
                            message
                                .set(
                                    match result {
                                        Ok(()) => String::new(),
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            qty.set(1);
                            refresh_view(state);
                        },
                        "{i18n::t(\"trade.buy_tab\")}"
                    }
                    button {
                        class: "trade-btn",
                        disabled: !can_quick,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::buy(&mut guard, trader_index, &buy_all_item_id, max_qty)
                            };
                            message
                                .set(
                                    match result {
                                        Ok(()) => String::new(),
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            qty.set(1);
                            refresh_view(state);
                        },
                        "{i18n::tf(\"trade.buy_n\", &[&max_qty.to_string()])}"
                    }
                } else {
                    button {
                        class: if can_primary { "trade-btn primary" } else if error.is_some() { "trade-btn blocked" } else { "trade-btn" },
                        disabled: !can_primary,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::sell(&mut guard, trader_index, &sell_item_id, quantity)
                            };
                            message
                                .set(
                                    match result {
                                        Ok(()) => String::new(),
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            qty.set(1);
                            refresh_view(state);
                        },
                        "{i18n::t(\"trade.sell_btn\")}"
                    }
                    button {
                        class: "trade-btn",
                        disabled: !can_quick,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::sell(&mut guard, trader_index, &sell_all_item_id, max_qty)
                            };
                            message
                                .set(
                                    match result {
                                        Ok(()) => String::new(),
                                        Err(error) => error.text().to_string(),
                                    },
                                );
                            qty.set(1);
                            refresh_view(state);
                        },
                        "{i18n::tf(\"trade.sell_n\", &[&max_qty.to_string()])}"
                    }
                }
                button {
                    class: "trade-btn cancel",
                    onclick: move |_| on_close.call(()),
                    "{i18n::t(\"btn.cancel\")}"
                }
            }

            div { class: "trade-advice", "{i18n::tf(\"trade.advice\", &[advice_text])}" }
        }
    }
}

fn is_available(view: &GameState, mode: Mode, good: &GoodState) -> bool {
    let Some(item) = items::item(&good.item_id) else {
        return false;
    };
    match mode {
        Mode::Buy => {
            let price = trader::buy_price_for(item, good.demand, good.supply, view.smith_level());
            good.stock > 0 && view.gold + 1e-9 >= price && view.can_add_item(&good.item_id, 1)
        }
        Mode::Sell => {
            view.item_count(&good.item_id) > 0
                && good.buy_limit > 0
                && view.gold + 1e-9 < view.gold_cap()
        }
    }
}

fn row_reason(view: &GameState, mode: Mode, item: &Item, good: &GoodState) -> Option<&'static str> {
    match mode {
        Mode::Buy => {
            let price = trader::buy_price_for(item, good.demand, good.supply, view.smith_level());
            if good.stock == 0 {
                Some(i18n::t("trade.reason.no_stock"))
            } else if view.gold + 1e-9 < price {
                Some(i18n::t("trade.reason.no_gold"))
            } else if !view.can_add_item(&item.id, 1) {
                Some(i18n::t("trade.reason.no_room"))
            } else {
                None
            }
        }
        Mode::Sell => {
            if good.buy_limit == 0 {
                Some(i18n::t("trade.reason.not_buying"))
            } else if view.item_count(&good.item_id) == 0 {
                Some(i18n::t("trade.reason.nothing"))
            } else if view.gold + 1e-9 >= view.gold_cap() {
                Some(i18n::t("trade.reason.gold_cap"))
            } else {
                None
            }
        }
    }
}

fn advice(status: MarketStatus, mode: Mode) -> &'static str {
    match (status, mode) {
        (MarketStatus::Deficit, Mode::Buy) => i18n::t("trade.tip.deficit_buy"),
        (MarketStatus::Deficit, Mode::Sell) => i18n::t("trade.tip.deficit_sell"),
        (MarketStatus::Balance, _) => i18n::t("trade.tip.balance"),
        (MarketStatus::Surplus, Mode::Buy) => i18n::t("trade.tip.surplus_buy"),
        (MarketStatus::Surplus, Mode::Sell) => i18n::t("trade.tip.surplus_sell"),
    }
}

fn bar(value: u8) -> String {
    let filled = (value.min(100) as usize) / 10;
    format!("{}{}", "#".repeat(filled), "-".repeat(10 - filled))
}

fn format_signed(value: i32) -> String {
    match value {
        0 => "0".to_string(),
        positive if positive > 0 => format!("+{positive}"),
        negative => format!("{}{}", i18n::minus(), -negative),
    }
}

pub(crate) fn format_money(value: f64) -> String {
    let rounded = value.round() as i64;
    let negative = rounded < 0;
    let digits = rounded.abs().to_string();
    let mut output = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(' ');
        }
        output.push(digit);
    }
    if negative {
        format!("{}{output}", i18n::minus())
    } else {
        output
    }
}

/// Окно «Предложить товар» с торгом.
#[component]
fn OfferModal(
    state: Signal<GameState>,
    view: GameState,
    trader_index: usize,
    on_select: EventHandler<(String, u32, f64, bool, u32)>,
    on_close: EventHandler<()>,
) -> Element {
    let mut state = state;
    let mut offer_msg = use_signal(String::new);
    let mut selected_id = use_signal(|| None::<String>);
    let mut qty = use_signal(|| 1u32);

    let mut sellable = Vec::new();
    for (id, count) in &view.inventory {
        if *count > 0
            && crate::game::items::is_sellable_from_bag(&view, id)
            && crate::game::trader::max_offer(&view, trader_index, id) > 0
        {
            sellable.push((id.clone(), *count));
        }
    }
    sellable.sort_by(|a, b| a.0.cmp(&b.0));

    let selected_now = selected_id();
    let qty_val = qty();

    let selected_max = selected_now
        .as_ref()
        .map(|id| crate::game::trader::max_offer(&view, trader_index, id))
        .unwrap_or(0);
    let selected_name = selected_now
        .as_ref()
        .and_then(|id| items::item(id))
        .map(|item| item.name().to_string())
        .unwrap_or_default();
    let selected_id_value = selected_now.clone().unwrap_or_default();

    rsx! {
        div { class: "offer-overlay",
            div { class: "trade-card",
                div { class: "trade-card-head",
                    span { class: "trade-card-title", "{i18n::t(\"trade.offer_title\")}" }
                    button {
                        class: "trade-close",
                        onclick: move |_| on_close.call(()),
                        "[ X ]"
                    }
                }

                if sellable.is_empty() {
                    div { class: "trade-empty", "{i18n::t(\"trade.offer_empty\")}" }
                } else {
                    div { class: "trade-table", style: "margin-top: 8px;",
                        div { class: "trade-offer-grid",
                            for (id , count) in sellable {
                                button {
                                    key: "{id}",
                                    class: if selected_now.as_deref() == Some(id.as_str()) { "inv-item selected" } else { "inv-item" },
                                    onclick: move |_| {
                                        selected_id.set(Some(id.clone()));
                                        qty.set(1);
                                        offer_msg.set(String::new());
                                    },
                                    if let Some(item) = items::item(&id) {
                                        ItemIcon {
                                            item: item.clone(),
                                            class: "inv-item-icon".to_string(),
                                        }
                                    }
                                    if count > 1 {
                                        span { class: "inv-badge", "{count}" }
                                    }
                                }
                            }
                        }
                    }

                    if selected_now.is_some() {
                        div { class: "trade-line", style: "margin-top: 8px;",
                            span { "{selected_name}" }
                            span { class: "trade-line-value", "x {qty_val}" }
                        }

                        div { class: "trade-qty",
                            span { class: "trade-qty-label", "{i18n::t(\"trade.qty\")}" }
                            button {
                                class: "trade-step",
                                disabled: qty_val <= 1,
                                onclick: move |_| qty.set(qty().saturating_sub(1).max(1)),
                                "−"
                            }
                            button {
                                class: "trade-step",
                                onclick: move |_| qty.set(1),
                                "0"
                            }
                            button {
                                class: "trade-step",
                                disabled: selected_max == 0 || qty_val >= selected_max,
                                onclick: move |_| qty.set((qty() + 1).min(selected_max)),
                                "+"
                            }
                            button {
                                class: "trade-step",
                                disabled: selected_max <= 1 || qty_val >= selected_max,
                                onclick: move |_| qty.set((qty() + 10).min(selected_max)),
                                "+10"
                            }
                            button {
                                class: "trade-max",
                                disabled: selected_max == 0,
                                onclick: move |_| qty.set(selected_max),
                                "{i18n::tf(\"btn.max\", &[&selected_max.to_string()])}"
                            }
                        }

                        div { class: "trade-actions",
                            button {
                                class: "trade-btn primary",
                                onclick: move |_| {
                                    let q = qty().clamp(1, selected_max);
                                    let result = {
                                        let mut guard = state.write();
                                        crate::game::trader::evaluate_offer(
                                            &mut guard,
                                            trader_index,
                                            &selected_id_value,
                                            q,
                                        )
                                    };
                                    match result {
                                        Ok(quote) => {
                                            let junk = quote.parts.iter().all(|p| p.is_junk);
                                            let rounds = view.haggle_rounds();
                                            on_select
                                                .call((selected_id_value.clone(), q, quote.total, junk, rounds));
                                        }
                                        Err(e) => offer_msg.set(e.text().to_string()),
                                    }
                                },
                                "{i18n::t(\"btn.next\")}"
                            }
                        }
                    }

                    if !offer_msg().is_empty() {
                        div { class: "trade-error", "{offer_msg()}" }
                    }
                }
            }
        }
    }
}

/// Доска объявлений с 6 квестами и блоком долга.
#[component]
fn QuestBoard(state: Signal<GameState>, view: GameState) -> Element {
    let mut state = state;
    let message = use_signal(String::new);
    let now = view.quest_clock;

    // Сортируем: сначала активные, потом доступные, потом остальные.
    let mut quests_sorted: Vec<_> = view.quests.to_vec();
    quests_sorted.sort_by_key(|q| match q.status {
        crate::game::quests::QuestStatus::Active => 0,
        crate::game::quests::QuestStatus::Available => 1,
        crate::game::quests::QuestStatus::Failed => 2,
        crate::game::quests::QuestStatus::Completed => 3,
    });

    let active_count = view.active_quests_count();
    let max_active = crate::game::quests::MAX_ACTIVE_QUESTS;
    let debt = view.debt;
    let gold = view.gold;

    // Время до следующего визита коллекторов.
    let collector_text = if debt > 0.0 && view.collector_tick_at > now {
        let left = (view.collector_tick_at - now).max(0.0).round() as u64;
        let min = left / 60;
        let sec = left % 60;
        let time = if min > 0 && sec > 0 {
            i18n::tf(
                "time.minutes_seconds",
                &[&min.to_string(), &sec.to_string()],
            )
        } else if min > 0 {
            i18n::tf("time.minutes_only", &[&min.to_string()])
        } else {
            i18n::tf("time.seconds", &[&sec.to_string()])
        };
        i18n::tf("quest.debt_next", &[&time])
    } else {
        String::new()
    };

    rsx! {
        div { class: "quest-board",
            // Заголовок.
            div { class: "quest-board-head",
                h3 { class: "quest-board-title", "{i18n::t(\"quest.board_title\")}" }
                div { class: "quest-board-counter", "{active_count} / {max_active}" }
            }
            div { class: "quest-board-hint", "{i18n::t(\"quest.board_hint\")}" }

            // Долг, если есть.
            if debt > 0.0 {
                div { class: "quest-debt",
                    div { class: "quest-debt-line",
                        span { "{i18n::tf(\"quest.debt_label\", &[&format_money(debt)])}" }
                        span { class: "quest-debt-gold",
                            "{i18n::tf(\"trade.gold\", &[&format_money(gold)])}"
                        }
                    }
                    div { class: "quest-debt-hint", "{i18n::t(\"quest.debt_hint\")}" }
                    if !collector_text.is_empty() {
                        div { class: "quest-debt-hint", "{collector_text}" }
                    }
                    div { class: "quest-debt-actions",
                        button {
                            class: "trade-btn primary",
                            disabled: gold < debt,
                            onclick: move |_| {
                                {
                                    let mut guard = state.write();
                                    let amount = guard.debt.min(guard.gold);
                                    crate::game::quests::pay_debt(&mut guard, amount);
                                }
                                refresh_view(state);
                            },
                            "{i18n::t(\"quest.debt_clear\")}"
                        }
                    }
                }
            }

            // Список квестов.
            if quests_sorted.is_empty() {
                div { class: "quest-empty", "{i18n::t(\"quest.board_empty\")}" }
            } else {
                for q in quests_sorted {
                    QuestCard {
                        key: "{q.id}",
                        quest: q.clone(),
                        state,
                        view: view.clone(),
                        now,
                        message,
                    }
                }
            }

            // Сообщение от действий.
            if !message().is_empty() {
                div { class: "trade-error", "{message()}" }
            }
        }
    }
}

/// Одна карточка квеста.
#[component]
fn QuestCard(
    quest: crate::game::quests::Quest,
    state: Signal<GameState>,
    view: GameState,
    now: f64,
    message: Signal<String>,
) -> Element {
    let mut message = message;
    let mut state = state;
    let title = i18n::t(&format!("quest.template.{}.title", quest.template_id));
    let author = quest.author.display_name();

    // Список запроса: «5 кожа, 2 ткань».
    let request_text = quest
        .request
        .iter()
        .map(|(id, qty)| {
            let name = super::inventory::ingredient_name(id);
            format!("{qty} {name}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let reward_text = quest.reward.describe();

    let body = i18n::tf_named(
        &format!("quest.template.{}.body", quest.template_id),
        &[
            ("author", &author),
            ("request", &request_text),
            ("reward", &reward_text),
        ],
    );

    let status = quest.status;
    let declined = quest.declined;
    let quest_id = quest.id.clone();
    let is_active = status == crate::game::quests::QuestStatus::Active;
    let is_available = status == crate::game::quests::QuestStatus::Available;

    // Таймер для активного.
    let timer_text = if is_active && quest.expires_at > now {
        let left = (quest.expires_at - now).max(0.0).round() as u64;
        let min = left / 60;
        let sec = left % 60;
        if min > 0 {
            i18n::tf(
                "time.minutes_seconds",
                &[&min.to_string(), &sec.to_string()],
            )
        } else {
            i18n::tf("time.seconds", &[&sec.to_string()])
        }
    } else {
        String::new()
    };

    // Сколько нужно для сдачи (проверка для кнопки).
    let can_complete = is_active
        && quest
            .request
            .iter()
            .all(|(id, qty)| view.amount_of(id) >= *qty);

    let can_decline = is_active
        && (now - (quest.expires_at - crate::game::quests::QUEST_DURATION))
            <= crate::game::quests::DECLINE_WINDOW;

    let class = match status {
        crate::game::quests::QuestStatus::Active => "quest-card active",
        crate::game::quests::QuestStatus::Failed => "quest-card failed",
        crate::game::quests::QuestStatus::Completed => "quest-card completed",
        _ => "quest-card",
    };

    rsx! {
        div { class: "{class}",
            div { class: "quest-author",
                "{i18n::tf_named(\"quest.author_line\", &[(\"author\", &author)])}"
            }
            div { class: "quest-title", "{title}" }
            div { class: "quest-body", "{body}" }

            if is_active {
                div { class: "quest-timer",
                    "{i18n::tf_named(\"quest.timer_line\", &[(\"time\", &timer_text)])}"
                }
            }

            div { class: "quest-actions",
                if is_available {
                    if declined {
                        div { class: "quest-declined", "{i18n::t(\"quest.status.declined\")}" }
                    } else {
                        button {
                            class: "trade-btn primary",
                            onclick: {
                                let quest_id = quest_id.clone();
                                move |_| {
                                    let result = {
                                        let mut guard = state.write();
                                        crate::game::quests::take_quest(&mut guard, &quest_id)
                                    };
                                    match result {
                                        Ok(()) => message.set(String::new()),
                                        Err(e) => message.set(e.text().to_string()),
                                    }
                                    refresh_view(state);
                                }
                            },
                            "{i18n::t(\"quest.btn.take\")}"
                        }
                    }
                } else if is_active {
                    button {
                        class: if can_complete { "trade-btn primary" } else { "trade-btn" },
                        disabled: !can_complete,
                        onclick: {
                            let quest_id = quest_id.clone();
                            move |_| {
                                let result = {
                                    let mut guard = state.write();
                                    crate::game::quests::complete_quest(&mut guard, &quest_id)
                                };
                                match result {
                                    Ok(()) => message.set(String::new()),
                                    Err(e) => message.set(e.text().to_string()),
                                }
                                refresh_view(state);
                            }
                        },
                        "{i18n::t(\"quest.btn.complete\")}"
                    }
                    button {
                        class: if can_decline { "trade-btn cancel" } else { "trade-btn cancel disabled" },
                        disabled: !can_decline,
                        onclick: {
                            let quest_id = quest_id.clone();
                            move |_| {
                                let result = {
                                    let mut guard = state.write();
                                    crate::game::quests::decline_quest(&mut guard, &quest_id)
                                };
                                match result {
                                    Ok(()) => message.set(String::new()),
                                    Err(e) => message.set(e.text().to_string()),
                                }
                                refresh_view(state);
                            }
                        },
                        "{i18n::t(\"quest.btn.decline\")}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_are_always_ten_characters() {
        assert_eq!(bar(0), "----------");
        assert_eq!(bar(50), "#####-----");
        assert_eq!(bar(100), "##########");
        assert_eq!(bar(120), "##########");
    }

    #[test]
    fn money_uses_space_separators() {
        assert_eq!(format_money(0.0), "0");
        assert_eq!(format_money(1240.0), "1 240");
        assert_eq!(format_money(1234567.0), "1 234 567");
    }

    #[test]
    fn signed_numbers_use_minus_sign() {
        assert_eq!(format_signed(5), "+5");
        assert_eq!(format_signed(0), "0");
        assert_eq!(format_signed(-3), "−3");
    }

    #[test]
    fn english_keeps_a_plain_hyphen() {
        let _guard = i18n::test_lock();
        i18n::set_lang(crate::game::i18n::Lang::En);
        assert_eq!(format_signed(-3), "-3");
        assert_eq!(format_money(-1240.0), "-1 240");
        assert_eq!(i18n::t("trade.title"), "TRADER: {}");
        i18n::set_lang(crate::game::i18n::Lang::Ru);
    }
}
