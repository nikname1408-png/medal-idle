//! Таблица строк: `TABLES[0]` — русский, `[1]` — английский, `[2]` —
//! испанский. Правка текста — это правка строк в `strings/{ru,en,es}.rs`.
//!
//! Ключи данных — это `id` каталога: `cat.<id>.name`, `loc.<id>.name`,
//! `trader.<id>`, `ach.<id>.name`. Подстановки в шаблонах пишутся как `{}`.
//!
//! Словарь языка состоит из четырёх частей: строки интерфейса (`ENTRIES`),
//! названия предметов (`ITEM_NAMES`, ключ — просто `id`), описания предметов
//! (`ITEM_DESCS`, ключ — тоже `id`) и мир (`WORLD`).

/// Словарь одного языка: пары «ключ → строка».
pub struct Table {
    pub entries: &'static [(&'static str, &'static str)],
    pub item_names: &'static [(&'static str, &'static str)],
    pub item_descs: &'static [(&'static str, &'static str)],
    pub world: &'static [(&'static str, &'static str)],
}

/// Три словаря: русский, английский, испанский.
pub static TABLES: [Table; 3] = [
    Table {
        entries: ru::ENTRIES,
        item_names: ru::ITEM_NAMES,
        item_descs: ru::ITEM_DESCS,
        world: ru::WORLD,
    },
    Table {
        entries: en::ENTRIES,
        item_names: en::ITEM_NAMES,
        item_descs: en::ITEM_DESCS,
        world: en::WORLD,
    },
    Table {
        entries: es::ENTRIES,
        item_names: es::ITEM_NAMES,
        item_descs: es::ITEM_DESCS,
        world: es::WORLD,
    },
];

#[path = "strings/en.rs"]
mod en;
#[path = "strings/es.rs"]
mod es;
#[path = "strings/ru.rs"]
mod ru;
