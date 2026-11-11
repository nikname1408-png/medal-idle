//! Перевод текстов каталога предметов: названия, описания и бонусы.
//!
//! Названия и описания лежат в словаре по ключу, равному `id` предмета из
//! `items.rs` (`ITEM_NAMES` / `ITEM_DESCS`). Чтобы не искать по массиву на
//! каждом обращении, словарь текущего языка один раз раскладывается в карту
//! «(id, поле) → перевод»; при смене языка карта строится заново.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::tables;

/// Поле каталога: название или описание.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Name,
    Desc,
}

/// Карта «(id, поле) → перевод» и номер словаря, из которого она собрана.
struct CatalogMap {
    language: u8,
    entries: HashMap<(&'static str, Field), &'static str>,
}

fn catalog() -> &'static Mutex<Option<CatalogMap>> {
    static CATALOG: OnceLock<Mutex<Option<CatalogMap>>> = OnceLock::new();
    CATALOG.get_or_init(|| Mutex::new(None))
}

/// Строка каталога по id. Если ключа нет ни в одном языке — пустая строка.
pub fn field(item_id: &str, field: Field) -> &'static str {
    lookup(item_id, field).unwrap_or("")
}

/// Название предмета по id.
pub fn name(item_id: &str) -> &'static str {
    match lookup(item_id, Field::Name) {
        Some(value) => value,
        // Неизвестный id показывается как есть — пропуск перевода сразу
        // заметен в игре.
        None => leak(item_id),
    }
}

/// Оставляет id в памяти навсегда: вызывается только для непереведённых id,
/// которых в норме нет.
fn leak(item_id: &str) -> &'static str {
    Box::leak(item_id.to_string().into_boxed_str())
}

/// Описание предмета по id.
pub fn description(item_id: &str) -> &'static str {
    field(item_id, Field::Desc)
}

fn lookup(item_id: &str, field: Field) -> Option<&'static str> {
    let language = super::language_code();
    let mut guard = catalog().lock().ok()?;
    let stale = guard.as_ref().map(|map| map.language) != Some(language);
    if stale {
        *guard = Some(build(language));
    }
    guard
        .as_ref()
        .and_then(|map| map.entries.get(&(item_id, field)).copied())
}

/// Раскладывает названия и описания выбранного языка по id.
fn build(language: u8) -> CatalogMap {
    let table = &tables::TABLES[(language as usize).min(tables::TABLES.len() - 1)];
    let mut entries = HashMap::with_capacity(table.item_names.len() + table.item_descs.len());
    for (id, value) in table.item_names {
        entries.insert((*id, Field::Name), *value);
    }
    for (id, value) in table.item_descs {
        entries.insert((*id, Field::Desc), *value);
    }
    CatalogMap { language, entries }
}
