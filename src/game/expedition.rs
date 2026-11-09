//! Походы по локациям.
//!
//! Один поход — это «прогулка» на [`balance::EXPEDITION_TICKS`] тиков пути.
//! На каждом тике с небольшим шансом находится крупная партия ресурсов или
//! предмет. Активные ресурсы уходят на склад (излишки сгорают), инвентарные —
//! в рюкзак; если рюкзак полон, поход прерывается. Уголь в походах не падает.
//!
//! Модуль не зависит от Dioxus: логику можно проверять через `cargo test`.

use super::actions::next_random;
use super::balance;
use super::state::GameState;

/// Ошибка похода.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpeditionError {
    UnknownLocation,
    NoEnergy,
}

impl ExpeditionError {
    pub fn text(self) -> &'static str {
        match self {
            Self::UnknownLocation => "Такой локации нет.",
            Self::NoEnergy => "Не хватает энергии на поход.",
        }
    }
}

/// Описание локации похода.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpeditionLocation {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Шанс находки за один тик пути, %.
    pub chance_percent: u32,
    /// Дропи на склад: (вид, минимум, максимум).
    pub active_drops: &'static [(&'static str, u32, u32)],
    /// Дропи в рюкзак: (вид, минимум, максимум).
    pub inventory_drops: &'static [(&'static str, u32, u32)],
}

/// Все локации походов.
pub const LOCATIONS: [ExpeditionLocation; 3] = [
    ExpeditionLocation {
        id: "forest",
        name: "Лес",
        description: "Ближний лес: древесина и волокно. Ходить близко и безопасно.",
        chance_percent: balance::EXPEDITION_CHANCE_FOREST,
        active_drops: &balance::EXPEDITION_FOREST_ACTIVE,
        inventory_drops: &balance::EXPEDITION_FOREST_INVENTORY,
    },
    ExpeditionLocation {
        id: "quarry",
        name: "Каменоломня",
        description: "Старая каменоломня: камень и глина, иногда кварц.",
        chance_percent: balance::EXPEDITION_CHANCE_QUARRY,
        active_drops: &balance::EXPEDITION_QUARRY_ACTIVE,
        inventory_drops: &balance::EXPEDITION_QUARRY_INVENTORY,
    },
    ExpeditionLocation {
        id: "ruins",
        name: "Руины",
        description: "Дальние руины: руда, кожа и серебро. Дорога опаснее, зато добыча богаче.",
        chance_percent: balance::EXPEDITION_CHANCE_RUINS,
        active_drops: &balance::EXPEDITION_RUINS_ACTIVE,
        inventory_drops: &balance::EXPEDITION_RUINS_INVENTORY,
    },
];

/// Находит локацию по идентификатору.
pub fn location(id: &str) -> Option<&'static ExpeditionLocation> {
    LOCATIONS.iter().find(|location| location.id == id)
}

/// Одна находка за поход.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loot {
    pub item_id: String,
    pub quantity: u32,
    /// `true` — ресурс ушёл на склад, `false` — в рюкзак.
    pub active: bool,
}

/// Итог похода.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpeditionReport {
    /// Найденное и реально добавленное.
    pub loot: Vec<Loot>,
    /// Рюкзак заполнился и поход прервался.
    pub interrupted: bool,
    /// Часть добычи не поместилась на склад и сгорела.
    pub overflow: bool,
}

/// Отправляет игрока в поход. Списывает энергию и возвращает итог.
pub fn run_expedition(
    state: &mut GameState,
    location_id: &str,
) -> Result<ExpeditionReport, ExpeditionError> {
    let location = location(location_id).ok_or(ExpeditionError::UnknownLocation)?;
    if !state.spend_energy(balance::EXPEDITION_ENERGY_COST) {
        return Err(ExpeditionError::NoEnergy);
    }

    let mut seed = state
        .market_seed
        .wrapping_add(state.stats.clicks.wrapping_mul(31))
        .wrapping_add((state.stats.play_seconds as u64).wrapping_mul(17));
    let bonus = 1.0 + state.expedition_bonus as f64;

    let mut report = ExpeditionReport::default();
    for _ in 0..balance::EXPEDITION_TICKS {
        if next_random(&mut seed) % 100 >= location.chance_percent as u64 {
            continue;
        }

        let total_drops = location.active_drops.len() + location.inventory_drops.len();
        if total_drops == 0 {
            continue;
        }
        let pick = (next_random(&mut seed) % total_drops as u64) as usize;

        if pick < location.active_drops.len() {
            let (kind, min_quantity, max_quantity) = location.active_drops[pick];
            let span = (max_quantity - min_quantity + 1) as u64;
            let base = min_quantity as f64 + (next_random(&mut seed) % span) as f64;
            let quantity = (base * bonus).round() as u32;
            // Излишки сверх лимита склада сгорают.
            let added = state.add_active(kind, quantity);
            if added < quantity {
                report.overflow = true;
            }
            if added > 0 {
                push_loot(&mut report.loot, kind, added, true);
            }
        } else {
            let (kind, min_quantity, max_quantity) =
                location.inventory_drops[pick - location.active_drops.len()];
            let span = (max_quantity - min_quantity + 1) as u64;
            let quantity = min_quantity + (next_random(&mut seed) % span) as u32;
            if !state.add_item(kind, quantity) {
                // Рюкзак полон: поход прерывается, уже найденное остаётся.
                state.market_seed = seed;
                report.interrupted = true;
                return Ok(report);
            }
            push_loot(&mut report.loot, kind, quantity, false);
        }
    }
    state.market_seed = seed;
    Ok(report)
}

/// Сливает одинаковые находки в одну строку.
fn push_loot(loot: &mut Vec<Loot>, item_id: &str, quantity: u32, active: bool) {
    if let Some(entry) = loot
        .iter_mut()
        .find(|entry| entry.item_id == item_id && entry.active == active)
    {
        entry.quantity += quantity;
    } else {
        loot.push(Loot {
            item_id: item_id.to_string(),
            quantity,
            active,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_location_is_rejected() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        assert_eq!(
            run_expedition(&mut state, "nowhere"),
            Err(ExpeditionError::UnknownLocation)
        );
        assert_eq!(state.energy, 100.0);
    }

    #[test]
    fn expedition_requires_energy() {
        let mut state = GameState {
            energy: 0.0,
            ..GameState::default()
        };

        assert_eq!(
            run_expedition(&mut state, "forest"),
            Err(ExpeditionError::NoEnergy)
        );
    }

    #[test]
    fn expedition_spends_energy() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        run_expedition(&mut state, "forest").expect("поход");
        assert!((state.energy - (100.0 - balance::EXPEDITION_ENERGY_COST)).abs() < 1e-9);
    }

    #[test]
    fn expedition_drops_only_location_resources() {
        let mut state = GameState {
            energy: 1_000.0,
            ..GameState::default()
        };
        let mut found = 0;

        for seed in 0..500u64 {
            state.energy = 1_000.0;
            state.market_seed = seed;
            let report = run_expedition(&mut state, "forest").expect("поход");
            for loot in &report.loot {
                found += 1;
                assert_ne!(loot.item_id, "coal");
                assert!(
                    loot.item_id == "wood" || loot.item_id == "fiber",
                    "неожиданный дроп: {}",
                    loot.item_id
                );
            }
        }

        assert!(found > 0, "за 500 походов не нашлось ни одной добычи");
    }

    #[test]
    fn expedition_loot_is_added_to_state() {
        let mut state = GameState {
            energy: 1_000.0,
            ..GameState::default()
        };
        let mut found = false;

        for seed in 0..200u64 {
            state.energy = 1_000.0;
            state.market_seed = seed;
            let report = run_expedition(&mut state, "quarry").expect("поход");
            for loot in report.loot {
                found = true;
                if loot.active {
                    assert!(state.active_amount(&loot.item_id) >= loot.quantity);
                } else {
                    assert!(state.item_count(&loot.item_id) >= loot.quantity);
                }
            }
        }

        assert!(found);
    }

    #[test]
    fn full_backpack_interrupts_expedition() {
        let mut state = GameState {
            energy: 1_000.0,
            ..GameState::default()
        };
        // Забиваем рюкзак нестакуемыми предметами под завязку.
        let slots = state.inventory_slots();
        for index in 0..slots {
            let id = format!("filler_{index}");
            state.inventory.insert(id, 1);
        }

        // Ищем зерно, при котором лес находит волокно (инвентарный дроп).
        for seed in 0..2_000u64 {
            state.energy = 1_000.0;
            state.market_seed = seed;
            let report = run_expedition(&mut state, "forest").expect("поход");
            if report.interrupted {
                assert!(state.inventory_free_slots() == 0 || !report.loot.is_empty());
                return;
            }
            // Откатываем изменения склада между попытками.
            state.active_resources.clear();
        }

        panic!("не удалось найти прерывание похода при полном рюкзаке");
    }
}
