//! Походы по локациям.
//!
//! Поход длится, пока есть энергия: каждую секунду пути тратится
//! [`balance::EXPEDITION_ENERGY_PER_TICK`]. На каждом шаге с небольшим шансом
//! находится партия ресурсов или предмет. Активные ресурсы уходят на склад
//! (излишки сгорают), инвентарные — в рюкзак; если рюкзак полон, поход
//! прерывается. Уголь в походах не падает.
//!
//! Модуль не зависит от Dioxus: логику можно проверять через `cargo test`.

use super::actions::next_random;
use super::balance;
use super::items::EquipSlot;
use super::state::GameState;

/// Ошибка похода.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
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
    /// Шанс находки за один игровой тик (1 с) пути, %.
    pub chance_percent: u32,
    /// Дропи на склад: (вид, минимум, максимум).
    pub active_drops: &'static [(&'static str, u32, u32)],
    /// Дропи в рюкзак: (вид, минимум, максимум).
    pub inventory_drops: &'static [(&'static str, u32, u32)],
}

/// Шанс находки за один тик пути.
fn tick_chance(location: &ExpeditionLocation) -> u64 {
    let scale = balance::EXPEDITION_CHANCE_SCALE.max(1) as u64;
    ((location.chance_percent as u64) / scale).max(1)
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
        description: "Старая каменоломня: камень на склад и глину в рюкзак.",
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
    /// Энергия закончилась и поход прервался.
    pub out_of_energy: bool,
    /// Часть добычи не поместилась на склад и сгорела.
    pub overflow: bool,
}

/// Итог одного тика похода.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpeditionTick {
    /// Найденное за этот тик.
    pub loot: Vec<Loot>,
    /// Найдено за время шага, сек (для счётчика времени в пути).
    pub seconds: f64,
    /// Рюкзак заполнился — поход прерван.
    pub interrupted: bool,
    /// Энергия закончилась — поход прерван.
    pub out_of_energy: bool,
    /// Часть добычи не поместилась на склад и сгорела.
    pub overflow: bool,
}

impl ExpeditionTick {
    /// Поход пора заканчивать.
    pub fn is_over(&self) -> bool {
        self.interrupted || self.out_of_energy
    }
}

/// Один шаг похода длительностью `dt` секунд.
///
/// Энергия тратится пропорционально времени; когда она кончается, поход
/// останавливается сам.
pub fn step_expedition(state: &mut GameState, location_id: &str, dt: f64) -> ExpeditionTick {
    let mut tick = ExpeditionTick::default();
    let Some(location) = location(location_id) else {
        tick.out_of_energy = true;
        return tick;
    };

    let seconds = dt.max(0.0) + state.expedition_timer;
    let rolls = seconds.floor() as u32;
    state.expedition_timer = seconds - rolls as f64;

    // Зимние перчатки и куртка снижают расход энергии в пути.
    let energy_discount = state.expedition_energy_discount.clamp(0.0, 0.9) as f64;
    let energy_cost = balance::EXPEDITION_ENERGY_PER_TICK * (1.0 - energy_discount);
    if !state.spend_energy(energy_cost * dt.max(0.0)) {
        tick.out_of_energy = true;
        return tick;
    }
    tick.seconds = dt.max(0.0);
    if rolls == 0 {
        return tick;
    }

    let bonus = 1.0 + state.expedition_bonus as f64;
    let chance = tick_chance(location);
    let total_drops = location.active_drops.len() + location.inventory_drops.len();
    for _ in 0..rolls {
        // Каждый бросок продвигает зерно, поэтому тики не повторяют друг друга.
        state.market_seed = state.market_seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut seed = state.market_seed;
        if next_random(&mut seed) % 100 >= chance || total_drops == 0 {
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
                tick.overflow = true;
            }
            if added > 0 {
                push_loot(&mut tick.loot, kind, added, true);
            }
        } else {
            let (kind, min_quantity, max_quantity) =
                location.inventory_drops[pick - location.active_drops.len()];
            let span = (max_quantity - min_quantity + 1) as u64;
            let quantity = min_quantity + (next_random(&mut seed) % span) as u32;
            if !state.add_item(kind, quantity) {
                // Рюкзак полон: поход прерывается, уже найденное остаётся.
                tick.interrupted = true;
                return tick;
            }
            push_loot(&mut tick.loot, kind, quantity, false);
        }
    }

    // Золотая подвеска повышает шанс найти драгоценные камни.
    let gems_bonus = EquipSlot::ALL
        .iter()
        .filter_map(|slot| state.equipped_item(*slot))
        .filter_map(|item| item.stats.gems_bonus)
        .fold(0.0f32, |sum, bonus| sum + bonus)
        / 100.0;
    let rare_chance = 1 + (gems_bonus.max(0.0) as f64 * 5.0).round() as u32;
    for _ in 0..rolls {
        state.market_seed = state.market_seed.wrapping_add(0x2545_f491_4f6c_dd1d);
        let mut seed = state.market_seed;
        if next_random(&mut seed) % 100 < rare_chance as u64 && state.add_item("gems", 1) {
            push_loot(&mut tick.loot, "gems", 1, false);
        }
    }
    tick
}

/// Отправляет игрока в поход на `seconds` секунд одним разом.
///
/// Используется в тестах и для быстрой проверки; интерфейс тикает по секундам.
#[cfg_attr(not(test), allow(dead_code))]
pub fn run_expedition(
    state: &mut GameState,
    location_id: &str,
    seconds: u32,
) -> Result<ExpeditionReport, ExpeditionError> {
    if location(location_id).is_none() {
        return Err(ExpeditionError::UnknownLocation);
    }
    if state.energy + 1e-9 < balance::EXPEDITION_ENERGY_PER_TICK {
        return Err(ExpeditionError::NoEnergy);
    }

    let mut report = ExpeditionReport::default();
    for _ in 0..seconds {
        let tick = step_expedition(state, location_id, 1.0);
        for loot in tick.loot {
            push_loot(&mut report.loot, &loot.item_id, loot.quantity, loot.active);
        }
        report.overflow |= tick.overflow;
        if tick.interrupted {
            report.interrupted = true;
            break;
        }
        if tick.out_of_energy {
            report.out_of_energy = true;
            break;
        }
    }
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

    /// Энергии хватит на много часов пути.
    fn endless_state() -> GameState {
        GameState {
            energy: 1_000_000.0,
            ..GameState::default()
        }
    }

    #[test]
    fn unknown_location_is_rejected() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        assert_eq!(
            run_expedition(&mut state, "nowhere", 10),
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
            run_expedition(&mut state, "forest", 10),
            Err(ExpeditionError::NoEnergy)
        );
    }

    #[test]
    fn expedition_spends_energy_per_tick() {
        let mut state = GameState {
            energy: 100.0,
            ..GameState::default()
        };

        run_expedition(&mut state, "forest", 10).expect("поход");
        let spent = 100.0 - state.energy;
        assert!((spent - balance::EXPEDITION_ENERGY_PER_TICK * 10.0).abs() < 1e-9);
    }

    #[test]
    fn expedition_stops_when_energy_runs_out() {
        let mut state = GameState {
            energy: balance::EXPEDITION_ENERGY_PER_TICK * 3.0,
            ..GameState::default()
        };

        let report = run_expedition(&mut state, "forest", 100).expect("поход");
        assert!(report.out_of_energy);
        assert!(state.energy.abs() < 1e-9);
    }

    #[test]
    fn expedition_drops_only_location_resources() {
        let mut state = endless_state();
        let mut found = 0;

        for seed in 0..2_000u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "forest", 60).expect("поход");
            for loot in &report.loot {
                found += 1;
                assert_ne!(loot.item_id, "coal");
                assert!(
                    matches!(loot.item_id.as_str(), "wood" | "fiber" | "gems"),
                    "неожиданный дроп: {}",
                    loot.item_id
                );
            }
            // Склад сбрасываем, чтобы лимиты не мешали повторять.
            state.active_resources.clear();
            state.inventory.clear();
        }

        assert!(found > 0, "за 2000 походов не нашлось ни одной добычи");
    }

    #[test]
    fn expedition_loot_is_added_to_state() {
        let mut state = endless_state();
        let mut found = false;

        for seed in 0..500u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "quarry", 60).expect("поход");
            for loot in report.loot {
                found = true;
                if loot.active {
                    assert!(state.active_amount(&loot.item_id) >= loot.quantity);
                } else {
                    // Глина — инвентарный ресурс, а не активный.
                    assert!(loot.item_id != "clay" || state.item_count("clay") >= loot.quantity);
                    assert!(state.item_count(&loot.item_id) >= loot.quantity);
                }
            }
            state.active_resources.clear();
            state.inventory.clear();
        }

        assert!(found);
    }

    #[test]
    fn full_backpack_interrupts_expedition() {
        let mut state = endless_state();
        // Забиваем рюкзак нестакуемыми предметами под завязку.
        let slots = state.inventory_slots();
        for index in 0..slots {
            let id = format!("filler_{index}");
            state.inventory.insert(id, 1);
        }

        // Ищем зерно, при котором лес находит волокно (инвентарный дроп).
        for seed in 0..5_000u64 {
            state.market_seed = seed;
            let report = run_expedition(&mut state, "forest", 60).expect("поход");
            if report.interrupted {
                return;
            }
            // Откатываем добычу склада между попытками.
            state.active_resources.clear();
        }

        panic!("не удалось найти прерывание похода при полном рюкзаке");
    }
}
