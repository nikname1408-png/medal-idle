//! Языки интерфейса и словарь строк.
//!
//! Модуль не зависит от Dioxus: текст выбирается из трёх словарей
//! (`TABLES[0]` — русский, `[1]` — английский, `[2]` — испанский) по ключу.
//!
//! Ключи данных — это `id` из каталога: `cat.<id>.name`, `loc.<id>.name`,
//! `trader.<id>`, `ach.<id>.name`. Перевод по ключу с подстановками —
//! [`tf`], без них — [`t`].
//!
//! Язык хранится в общей ячейке (`AtomicU8`): его читают и отрисовка, и
//! логика, а WebView-рантайм может обслуживать отрисовку на другом потоке,
//! поэтому потоковая ячейка здесь не подходит.

pub mod catalog;

mod tables;

use std::sync::atomic::{AtomicU8, Ordering};

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

/// Язык интерфейса — один на всю программу. Читают его и рендер, и логика,
/// поэтому ячейка общая (`AtomicU8`), а не потоковая: WebView-рантайм Dioxus
/// может обслуживать отрисовку на другом потоке. Тесты, которые переключают
/// язык, берут [`test_lock`], чтобы не мешать параллельным.
static LANG: AtomicU8 = AtomicU8::new(0);

/// Текущий язык интерфейса.
pub fn lang() -> Lang {
    Lang::from_code(LANG.load(Ordering::Relaxed))
}

/// Номер словаря текущего языка — для кэша каталога.
pub(crate) fn language_code() -> u8 {
    LANG.load(Ordering::Relaxed)
}

/// Переключает язык интерфейса.
pub fn set_lang(value: Lang) {
    LANG.store(value.code(), Ordering::Relaxed);
}

/// Общий замок для тестов, которые меняют язык: без него параллельные тесты
/// экономики видят английские строки.
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
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

/// Перевод шаблона с именованными подстановками `{name}`.
///
/// Нужен там, где порядок частей в переводе отличается от порядка данных
/// (тексты объявлений: автор может стоять в середине фразы).
pub fn tf_named(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
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
    fn named_placeholders_match_between_languages() {
        // Тексты объявлений собираются через `tf_named`: подстановки в них
        // именованные ({author}, {request}, {reward}). Если перевод потерял
        // или переименовал подстановку, в интерфейсе останется «{author}».
        fn names(value: &str) -> std::collections::BTreeSet<String> {
            let mut found = std::collections::BTreeSet::new();
            let mut rest = value;
            while let Some(start) = rest.find('{') {
                let tail = &rest[start + 1..];
                let Some(end) = tail.find('}') else { break };
                let name = &tail[..end];
                if !name.is_empty() {
                    found.insert(name.to_string());
                }
                rest = &tail[end + 1..];
            }
            found
        }

        for (key, value) in parts(&tables::TABLES[0]).into_iter().flatten() {
            let expected = names(value);
            if expected.is_empty() {
                continue;
            }
            for table in tables::TABLES.iter().skip(1) {
                let translated = parts(table)
                    .into_iter()
                    .flatten()
                    .find(|(other, _)| other == key)
                    .map(|(_, v)| *v)
                    .unwrap();
                assert_eq!(
                    names(translated),
                    expected,
                    "в ключе {key} другой набор именованных подстановок"
                );
            }
        }
    }

    #[test]
    fn named_templates_fill_by_name() {
        set_lang(Lang::Ru);
        let text = tf_named("quest.author_line", &[("author", "Марта")]);
        assert!(text.contains("Марта"), "автор не подставился: {text}");
        assert!(!text.contains('{'), "в тексте осталась подстановка: {text}");
        set_lang(Lang::Ru);
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
