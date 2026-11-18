//! Динамические события: обмены (монетка), баффы/дебаффы и оффлайн-сюрпризы.
//!
//! Модуль не зависит от Dioxus: события проверяются через `cargo test`.
//! Кризисы и сюжетные события — следующим шагом, здесь только инфраструктура
//! и два «повседневных» триггера.

use serde::{Deserialize, Serialize};

use super::actions::next_random;
use super::balance;
use super::i18n;
use super::items;
use super::state::{is_active_resource, GameState};

/// Минимальная доля запаса, которую берёт оптовик.
const BULK_MIN_SHARE: f64 = 0.10;
/// Максимальная доля запаса, которую берёт оптовик.
const BULK_MAX_SHARE: f64 = 1.00;
/// Насколько оптовик платит выше базовой цены (× 1.1).
const BULK_PRICE_MULT: f64 = 1.10;

/// Минимальный множитель коллекционера.
const COLLECTOR_MIN_MULT: f64 = 1.15;
/// Максимальный множитель коллекционера.
const COLLECTOR_MAX_MULT: f64 = 2.00;

/// Минимальная доля запаса ресурса, которую скупает коллекционер.
const COLLECTOR_MIN_SHARE: f64 = 0.20;
/// Максимальная доля запаса ресурса, которую скупает коллекционер.
const COLLECTOR_MAX_SHARE: f64 = 0.50;
/// Минимальная партия коллекционера: мелочь он не берёт.
const COLLECTOR_MIN_UNITS: u32 = 10;

/// Порог «оффлайн 2+ часа» из концепта.
const OFFLINE_MIN_HOURS: f64 = 2.0;
/// Через сколько часов отсутствия шанс сюрприза становится 100%.
const OFFLINE_FULL_HOURS: f64 = 4.0;

/// Стат, на который действует бафф.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuffStat {
    /// +N к регенерации энергии в секунду.
    EnergyRegenFlat,
    /// +N% к скорости походов.
    ExpeditionSpeed,
    /// N% к добыче лесопилки (для дебаффа значение отрицательное).
    SawmillMult,
    /// +N% к расходу энергии (дебафф).
    EnergyCostMult,
    /// N% к добыче шахты (для дебаффа значение отрицательное).
    MineMult,
}

/// Активный бафф или дебафф.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuffInstance {
    /// `id` из [`BUFFS`] — по нему берутся `buff.<id>.name` / `buff.<id>.desc`.
    pub buff_id: String,
    pub stat: BuffStat,
    pub value: f64,
    /// Момент окончания в базе `GameState::event_clock`.
    pub expires_at: f64,
    /// Переживает ли перерождение.
    pub global: bool,
}

impl BuffInstance {
    pub fn name(&self) -> &'static str {
        buff_name(&self.buff_id)
    }

    pub fn summary(&self) -> String {
        buff_desc(&self.buff_id, self.value)
    }

    pub fn is_active(&self, now: f64) -> bool {
        self.expires_at > now
    }

    pub fn remaining_secs(&self, now: f64) -> f64 {
        (self.expires_at - now).max(0.0)
    }
}

/// Определение баффа: диапазон значений и длительности.
#[derive(Clone, Copy, Debug)]
pub struct BuffDef {
    pub id: &'static str,
    pub stat: BuffStat,
    pub value_min: f64,
    pub value_max: f64,
    pub secs_min: f64,
    pub secs_max: f64,
    pub global: bool,
}

/// Пул баффов и дебаффов из концепта.
pub const BUFFS: [BuffDef; 6] = [
    BuffDef {
        id: "good_mood",
        stat: BuffStat::EnergyRegenFlat,
        value_min: 1.0,
        value_max: 3.0,
        secs_min: 180.0,
        secs_max: 300.0,
        global: false,
    },
    BuffDef {
        id: "second_wind",
        stat: BuffStat::ExpeditionSpeed,
        value_min: 5.0,
        value_max: 15.0,
        secs_min: 120.0,
        secs_max: 240.0,
        global: false,
    },
    BuffDef {
        id: "mosquitoes",
        stat: BuffStat::SawmillMult,
        value_min: -30.0,
        value_max: -10.0,
        secs_min: 180.0,
        secs_max: 300.0,
        global: false,
    },
    BuffDef {
        id: "clumsy_hands",
        stat: BuffStat::EnergyCostMult,
        value_min: 10.0,
        value_max: 30.0,
        secs_min: 120.0,
        secs_max: 180.0,
        global: false,
    },
    BuffDef {
        id: "after_fire",
        stat: BuffStat::EnergyRegenFlat,
        value_min: -1.0,
        value_max: -1.0,
        secs_min: 180.0,
        secs_max: 240.0,
        global: false,
    },
    BuffDef {
        id: "broken_tool",
        stat: BuffStat::MineMult,
        value_min: -20.0,
        value_max: -20.0,
        secs_min: 120.0,
        secs_max: 180.0,
        global: false,
    },
];

/// Полезная нагрузка события.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    /// Оптовик: скупает `amount` единиц ресурса за `gold` золота.
    MerchantBulk {
        resource_id: String,
        amount: u32,
        gold: u32,
    },
    /// Коллекционер: покупает `quantity` единиц предмета за `gold`.
    MerchantCollector {
        item_id: String,
        quantity: u32,
        gold: u32,
    },
    /// Бафф уже применён к состоянию (см. [`apply_buff`]).
    BuffApplied {
        buff_id: String,
        value: f64,
        duration_secs: u32,
    },
    /// Оффлайн: «Крысы на складе».
    OfflineRats { lost_wood: u32 },
    /// Оффлайн: «Пока вас не было, работники нашли что-то в шахте».
    OfflineMineFind { iron: u32 },
    /// «В грязи»: находка. Одна кнопка «Забрать».
    CrisisMud { item_id: String, quantity: u32 },
    /// «Пожар»: две кнопки — камень или энергия + дебафф.
    CrisisFire,
    /// «Воры»: две кнопки — рискнуть или заплатить стражe.
    CrisisThieves,
    /// «Сломался инструмент»: две кнопки — починить или сколотить.
    CrisisToolBroken,
    /// Сюжетное событие. `id` — ключ `event.thematic.<id>.title/body`.
    Thematic { id: String },
    /// Коллекторы забрали предметы в счёт долга.
    CollectorsArrived {
        gold_taken: f64,
        items: Vec<(String, u32)>,
        remaining_debt: f64,
    },
}

/// Событие, которое сейчас на экране.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingEvent {
    pub payload: EventPayload,
}

/// Отложенный триггер в базе `event_clock`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionTrigger {
    pub fire_at: f64,
    pub kind: TriggerKind,
}

/// Вид триггера. Кризисы и сюжетка добавляются сюда же.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TriggerKind {
    Baseline,
    /// Сюжетное событие: срабатывает через 5–10 минут после действия.
    Thematic {
        id: String,
    },
}

/// Ошибка разрешения события.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    NoPending,
    CannotAfford,
}

// ---------- Работа с баффами ----------

/// Сумма активных значений баффов по стату.
pub fn active_buff_sum(state: &GameState, stat: BuffStat) -> f64 {
    let now = state.event_clock;
    state
        .buffs
        .iter()
        .filter(|buff| buff.stat == stat && buff.is_active(now))
        .map(|buff| buff.value)
        .sum()
}

/// Применяет бафф. Повторный вызов с тем же `buff_id` обновляет таймер
/// и берёт значение с большим модулем (сильнее и бафф, и дебафф).
pub fn apply_buff(
    state: &mut GameState,
    buff_id: &str,
    value: f64,
    duration_secs: f64,
    global: bool,
) {
    let Some(def) = BUFFS.iter().find(|buff| buff.id == buff_id) else {
        return;
    };
    let now = state.event_clock;
    if let Some(existing) = state.buffs.iter_mut().find(|b| b.buff_id == buff_id) {
        existing.expires_at = existing.expires_at.max(now + duration_secs);
        if value.abs() > existing.value.abs() {
            existing.value = value;
        }
        return;
    }
    state.buffs.push(BuffInstance {
        buff_id: buff_id.to_string(),
        stat: def.stat,
        value,
        expires_at: now + duration_secs,
        global,
    });
}

pub fn buff_name(buff_id: &str) -> &'static str {
    i18n::t_or(&format!("buff.{}.name", buff_id), "")
}

pub fn buff_desc(buff_id: &str, value: f64) -> String {
    let key = format!("buff.{}.desc", buff_id);
    let text = if value.abs() < 1.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    i18n::tf(&key, &[&text])
}

// ---------- Утилиты ГПСЧ ----------

fn roll_unit(seed: &mut u64) -> f64 {
    (next_random(seed) >> 33) as f64 / (u64::MAX >> 33) as f64
}

fn roll_range(seed: &mut u64, min: f64, max: f64) -> f64 {
    min + (max - min) * roll_unit(seed)
}

fn roll_range_u32(seed: &mut u64, min: u32, max: u32) -> u32 {
    if max <= min {
        return min;
    }
    min + (next_random(seed) as u32) % (max - min + 1)
}

// ---------- Планирование ----------

/// Откладывает событие на `delay_secs` секунд игрового времени.
pub fn schedule(state: &mut GameState, kind: TriggerKind, delay_secs: f64) {
    state.action_queue.push(ActionTrigger {
        fire_at: state.event_clock + delay_secs.max(0.0),
        kind,
    });
}

/// Откладывает тематическое событие на случайную задержку в `[min_secs, max_secs]`.
pub fn schedule_thematic(state: &mut GameState, id: &str, min_secs: f64, max_secs: f64) {
    let mut seed = state.market_seed;
    let delay = roll_range(&mut seed, min_secs, max_secs);
    state.market_seed = seed;
    schedule(state, TriggerKind::Thematic { id: id.to_string() }, delay);
}

/// Тик: снимает истёкшие баффы, выбирает следующее событие.
///
/// Порядок приоритетов: оффлайн → action-based → baseline.
pub fn tick(state: &mut GameState, _dt: f64) {
    let now = state.event_clock;
    state.buffs.retain(|buff| buff.expires_at > now);

    if state.pending_event.is_some() {
        return;
    }

    // Оффлайн-очередь — вне конкуренции с фоновыми.
    if !state.offline_events.is_empty() {
        state.pending_event = Some(state.offline_events.remove(0));
        return;
    }

    // Action-based приоритетнее фонового.
    if let Some(pos) = state.action_queue.iter().position(|t| now >= t.fire_at) {
        let trigger = state.action_queue.remove(pos);
        match trigger.kind {
            TriggerKind::Baseline => {
                if let Some(event) = spawn_baseline(state) {
                    state.pending_event = Some(event);
                }
            }
            TriggerKind::Thematic { id } => {
                state.pending_event = Some(PendingEvent {
                    payload: EventPayload::Thematic { id },
                });
            }
        }
        // Любое событие сбрасывает фоновый таймер.
        reset_baseline_timer(state);
        return;
    }

    // Фоновый рандом.
    if now >= state.next_baseline_at {
        let mut seed = state.market_seed;
        let chance = next_random(&mut seed) % 100;
        state.market_seed = seed;

        if chance < balance::EVENTS_BASELINE_CHANCE {
            if let Some(event) = spawn_baseline(state) {
                state.pending_event = Some(event);
            }
        }
        reset_baseline_timer(state);
    }
}

fn reset_baseline_timer(state: &mut GameState) {
    let mut seed = state.market_seed;
    let delay = roll_range(
        &mut seed,
        balance::EVENTS_BASELINE_MIN_SECS,
        balance::EVENTS_BASELINE_MAX_SECS,
    );
    state.market_seed = seed;
    state.next_baseline_at = state.event_clock + delay;
}

// ---------- Генерация ----------

/// Строит случайное фоновое событие. Бафф всегда срабатывает как fallback.
pub fn spawn_baseline(state: &mut GameState) -> Option<PendingEvent> {
    let mut seed = state.market_seed;
    let roll = next_random(&mut seed) % 100;
    state.market_seed = seed;

    if roll < 25 {
        spawn_bulk(state)
            .or_else(|| spawn_collector(state))
            .or_else(|| spawn_buff(state))
    } else if roll < 50 {
        spawn_collector(state)
            .or_else(|| spawn_bulk(state))
            .or_else(|| spawn_buff(state))
    } else if roll < 80 {
        spawn_buff(state)
    } else {
        spawn_crisis(state).or_else(|| spawn_buff(state))
    }
}

fn spawn_bulk(state: &mut GameState) -> Option<PendingEvent> {
    // Оптовик берёт только предметы из рюкзака — дерево, камень и прочие
    // активные ресурсы со склада ему не нужны. Выбираем то, чего в
    // рюкзаке больше всего штук.
    let mut best: Option<(String, u32, u32)> = None; // (id, qty, base_price)
    for (item_id, qty) in &state.inventory {
        if *qty == 0 {
            continue;
        }
        // Карты и чертежи не продаём: они нужны игроку.
        if item_id.starts_with("map_")
            || item_id == "steam_blueprint"
            || item_id == "steam_prototype"
        {
            continue;
        }
        let Some(item) = items::item(item_id) else {
            continue;
        };
        if item.base_price == 0 {
            continue;
        }
        if best.as_ref().map(|(_, bq, _)| *qty > *bq).unwrap_or(true) {
            best = Some((item_id.clone(), *qty, item.base_price));
        }
    }

    let (resource_id, stock, base_price) = best?;
    if stock == 0 {
        return None;
    }

    let mut seed = state.market_seed;
    let share = roll_range(&mut seed, BULK_MIN_SHARE, BULK_MAX_SHARE);
    let unit_price = base_price as f64 * BULK_PRICE_MULT;
    let gold_room = (state.gold_cap() - state.gold).max(0.0);
    let max_by_gold = (gold_room / unit_price).floor() as u32;
    let raw_amount = (share * stock as f64).floor() as u32;
    let amount = raw_amount.min(max_by_gold).min(stock);
    state.market_seed = seed;

    if amount == 0 {
        return None;
    }
    let total = (amount as f64 * unit_price).round() as u32;
    if total < balance::EVENTS_BULK_MIN_GOLD {
        return None;
    }

    Some(PendingEvent {
        payload: EventPayload::MerchantBulk {
            resource_id,
            amount,
            gold: total,
        },
    })
}

fn spawn_collector(state: &mut GameState) -> Option<PendingEvent> {
    // Коллекционер покупает самый дорогой предмет рюкзака (по цене за
    // штуку). Дерево, камень и уголь со склада его не интересуют, карты и
    // чертежи он не трогает.
    let mut best: Option<(String, u32, u32)> = None; // (id, stock, base_price)
    for (item_id, qty) in &state.inventory {
        if *qty == 0 {
            continue;
        }
        if item_id.starts_with("map_")
            || item_id == "steam_blueprint"
            || item_id == "steam_prototype"
        {
            continue;
        }
        let Some(item) = items::item(item_id) else {
            continue;
        };
        if item.base_price == 0 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, _, best_price)| item.base_price > *best_price)
            .unwrap_or(true)
        {
            best = Some((item_id.clone(), *qty, item.base_price));
        }
    }
    let (item_id, stock, base_price) = best?;

    let mut seed = state.market_seed;
    let mult = roll_range(&mut seed, COLLECTOR_MIN_MULT, COLLECTOR_MAX_MULT);
    let unit_price = base_price as f64 * mult;
    let share = roll_range(&mut seed, COLLECTOR_MIN_SHARE, COLLECTOR_MAX_SHARE);
    let gold_room = (state.gold_cap() - state.gold).max(0.0);
    let max_by_gold = (gold_room / unit_price).floor() as u32;
    let raw_amount = (share * stock as f64).ceil() as u32;
    let buy_qty = raw_amount
        .max(COLLECTOR_MIN_UNITS)
        .min(max_by_gold)
        .min(stock);
    state.market_seed = seed;

    if buy_qty == 0 {
        return None;
    }
    let total = (buy_qty as f64 * unit_price).round() as u32;

    Some(PendingEvent {
        payload: EventPayload::MerchantCollector {
            item_id,
            quantity: buy_qty,
            gold: total,
        },
    })
}

fn spawn_buff(state: &mut GameState) -> Option<PendingEvent> {
    if BUFFS.is_empty() {
        return None;
    }
    let mut seed = state.market_seed;
    let idx = (next_random(&mut seed) as usize) % BUFFS.len();
    let def = &BUFFS[idx];
    let value = roll_range(&mut seed, def.value_min, def.value_max);
    let secs = roll_range(&mut seed, def.secs_min, def.secs_max).round();
    state.market_seed = seed;

    apply_buff(state, def.id, value, secs, def.global);

    Some(PendingEvent {
        payload: EventPayload::BuffApplied {
            buff_id: def.id.to_string(),
            value,
            duration_secs: secs as u32,
        },
    })
}

// ---------- Разрешение ----------

fn spawn_crisis(state: &mut GameState) -> Option<PendingEvent> {
    let mut seed = state.market_seed;
    let roll = next_random(&mut seed) % 100;
    state.market_seed = seed;

    if roll < 25 {
        spawn_mud(state)
    } else if roll < 55 {
        Some(PendingEvent {
            payload: EventPayload::CrisisFire,
        })
    } else if roll < 80 {
        Some(PendingEvent {
            payload: EventPayload::CrisisThieves,
        })
    } else {
        Some(PendingEvent {
            payload: EventPayload::CrisisToolBroken,
        })
    }
}

fn spawn_mud(state: &mut GameState) -> Option<PendingEvent> {
    let mut seed = state.market_seed;
    let roll = next_random(&mut seed) % 100;
    let (item_id, quantity) = if roll < 50 {
        ("wood", roll_range_u32(&mut seed, 5, 20))
    } else if roll < 90 {
        ("clay", roll_range_u32(&mut seed, 2, 8))
    } else if roll < 98 {
        ("iron", roll_range_u32(&mut seed, 1, 5))
    } else {
        ("silver", roll_range_u32(&mut seed, 1, 2))
    };
    state.market_seed = seed;

    Some(PendingEvent {
        payload: EventPayload::CrisisMud {
            item_id: item_id.to_string(),
            quantity,
        },
    })
}

/// Разрешает кризис по индексу выбранной кнопки.
///
/// `0` — левая кнопка, `1` — правая, `2` — «Пропустить» (принять последствия).
/// Для «В грязи» и сюжетных событий всегда `choice == 0`.
pub fn resolve_choice(state: &mut GameState, choice: usize) -> Result<(), EventError> {
    let Some(event) = state.pending_event.take() else {
        return Err(EventError::NoPending);
    };
    match event.payload {
        EventPayload::CrisisMud { item_id, quantity } => {
            // Одна кнопка: забрать находку. Если нет места — предмет теряется.
            if is_active_resource(&item_id) {
                state.add_active(&item_id, quantity);
            } else {
                let _ = state.add_item(&item_id, quantity);
            }
            state.refresh_unlocks();
            Ok(())
        }
        EventPayload::CrisisFire => match choice {
            0 => {
                let cost = balance::CRISIS_FIRE_STONE;
                if state.active_amount("stone") < cost {
                    state.pending_event = Some(PendingEvent {
                        payload: EventPayload::CrisisFire,
                    });
                    return Err(EventError::CannotAfford);
                }
                state.spend_active("stone", cost);
                Ok(())
            }
            1 => {
                let cost = balance::CRISIS_FIRE_ENERGY as f64;
                if state.energy + 1e-9 < cost {
                    state.pending_event = Some(PendingEvent {
                        payload: EventPayload::CrisisFire,
                    });
                    return Err(EventError::CannotAfford);
                }
                state.energy = (state.energy - cost).max(0.0);
                let mut seed = state.market_seed;
                let secs = roll_range(
                    &mut seed,
                    balance::CRISIS_FIRE_DEBUFF_MIN_SECS,
                    balance::CRISIS_FIRE_DEBUFF_MAX_SECS,
                );
                state.market_seed = seed;
                apply_buff(state, "after_fire", -1.0, secs, false);
                // Показываем игроку, что он получил дебафф
                state.pending_event = Some(PendingEvent {
                    payload: EventPayload::BuffApplied {
                        buff_id: "after_fire".to_string(),
                        value: -1.0,
                        duration_secs: secs as u32,
                    },
                });
                Ok(())
            }
            _ => Ok(()),
        },
        EventPayload::CrisisThieves => match choice {
            0 => {
                let wood = state.active_amount("wood");
                let lost = (wood as f64 * balance::CRISIS_THIEVES_WOOD_PERCENT as f64 / 100.0)
                    .round() as u32;
                if lost > 0 {
                    state.spend_active("wood", lost);
                }
                let _ = state.add_item("iron", balance::CRISIS_THIEVES_IRON);
                Ok(())
            }
            1 => {
                let cost = balance::CRISIS_THIEVES_GOLD as f64;
                if state.gold + 1e-9 < cost {
                    state.pending_event = Some(PendingEvent {
                        payload: EventPayload::CrisisThieves,
                    });
                    return Err(EventError::CannotAfford);
                }
                state.gold = (state.gold - cost).max(0.0);
                Ok(())
            }
            _ => Ok(()),
        },
        EventPayload::CrisisToolBroken => match choice {
            0 => {
                let cost = balance::CRISIS_TOOL_IRON;
                if state.item_count("iron") < cost {
                    state.pending_event = Some(PendingEvent {
                        payload: EventPayload::CrisisToolBroken,
                    });
                    return Err(EventError::CannotAfford);
                }
                state.remove_item("iron", cost);
                Ok(())
            }
            1 => {
                let mut seed = state.market_seed;
                let secs = roll_range(
                    &mut seed,
                    balance::CRISIS_TOOL_DEBUFF_MIN_SECS,
                    balance::CRISIS_TOOL_DEBUFF_MAX_SECS,
                );
                state.market_seed = seed;
                apply_buff(state, "broken_tool", -20.0, secs, false);
                // Показываем игроку, что он получил дебафф
                state.pending_event = Some(PendingEvent {
                    payload: EventPayload::BuffApplied {
                        buff_id: "broken_tool".to_string(),
                        value: -20.0,
                        duration_secs: secs as u32,
                    },
                });
                Ok(())
            }
            _ => Ok(()),
        },
        EventPayload::Thematic { .. } => Ok(()),
        // Не кризис — вернуть событие и не трогать.
        _ => {
            state.pending_event = Some(event);
            Err(EventError::NoPending)
        }
    }
}

/// Закрывает обычное (не кризисное) событие.
/// `accepted == false` — «Позже/Нет»: эффекта нет.
pub fn resolve(state: &mut GameState, accepted: bool) -> Result<(), EventError> {
    let Some(event) = state.pending_event.take() else {
        return Err(EventError::NoPending);
    };
    if !accepted {
        return Ok(());
    }
    match event.payload.clone() {
        EventPayload::MerchantBulk {
            resource_id,
            amount,
            gold,
        } => {
            if is_active_resource(&resource_id) {
                if state.active_amount(&resource_id) < amount {
                    state.pending_event = Some(event);
                    return Err(EventError::CannotAfford);
                }
                state.spend_active(&resource_id, amount);
            } else {
                if state.amount_of(&resource_id) < amount {
                    state.pending_event = Some(event);
                    return Err(EventError::CannotAfford);
                }
                state.remove_item(&resource_id, amount);
            }
            let before = state.gold;
            state.gold = (state.gold + gold as f64).min(state.gold_cap());
            state.stats.gold_earned += state.gold - before;
            state.refresh_unlocks();
            Ok(())
        }
        EventPayload::MerchantCollector {
            item_id,
            quantity,
            gold,
        } => {
            if state.amount_of(&item_id) < quantity {
                state.pending_event = Some(event);
                return Err(EventError::CannotAfford);
            }
            if is_active_resource(&item_id) {
                state.spend_active(&item_id, quantity);
            } else {
                state.remove_item(&item_id, quantity);
            }
            let before = state.gold;
            state.gold = (state.gold + gold as f64).min(state.gold_cap());
            state.stats.gold_earned += state.gold - before;
            state.refresh_unlocks();
            Ok(())
        }
        // Бафф уже наложен при генерации события.
        EventPayload::BuffApplied { .. } => Ok(()),
        // Коллекторы уже забрали предметы в `tick_collectors`, тут только
        // закрываем модалку.
        EventPayload::CollectorsArrived { .. } => Ok(()),
        _ => Ok(()),
    }
}

// ---------- Оффлайн ----------

/// Проверка «пока вас не было». Кладёт не больше одного события в очередь.
pub fn check_offline(state: &mut GameState, hours_away: f64) {
    if hours_away < OFFLINE_MIN_HOURS {
        return;
    }
    let chance = ((hours_away - OFFLINE_MIN_HOURS) / (OFFLINE_FULL_HOURS - OFFLINE_MIN_HOURS))
        .clamp(0.0, 1.0);
    let mut seed = state.market_seed;
    if roll_unit(&mut seed) >= chance {
        state.market_seed = seed;
        return;
    }

    // Одно из двух: крысы или находка. Без шахты — всегда крысы.
    let kind = if state.mine_level == 0 {
        0
    } else {
        next_random(&mut seed) % 2
    };
    state.market_seed = seed;

    match kind {
        0 => {
            let wood = state.active_amount("wood");
            if wood < 10 {
                return;
            }
            let mut seed = state.market_seed;
            let lost = (wood as f64 * roll_range(&mut seed, 0.10, 0.30)).round() as u32;
            state.market_seed = seed;
            if lost > 0 && state.spend_active("wood", lost) {
                state.offline_events.push(PendingEvent {
                    payload: EventPayload::OfflineRats { lost_wood: lost },
                });
            }
        }
        _ => {
            let mut seed = state.market_seed;
            let iron = roll_range_u32(&mut seed, 2, 10);
            state.market_seed = seed;
            if state.add_item("iron", iron) {
                state.stats.iron_mined += iron as f64;
                state.offline_events.push(PendingEvent {
                    payload: EventPayload::OfflineMineFind { iron },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::state::GameState;

    #[test]
    fn bulk_pays_above_base_price() {
        let mut state = GameState {
            gold: 0.0,
            ..GameState::default()
        };
        // Оптовика интересует рюкзак, а не склад: дерево/камень ему не нужны.
        state.add_active("wood", 500);
        state.add_item("cloth", 500);
        let event = spawn_bulk(&mut state).expect("оптовик");
        match event.payload {
            EventPayload::MerchantBulk {
                resource_id,
                amount,
                gold,
            } => {
                assert_eq!(resource_id, "cloth");
                assert!(amount >= 10 && amount <= 500);
                assert!(gold > 0);
            }
            _ => panic!("не тот тип"),
        }
    }

    #[test]
    fn bulk_respects_gold_cap() {
        let mut state = GameState {
            gold: 900.0,
            ..GameState::default()
        };
        state.add_active("wood", 10_000);
        state.add_item("cloth", 1000);
        // Остаток до 1000 = 100 золота; 100 / 12.1 ≈ 8 единиц.
        let event = spawn_bulk(&mut state).expect("оптовик");
        match event.payload {
            EventPayload::MerchantBulk { amount, gold, .. } => {
                assert!(gold <= 100);
                assert!(amount <= 8);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn collector_buys_the_most_expensive_bag_item() {
        let mut state = GameState::default();
        state.add_item("cloth", 500);
        state.add_item("sickle", 2);
        // Ресурсы со склада коллекционера не интересуют.
        state.add_active("wood", 500);
        state.add_active("stone", 100);

        let event = spawn_collector(&mut state).expect("коллекционер");
        match event.payload {
            EventPayload::MerchantCollector {
                item_id,
                quantity,
                gold,
            } => {
                // Серп (96 за штуку) дороже ткани (11), несмотря на запас.
                assert_eq!(item_id, "sickle");
                assert!(quantity <= 2);
                assert!(quantity >= 1);
                assert!(gold >= (quantity as f64 * 96.0 * 1.15).round() as u32);
            }
            _ => panic!(),
        }

        // Без предметов в рюкзаке коллекционеру нечего предложить.
        let mut store_only = GameState::default();
        store_only.add_active("wood", 500);
        assert!(spawn_collector(&mut store_only).is_none());
    }

    #[test]
    fn buff_refreshes_timer_and_takes_stronger_value() {
        let mut state = GameState::default();
        state.event_clock = 100.0;
        apply_buff(&mut state, "good_mood", 1.0, 60.0, false);
        apply_buff(&mut state, "good_mood", 3.0, 60.0, false);
        assert_eq!(state.buffs.len(), 1);
        assert_eq!(state.buffs[0].value, 3.0);
        assert_eq!(state.buffs[0].expires_at, 160.0);
    }

    #[test]
    fn expired_buffs_are_pruned() {
        let mut state = GameState::default();
        state.event_clock = 100.0;
        apply_buff(&mut state, "good_mood", 1.0, 10.0, false);
        state.event_clock = 200.0;
        tick(&mut state, 0.0);
        assert!(state.buffs.is_empty());
    }

    #[test]
    fn resolve_bulk_transfers_resource_and_gold() {
        let mut state = GameState::default();
        state.storage_level = 4;
        state.add_active("wood", 200);
        state.pending_event = Some(PendingEvent {
            payload: EventPayload::MerchantBulk {
                resource_id: "wood".to_string(),
                amount: 50,
                gold: 110,
            },
        });
        assert_eq!(resolve(&mut state, true), Ok(()));
        assert_eq!(state.active_amount("wood"), 150);
        assert!((state.gold - 110.0).abs() < 1e-9);
        assert!(state.pending_event.is_none());
    }

    #[test]
    fn resolve_declined_leaves_state_alone() {
        let mut state = GameState::default();
        state.storage_level = 4;
        state.add_active("wood", 200);
        state.pending_event = Some(PendingEvent {
            payload: EventPayload::MerchantBulk {
                resource_id: "wood".to_string(),
                amount: 50,
                gold: 110,
            },
        });
        assert_eq!(resolve(&mut state, false), Ok(()));
        assert_eq!(state.active_amount("wood"), 200);
        assert_eq!(state.gold, 0.0);
        assert!(state.pending_event.is_none());
    }

    #[test]
    fn global_buffs_survive_rebirth() {
        let mut state = GameState::default();
        state.total_play_seconds = 100.0;
        apply_buff(&mut state, "good_mood", 2.0, 60.0, false);
        apply_buff(&mut state, "second_wind", 10.0, 60.0, true);
        state.reset_for_cycle();
        assert_eq!(state.buffs.len(), 1);
        assert_eq!(state.buffs[0].buff_id, "second_wind");
    }
}
