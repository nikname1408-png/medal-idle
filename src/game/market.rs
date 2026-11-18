//! Общее ядро рынка для торговцев и города.
//!
//! У обоих одна и та же механика: спрос и предложение плавают шумом,
//! тянутся к настроению дня и возвращаются к равновесию одним шагом.
//! Разница только в масштабе (см. `market_scale`).
//!
//! Настроение — настоящий блуждающий тренд с памятью: он двигается
//! небольшим случайным шагом и сам возвращается к нулю, а не бросается
//! заново каждый раз. Из-за этого дефицит и избыток держатся часами,
//! а не на один случайный бросок.
//!
//! Модуль не знает про `GameState`, Dioxus и прочие части игры: только
//! числа и сид ГПСЧ.

use super::trader;

/// Предел хранимого тренда.
pub const TREND_LIMIT: i32 = 20;

/// Сколько дешёвых товаров копится в одной единице ёмкости.
fn cheap_unit() -> f64 {
    super::balance::DEMAND_UNIT_BUDGET
        / super::balance::DEMAND_UNIT_PRICE.max(1.0)
}

/// Один цех города: сколько единиц ёмкости приходится на одну единицу
/// тренда. Дешёвый товар ещё и делится на `DEMAND_UNIT_BUDGET`, иначе
/// свечи и дрова «съедали» бы весь бюджет спроса.
pub fn capacity_per_trend(item_id: &str) -> f64 {
    let price = super::items::item(item_id)
        .map(|item| item.base_price as f64)
        .unwrap_or(1.0)
        .max(1.0);
    let cheap = (super::balance::DEMAND_UNIT_BUDGET / price).max(1.0);
    cheap_unit() * cheap * super::balance::DEMAND_CAPACITY_PER_TREND
}

/// Один шаг настроения: случайный дрейф, притяжение к равновесию и малое
/// тяготение к настроению дня.
///
/// Дрейф — основной источник движения; равновесие ловит тренд, если он
/// уехал слишком далеко, а тяга к настроению дня не даёт рынку навсегда
/// застрять в избытке или дефиците. Все шаги в целых пунктах.
pub fn drift_trend(trend: i32, seed: &mut u64) -> i32 {
    let drift = trader::random_below(seed, (TREND_LIMIT * 2 + 1) as u64) as i32 - TREND_LIMIT;
    let pulled = trader::random_below(seed, 5) as i32 - 2;
    (trend + drift + pulled).clamp(-TREND_LIMIT, TREND_LIMIT)
}

/// Возврат значения к цели на `step` пунктов.
pub fn approach(value: u8, target: u8, step: u8) -> u8 {
    if value < target {
        value.saturating_add(step).min(target)
    } else {
        value.saturating_sub(step).max(target)
    }
}

/// Шум в диапазоне `-range..=range`.
pub fn noise(seed: &mut u64, range: i32) -> i32 {
    if range <= 0 {
        return 0;
    }
    let value = trader::random_below(seed, (range * 2 + 1) as u64) as i32;
    value - range
}

/// Статус рынка по разнице спроса и предложения.
///
/// `>= +16` — дефицит, `<= -16` — избыток, между ними — баланс.
/// Тот же порог, что и `MARKET_TREND_THRESHOLD` в балансе.
pub fn status(demand: u8, supply: u8) -> trader::MarketStatus {
    trader::market_status(demand, supply)
}

/// Доля налитого спроса 0..1 — для полоски в интерфейсе.
pub fn ratio(available: f64, capacity: f64) -> f64 {
    if capacity <= 0.0 {
        0.0
    } else {
        (available / capacity).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_walks_smoothly_and_visits_both_sides() {
        let mut seed = 0x5eed_1234;
        let mut trend = 0;
        let mut visits = Vec::new();
        let mut biggest_jump = 0;
        for _ in 0..2_000 {
            let next = drift_trend(trend, &mut seed);
            assert!(next.abs() <= TREND_LIMIT);
            biggest_jump = biggest_jump.max((next - trend).abs());
            visits.push(next);
            trend = next;
        }
        // Тренд ходит по обеим сторонам: и дефицит, и избыток бывают.
        assert!(visits.iter().any(|value| *value > 8));
        assert!(visits.iter().any(|value| *value < -8));
        // И при этом не скачет как новый бросок: шаг ограничен.
        assert!(biggest_jump < TREND_LIMIT * 2);
    }

    #[test]
    fn approach_moves_towards_the_target_only() {
        assert_eq!(approach(10, 50, 1), 11);
        assert_eq!(approach(90, 50, 1), 89);
        assert_eq!(approach(49, 50, 5), 50);
        assert_eq!(approach(50, 50, 5), 50);
    }
}
