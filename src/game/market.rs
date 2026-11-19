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

/// Шаг настроения за один тик рынка.
///
/// Точно такой же, как у торговцев (`market_step` сдвигает тренд на
/// `noise(±4)`): город и лавки живут в одном ритме, а не скачут на ±20.
pub const TREND_STEP: i32 = 4;

/// Сколько штук товара город готов принять за один заказ при равновесии.
///
/// Масштаб берётся у торговца: та же наценка за тир товара
/// (`Tier::values().1`), только умноженная на `DEMAND_CITY_SCALE` — город
/// закупает партиями в `DEMAND_CITY_SCALE` раз больше одной лавки.
///
/// Раньше здесь была своя формула «бюджет / цена» с двойным счётом, и у
/// свечи выходило 3×10¹⁰ штук. Теперь никакой отдельной экономики у города
/// нет: один тир, один масштаб, те же числа, что на витрине торговца.
pub fn capacity_per_trend(item_id: &str) -> f64 {
    let per_order = trader::tier_of(item_id).values().1 as f64;
    per_order * super::balance::DEMAND_CITY_SCALE
}

/// Один шаг настроения: маленький случайный дрейф, как у торговцев.
///
/// Раньше город бросал тренд на ±20 за шаг — рынок дёргался, но «умирал»
/// от любой продажи. Теперь шаг тот же, что в `market_step`: ±`TREND_STEP`
/// за тик, с потолком `TREND_LIMIT`.
pub fn drift_trend(trend: i32, seed: &mut u64) -> i32 {
    let drift = trader::random_below(seed, (TREND_STEP * 2 + 1) as u64) as i32 - TREND_STEP;
    (trend + drift).clamp(-TREND_LIMIT, TREND_LIMIT)
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
