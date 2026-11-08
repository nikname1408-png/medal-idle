//! Полноэкранный экран торговцев, живой рынок и карточка товара.

use dioxus::prelude::*;

use crate::game::items::{self, Item};
use crate::game::state::GameState;
use crate::game::trader::{self, GoodState, MarketStatus};

use super::app::refresh_view;

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
    let mut filter = use_signal(|| Filter::All);
    let mut selected = use_signal(|| None::<String>);
    let mut qty = use_signal(|| 1u32);
    let mut message = use_signal(String::new);
    let mut handled_back = use_signal(|| *back.peek());

    use_effect(move || {
        let count = back();
        if count <= *handled_back.peek() {
            return;
        }
        handled_back.set(count);
        if selected.peek().is_some() {
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
        "Ничего не найдено.".to_string()
    } else if let Some(item_id) = &selected_now {
        items::item(item_id)
            .map(|item| format!("Выбрано: {}.", item.name))
            .unwrap_or_default()
    } else {
        "Нажми на товар, чтобы открыть карточку.".to_string()
    };

    rsx! {
        div {
            class: "trader-overlay full",
            div { class: "trader-window",
                div { class: "trade-book",
                    div { class: "trade-head",
                        span { class: "trade-title", "ТОРГОВЕЦ: {trader::trader_name(trader_now)}" }
                        span { class: "trade-gold", "Золото: {format_money(view.gold)}" }
                    }
                    div { class: "trade-subhead",
                        span { "Рынок живёт сам: спрос и предложение меняются каждые 5 секунд." }
                        button {
                            class: "trade-close",
                            onclick: move |_| on_close.call(()),
                            "[ X ] Закрыть"
                        }
                    }

                    div { class: "trade-trader-picker",
                        for (index, trader_def) in trader::TRADERS.iter().enumerate() {
                            button {
                                key: "{trader_def.id}",
                                class: if index == trader_now { "trade-trader active" } else { "trade-trader" },
                                onclick: move |_| {
                                    trader_index.set(index);
                                    selected.set(None);
                                    qty.set(1);
                                    message.set(String::new());
                                },
                                "{trader_def.name}"
                            }
                        }
                    }

                    div { class: "trade-controls",
                        div { class: "trade-tabs",
                            button {
                                class: if mode_now.is_buy() { "trade-tab active" } else { "trade-tab" },
                                onclick: move |_| {
                                    mode.set(Mode::Buy);
                                    selected.set(None);
                                    qty.set(1);
                                    message.set(String::new());
                                },
                                "КУПИТЬ"
                            }
                            button {
                                class: if mode_now.is_buy() { "trade-tab" } else { "trade-tab active" },
                                onclick: move |_| {
                                    mode.set(Mode::Sell);
                                    selected.set(None);
                                    qty.set(1);
                                    message.set(String::new());
                                },
                                "Продать"
                            }
                        }
                        label { class: "trade-field",
                            span { "Фильтр:" }
                            select {
                                class: "trade-select",
                                value: match filter_now {
                                    Filter::All => "all",
                                    Filter::Available => "available",
                                },
                                onchange: move |event| {
                                    filter.set(if event.value() == "available" {
                                        Filter::Available
                                    } else {
                                        Filter::All
                                    });
                                },
                                option { value: "all", "Все" }
                                option { value: "available", "Доступные" }
                            }
                        }
                    }

                    div { class: "trade-table",
                        div { class: "trade-row trade-head-row",
                            span { "ТОВАР" }
                            span { class: "num",
                                if mode_now.is_buy() { "ЗАП." } else { "У ВАС" }
                            }
                            span { class: "num", "КУП." }
                            span { class: "num", "ПРОД." }
                            span { class: "num", "С/П" }
                            span { class: "num", "РЫНОК" }
                        }
                        if visible.is_empty() {
                            div { class: "trade-empty", "Ничего не найдено." }
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

                    div { class: "trade-foot",
                        div { "С/П = спрос/предложение. Рынок: Д — дефицит, Б — баланс, И — избыток." }
                        div { class: "trade-hint", "{hint}" }
                        if !message_now.is_empty() {
                            div { class: "trade-error", "{message_now}" }
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
    let buy = trader::buy_price(&item, good.demand, good.supply);
    let sell = trader::sell_price(&item, good.demand, good.supply, view.trade_bonus);
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
    let item_id = item.id.clone();

    rsx! {
        button {
            class: if selected { "trade-row selected" } else { "trade-row" },
            onclick: move |_| on_select.call(item_id.clone()),
            span { class: "trade-name",
                span { class: "trade-cursor", if selected { ">" } else { " " } }
                span { "{item.name}" }
                if let Some(reason) = reason {
                    span { class: "trade-reason", " · {reason}" }
                }
            }
            span { class: "num", "{stock}" }
            span { class: "num", "{format_money(buy)}" }
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
        return rsx! { div {} };
    };

    let buy = trader::buy_price(&item, good.demand, good.supply);
    let sell = trader::sell_price(&item, good.demand, good.supply, view.trade_bonus);
    let margin = trader::margin(&item, good.demand, good.supply, view.trade_bonus);
    let status = good.status();
    let owned = view.item_count(&item.id);

    let space_limit = if item.stackable && (owned > 0 || view.inventory_free_slots() > 0) {
        u32::MAX
    } else {
        view.inventory_free_slots()
    };
    let max_qty = match mode {
        Mode::Buy => trader::max_buy(view.gold, good.stock, buy).min(space_limit),
        Mode::Sell => trader::max_sell(owned, good.buy_limit, view.gold, view.gold_cap(), sell),
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
        format!("{} зл.", format_money(total))
    } else {
        "—".to_string()
    };

    let error = match mode {
        Mode::Buy => {
            if good.stock == 0 {
                Some("Нет в наличии".to_string())
            } else if max_qty == 0 && space_limit == 0 {
                Some("В сумке нет места".to_string())
            } else if view.gold + 1e-9 < buy {
                Some("Недостаточно золота".to_string())
            } else if quantity == 0 {
                Some("Нечего покупать".to_string())
            } else {
                None
            }
        }
        Mode::Sell => {
            if owned == 0 {
                Some("У вас нет этого товара".to_string())
            } else if good.buy_limit == 0 {
                Some("Лимит скупки за цикл исчерпан".to_string())
            } else if view.gold + 1e-9 >= view.gold_cap() {
                Some("Лимит золота достигнут. Улучши банк.".to_string())
            } else if quantity == 0 {
                Some("Не хватает лимита золота на сделку.".to_string())
            } else {
                None
            }
        }
    };

    let advice_text = if mode == Mode::Sell && good.buy_limit == 0 {
        "Торговец этот товар больше не скупает."
    } else {
        advice(status, mode)
    };
    let can_primary = error.is_none() && quantity > 0;
    let can_quick = max_qty >= 1;
    let buy_item_id = item.id.clone();
    let buy_all_item_id = item.id.clone();
    let sell_item_id = item.id.clone();
    let sell_all_item_id = item.id.clone();
    let item_name = item.name.clone();
    let rarity = item.rarity();
    let message_now = message();

    rsx! {
        div { class: "trade-card",
            div { class: "trade-card-head",
                span { class: "trade-card-title", "{item_name}" }
                span { class: "trade-rarity", "[редкость: {rarity}]" }
            }

            div { class: "trade-line",
                span { "Цена покупки:" }
                span { class: "trade-line-value", "{format_money(buy)} зл." }
            }
            div { class: "trade-line",
                span { "Цена продажи:" }
                span { class: "trade-line-value",
                    "{format_money(sell)} зл.   (наценка: {format_money(margin)})"
                }
            }
            div { class: "trade-line",
                span { "Спрос:" }
                span { class: "trade-bar", "{bar(good.demand)}" }
                span { class: "num trade-line-value", "{good.demand}" }
            }
            div { class: "trade-line",
                span { "Предложение:" }
                span { class: "trade-bar", "{bar(good.supply)}" }
                span { class: "num trade-line-value", "{good.supply}" }
            }
            div { class: "trade-line",
                span { "Рынок:" }
                span { class: "trade-market", "{status.label()} {status.arrow()}" }
                span { class: "trade-line-muted",
                    "(спрос − предложение = {format_signed(good.diff())})"
                }
            }
            div { class: "trade-line",
                span { "Запас у торговца:" }
                span { class: "trade-line-value", "{good.stock}" }
            }
            div { class: "trade-line",
                span { "У вас:" }
                span { class: "trade-line-value", "{owned}" }
            }

            div { class: "trade-qty",
                span { class: "trade-qty-label", "Кол-во:" }
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
                    "[MAX: {max_qty}]"
                }
            }

            div { class: "trade-line trade-total",
                span { "Итого:" }
                span { class: "trade-line-value", "{total_text}" }
                span { class: "trade-line-muted", "Золото после: {format_money(gold_after)}" }
            }

            if let Some(error) = error {
                div { class: "trade-error", "{error}" }
            }
            if !message_now.is_empty() {
                div { class: "trade-error", "{message_now}" }
            }

            div { class: "trade-actions",
                if mode == Mode::Buy {
                    button {
                        class: if can_primary { "trade-btn primary" } else { "trade-btn" },
                        disabled: !can_primary,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::buy(&mut guard, trader_index, &buy_item_id, quantity)
                            };
                            message.set(match result {
                                Ok(()) => String::new(),
                                Err(error) => error.text().to_string(),
                            });
                            qty.set(1);
                            refresh_view(state);
                        },
                        "КУПИТЬ"
                    }
                    button {
                        class: "trade-btn",
                        disabled: !can_quick,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::buy(&mut guard, trader_index, &buy_all_item_id, max_qty)
                            };
                            message.set(match result {
                                Ok(()) => String::new(),
                                Err(error) => error.text().to_string(),
                            });
                            qty.set(1);
                            refresh_view(state);
                        },
                        "КУПИТЬ ×{max_qty}"
                    }
                } else {
                    button {
                        class: if can_primary { "trade-btn primary" } else { "trade-btn" },
                        disabled: !can_primary,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::sell(&mut guard, trader_index, &sell_item_id, quantity)
                            };
                            message.set(match result {
                                Ok(()) => String::new(),
                                Err(error) => error.text().to_string(),
                            });
                            qty.set(1);
                            refresh_view(state);
                        },
                        "ПРОДАТЬ"
                    }
                    button {
                        class: "trade-btn",
                        disabled: !can_quick,
                        onclick: move |_| {
                            let result = {
                                let mut guard = state.write();
                                trader::sell(&mut guard, trader_index, &sell_all_item_id, max_qty)
                            };
                            message.set(match result {
                                Ok(()) => String::new(),
                                Err(error) => error.text().to_string(),
                            });
                            qty.set(1);
                            refresh_view(state);
                        },
                        "ПРОДАТЬ ×{max_qty}"
                    }
                }
                button {
                    class: "trade-btn cancel",
                    onclick: move |_| on_close.call(()),
                    "Отмена"
                }
            }

            div { class: "trade-advice", "Совет: {advice_text}" }
        }
    }
}

fn is_available(view: &GameState, mode: Mode, good: &GoodState) -> bool {
    let Some(item) = items::item(&good.item_id) else {
        return false;
    };
    match mode {
        Mode::Buy => {
            let price = trader::buy_price(item, good.demand, good.supply);
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
            let price = trader::buy_price(item, good.demand, good.supply);
            if good.stock == 0 {
                Some("нет в наличии")
            } else if view.gold + 1e-9 < price {
                Some("нет золота")
            } else if !view.can_add_item(&item.id, 1) {
                Some("нет места")
            } else {
                None
            }
        }
        Mode::Sell => {
            if good.buy_limit == 0 {
                Some("не скупает")
            } else if view.item_count(&good.item_id) == 0 {
                Some("нечего продать")
            } else if view.gold + 1e-9 >= view.gold_cap() {
                Some("лимит золота")
            } else {
                None
            }
        }
    }
}

fn advice(status: MarketStatus, mode: Mode) -> &'static str {
    match (status, mode) {
        (MarketStatus::Deficit, Mode::Buy) => {
            "Дефицит — цена выросла. Покупка сейчас дороже обычного."
        }
        (MarketStatus::Deficit, Mode::Sell) => {
            "Дефицит — цена продажи выше обычной. Хороший момент продать."
        }
        (MarketStatus::Balance, _) => "Рынок в равновесии — цена стабильна.",
        (MarketStatus::Surplus, Mode::Buy) => {
            "Избыток — товара много, цена низкая. Хороший момент для закупки."
        }
        (MarketStatus::Surplus, Mode::Sell) => {
            "Избыток — торговец даёт меньше за единицу. Продавать сейчас невыгодно."
        }
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
        negative => format!("−{}", -negative),
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
        format!("−{output}")
    } else {
        output
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
}
