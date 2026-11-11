//! Языки интерфейса и словарь строк.
//!
//! Модуль не зависит от Dioxus: текст выбирается из трёх словарей
//! (`TABLES[0]` — русский, `[1]` — английский, `[2]` — испанский) по ключу.
//!
//! Ключи данных — это `id` из каталога: `cat.<id>.name`, `loc.<id>.name`,
//! `trader.<id>`, `ach.<id>.name`. Перевод по ключу с подстановками —
//! [`tf`], без них — [`t`].
//!
//! Язык хранится в TLS-ячейке потока. Игра — одна задача на одном потоке, а
//! тесты так не мешают друг другу: каждый поток начинает с русского.

pub mod catalog;

mod tables;

use std::cell::Cell;

/// Язык интерфейса.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Ru,
    En,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::Ru, Lang::En, Lang::Es];

    fn code(self) -> u8 {
        match self {
            Lang::Ru => 0,
            Lang::En => 1,
            Lang::Es => 2,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Lang::En,
            2 => Lang::Es,
            _ => Lang::Ru,
        }
    }

    /// Двухбуквенный код для хранилища.
    pub fn code_str(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    /// Код из хранилища: всё неизвестное считается русским.
    pub fn parse(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "en" => Lang::En,
            "es" => Lang::Es,
            _ => Lang::Ru,
        }
    }

    /// Название языка на нём самом.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Ru => "Русский",
            Lang::En => "English",
            Lang::Es => "Español",
        }
    }
}

thread_local! {
    /// Язык текущего потока (номер словаря, см. `Lang::code`).
    static LANG: Cell<u8> = const { Cell::new(0) };
}

/// Текущий язык интерфейса.
pub fn lang() -> Lang {
    Lang::from_code(LANG.with(Cell::get))
}

/// Номер словаря текущего языка — для кэша каталога.
pub(crate) fn language_code() -> u8 {
    LANG.with(Cell::get)
}

/// Переключает язык интерфейса.
pub fn set_lang(value: Lang) {
    LANG.with(|cell| cell.set(value.code()));
}

/// Русский ли сейчас интерфейс.
pub fn is_ru() -> bool {
    lang() == Lang::Ru
}

/// Перевод строки по ключу. Неизвестный ключ показывается как `?` — так
/// пропуск сразу заметен в игре.
pub fn t(key: &str) -> &'static str {
    t_or(key, "?")
}

/// Перевод строки по ключу с запасным вариантом.
pub fn t_or(key: &str, fallback: &'static str) -> &'static str {
    // Ключа нет в выбранном языке — отдаём русский вариант.
    find(&tables::TABLES[lang().code() as usize], key)
        .or_else(|| find(&tables::TABLES[0], key))
        .unwrap_or(fallback)
}

/// Ищет ключ во всех частях словаря.
fn find(table: &'static tables::Table, key: &str) -> Option<&'static str> {
    let parts = [
        table.entries,
        table.item_names,
        table.item_descs,
        table.world,
    ];
    parts
        .into_iter()
        .find_map(|part| part.iter().find(|(entry_key, _)| *entry_key == key))
        .map(|(_, value)| *value)
}

/// Перевод шаблона: `{}` по порядку заменяется аргументами.
///
/// Если подстановок меньше, чем аргументов, лишние отбрасываются.
pub fn tf(key: &str, args: &[&str]) -> String {
    let mut text = t(key).to_string();
    for arg in args {
        match text.find("{}") {
            Some(position) => text.replace_range(position..position + 2, arg),
            None => break,
        }
    }
    text
}

/// Знак минуса: в русском типографский `−`, в остальных — обычный дефис.
pub fn minus() -> &'static str {
    if is_ru() {
        "−"
    } else {
        "-"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все части словаря в одном списке: ключи, названия, описания, мир.
    fn parts(table: &tables::Table) -> [&'static [(&'static str, &'static str)]; 4] {
        [
            table.entries,
            table.item_names,
            table.item_descs,
            table.world,
        ]
    }

    #[test]
    fn every_key_exists_in_all_languages() {
        for (index, table) in tables::TABLES.iter().enumerate().skip(1) {
            for key in parts(&tables::TABLES[0]).into_iter().flatten().map(|p| p.0) {
                assert!(
                    parts(table)
                        .into_iter()
                        .any(|part| part.iter().any(|(other, _)| *other == key)),
                    "в языке {index} нет ключа {key}"
                );
            }
        }
    }

    #[test]
    fn keys_are_unique() {
        for table in tables::TABLES.iter() {
            for (name, part) in [
                ("entries", table.entries),
                ("names", table.item_names),
                ("descs", table.item_descs),
                ("world", table.world),
            ] {
                for (i, (key, _)) in part.iter().enumerate() {
                    for (other, _) in part.iter().skip(i + 1) {
                        assert_ne!(key, other, "ключ {key} повторяется в {name}");
                    }
                }
            }
        }
    }

    #[test]
    fn placeholders_match_between_languages() {
        for (key, value) in parts(&tables::TABLES[0]).into_iter().flatten() {
            let expected = value.matches("{}").count();
            for table in tables::TABLES.iter().skip(1) {
                let translated = parts(table)
                    .into_iter()
                    .flatten()
                    .find(|(other, _)| other == key)
                    .map(|(_, v)| *v)
                    .unwrap();
                assert_eq!(
                    translated.matches("{}").count(),
                    expected,
                    "в ключе {key} другое число подстановок"
                );
            }
        }
    }

    #[test]
    fn language_switching_changes_text() {
        set_lang(Lang::Ru);
        assert_eq!(t("tab.forge"), "Кузница");
        set_lang(Lang::En);
        assert_eq!(t("tab.forge"), "Forge");
        set_lang(Lang::Es);
        assert_eq!(t("tab.forge"), "Forja");
        set_lang(Lang::Ru);
        assert_eq!(catalog::name("iron"), "Железная руда");
        set_lang(Lang::En);
        assert_eq!(catalog::name("iron"), "Iron ore");
        set_lang(Lang::Ru);
    }

    #[test]
    fn templates_fill_in_order() {
        set_lang(Lang::Ru);
        assert_eq!(tf("time.seconds", &["12"]), "12 с");
        set_lang(Lang::En);
        assert_eq!(tf("time.seconds", &["12"]), "12 s");
        set_lang(Lang::Ru);
    }

    #[test]
    fn parse_falls_back_to_russian() {
        assert_eq!(Lang::parse("es"), Lang::Es);
        assert_eq!(Lang::parse("EN"), Lang::En);
        assert_eq!(Lang::parse("de"), Lang::Ru);
    }
}
