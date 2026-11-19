mod app;
mod city;
mod expedition;
mod inventory;
mod save;
mod smith;
mod trader;

// Тексты интерфейса живут в исходниках Rust, и правки должны сохранять
// их в UTF-8: проверка `sources_have_no_broken_cyrillic` ловит «поплывшие»
// символы раньше, чем они доедут до APK.
#[cfg(test)]
mod text_encoding_tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Файлы с кириллицей, которые попадают в APK или в документацию.
    const SOURCE_DIRS: [&str; 2] = ["src", "assets"];
    const EXTENSIONS: [&str; 4] = ["rs", "css", "md", "toml"];

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn collect(path: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
                continue;
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if EXTENSIONS.contains(&extension) {
                out.push(path);
            }
        }
    }

    /// Пары символов, которые получаются, когда UTF-8 текст перезаписали
    /// в cp1251: знак умножения, длинное тире и эмодзи распадаются на
    /// «кириллицу + мусорный символ». Обычной русской речи такие пары
    /// не встречаются, поэтому строки ниже записаны escape-кодами.
    const MARKERS: [&str; 15] = [
        "\u{0413}\u{2014}",         // распавшийся знак умножения
        "\u{0413}\u{0405}",         // его же продолжение
        "\u{0432}\u{0402}\u{201d}", // распавшееся длинное тире
        "\u{0432}\u{0402}\u{0459}", // распавшийся знак евро
        "\u{0432}\u{0409}",         // обрывок трёхбайтовых символов
        "\u{0432}\u{0459}",
        "\u{0432}\u{2039}",         // кавычки-ёлочки
        "\u{0432}\u{203a}",
        "\u{0432}\u{0458}",         // многоточие
        "\u{0432}\u{2019}",
        "\u{0432}\u{00b7}",         // интерпункт
        "\u{0440}\u{045f}",         // эмодзи
        "\u{0421}\u{0402}",         // заглавная половина
        "\u{0420}\u{0451}",
        "\u{0420}\u{203a}",
    ];

    /// Текст с «поплывшими» символами: символ замены или битая пара.
    fn looks_broken(line: &str) -> bool {
        line.contains('\u{FFFD}') || MARKERS.iter().any(|marker| line.contains(marker))
    }

    /// Собирает все подозрительные строки в папках проекта.
    fn broken_lines() -> Vec<String> {
        let root = repo_root();
        let mut files = Vec::new();
        for dir in SOURCE_DIRS {
            let path = root.join(dir);
            if path.is_dir() {
                collect(&path, &mut files);
            }
        }

        let mut broken = Vec::new();
        for path in files {
            let Ok(text) = fs::read_to_string(&path) else {
                broken.push(format!("{}: не читается как UTF-8", path.display()));
                continue;
            };
            for (number, line) in text.lines().enumerate() {
                if looks_broken(line) {
                    broken.push(format!("{}:{}: {}", path.display(), number + 1, line.trim()));
                }
            }
        }
        broken
    }

    #[test]
    fn sources_have_no_broken_cyrillic() {
        let broken = broken_lines();
        assert!(
            broken.is_empty(),
            "в исходниках битая кодировка:\n{}",
            broken.join("\n")
        );
    }

    /// Сама проверка тоже не должна молча промахиваться.
    #[test]
    fn broken_marker_is_detected() {
        let sample = format!("цена {}3", "\u{0413}\u{2014}");
        assert!(looks_broken(&sample));
        assert!(looks_broken("\u{0440}\u{045f}\u{040f}\u{0459}"));
        assert!(!looks_broken("Обычная строка: 3 × 5 — 2."));
        assert!(!looks_broken("Ресурс уже упёрся в лимит склада."));
    }
}

pub use app::App;
