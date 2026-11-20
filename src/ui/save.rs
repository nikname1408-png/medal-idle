//! Сохранение прогресса в localStorage через WebView.
//!
//! Работает и на десктопе (WebView2), и на Android (WebView).

use dioxus::prelude::*;

use crate::game::i18n::Lang;
use crate::game::GameState;

/// Ключ, под которым лежит JSON сохранения.
const SAVE_KEY: &str = "pole_save";

/// Ключ языка интерфейса. Язык живёт отдельно: «Сбросить прогресс» его не
/// трогает.
const LANG_KEY: &str = "pole_lang";

/// Читает сохранённый язык. `Ok(None)` — язык ещё не выбирали.
pub async fn load_lang() -> Result<Option<Lang>, String> {
    let script = format!("return localStorage.getItem('{LANG_KEY}');");
    let value = document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    Ok(value.as_str().map(Lang::parse))
}

/// Запоминает выбранный язык.
pub async fn save_lang(lang: Lang) -> Result<(), String> {
    let script = format!(
        "localStorage.setItem('{LANG_KEY}', '{}'); return true;",
        lang.code_str()
    );
    document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// Загружает сохранение. `Ok(None)` — сохранения ещё нет.
pub async fn load() -> Result<Option<GameState>, String> {
    let script = format!("return localStorage.getItem('{SAVE_KEY}');");
    let value = document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    let Some(text) = value.as_str() else {
        return Ok(None);
    };

    serde_json::from_str(text)
        .map(Some)
        .map_err(|error| format!("сохранение повреждено: {error}"))
}

/// Префикс закодированного сохранения при переносе.
///
/// Он же отличает новый формат от старого: раньше в поле лежал обычный JSON,
/// и такие сохранения по-прежнему принимаются.
const EXPORT_PREFIX: &str = "POLE1:";

/// Ключ перемешивания. Это не шифрование, а защита от правки в блокноте:
/// без ключа текст читается как случайные буквы и цифры.
const EXPORT_KEY: &[u8] = b"pole-medieval-idle-2026";

/// Имя файла, который сохраняется на устройство.
pub const EXPORT_FILE_NAME: &str = "pole-save.txt";

fn xor_bytes(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(index, byte)| byte ^ EXPORT_KEY[index % EXPORT_KEY.len()])
        .collect()
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (first << 16) | (second << 8) | third;
        out.push(BASE64[(triple >> 18) as usize & 63] as char);
        out.push(BASE64[(triple >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(BASE64[(triple >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64[triple as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err("сохранение повреждено: неизвестный символ".to_string()),
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// Готовит сохранение в виде текста для переноса на другое устройство.
///
/// Внутри — тот же JSON, но перемешанный и переведённый в base64: значения
/// в нём не поправить руками, не сломав контроль формата.
pub fn export_text(state: &GameState) -> Result<String, String> {
    let json = serde_json::to_string(state)
        .map_err(|error| format!("не удалось сериализовать состояние: {error}"))?;
    let packed = base64_encode(&xor_bytes(json.as_bytes()));
    Ok(format!("{EXPORT_PREFIX}{packed}"))
}

/// Принимает сохранение из текста и прогоняет его через миграции.
///
/// Понимает и новый закодированный формат, и старый JSON: игрок мог
/// сохранить файл ещё до перехода на кодирование.
pub fn import_text(text: &str) -> Result<GameState, String> {
    let trimmed = text.trim();
    let json = if let Some(rest) = trimmed.strip_prefix(EXPORT_PREFIX) {
        let packed: String = rest.chars().filter(|ch| !ch.is_whitespace()).collect();
        let bytes = base64_decode(&packed)?;
        String::from_utf8(xor_bytes(&bytes))
            .map_err(|_| "сохранение повреждено: это не текст".to_string())?
    } else {
        trimmed.to_string()
    };
    let mut state: GameState =
        serde_json::from_str(&json).map_err(|error| format!("сохранение повреждено: {error}"))?;
    state.migrate();
    Ok(state)
}

/// Сохраняет текст сохранения файлом на устройство.
///
/// На Android WebView файл уходит в «Загрузки» через обычную ссылку с
/// `download`. Если браузер загрузку не поддерживает, текст остаётся в
/// поле на экране — его можно скопировать вручную.
pub async fn download_file(name: &str, text: &str) -> Result<(), String> {
    let name_literal = serde_json::to_string(name)
        .map_err(|error| format!("не удалось подготовить имя: {error}"))?;
    let text_literal = serde_json::to_string(text)
        .map_err(|error| format!("не удалось подготовить строку: {error}"))?;

    let script = format!(
        r#"(() => {{
            try {{
                const blob = new Blob([{text_literal}], {{ type: 'text/plain;charset=utf-8' }});
                const url = URL.createObjectURL(blob);
                const link = document.createElement('a');
                link.href = url;
                link.download = {name_literal};
                link.style.display = 'none';
                document.body.appendChild(link);
                link.click();
                document.body.removeChild(link);
                setTimeout(() => URL.revokeObjectURL(url), 4000);
                return true;
            }} catch (error) {{
                return 'error: ' + error;
            }}
        }})()"#
    );

    let value = document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    if value.as_bool() == Some(true) {
        Ok(())
    } else {
        Err(value
            .as_str()
            .unwrap_or("браузер не дал сохранить файл")
            .to_string())
    }
}

/// Записывает состояние в localStorage.
pub async fn save(state: &GameState) -> Result<(), String> {
    let text = serde_json::to_string(state)
        .map_err(|error| format!("не удалось сериализовать состояние: {error}"))?;
    let literal = serde_json::to_string(&text)
        .map_err(|error| format!("не удалось подготовить строку: {error}"))?;

    let script = format!("localStorage.setItem('{SAVE_KEY}', {literal}); return true;");
    document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// Удаляет сохранение.
pub async fn clear() -> Result<(), String> {
    let script = format!("localStorage.removeItem('{SAVE_KEY}'); return true;");
    document::eval(&script)
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_encoded_and_round_trips() {
        let mut state = GameState::default();
        state.gold = 12_345.0;
        state.cycles = 4;

        let text = export_text(&state).expect("экспорт");
        assert!(text.starts_with(EXPORT_PREFIX), "нет префикса формата");
        // В закодированном тексте не должно быть читаемых значений: иначе
        // сохранение правится в блокноте одной правкой.
        assert!(!text.contains("\"gold\""));
        assert!(!text.contains("12345"));
        assert!(!text.contains('{'));

        let loaded = import_text(&text).expect("импорт");
        assert!((loaded.gold - 12_345.0).abs() < 1e-6);
        assert_eq!(loaded.cycles, 4);
    }

    #[test]
    fn plain_json_save_still_imports() {
        // Старый формат — обычный JSON: он должен читаться и после перехода
        // на кодирование, иначе игрок потеряет перенесённое сохранение.
        let mut state = GameState::default();
        state.gold = 777.0;
        let json = serde_json::to_string(&state).expect("json");

        let loaded = import_text(&json).expect("импорт старого формата");
        assert!((loaded.gold - 777.0).abs() < 1e-6);
    }

    #[test]
    fn broken_code_is_rejected_with_a_message() {
        let error = import_text("POLE1:это не сохранение").expect_err("должно упасть");
        assert!(error.contains("повреждено"), "неожиданный текст: {error}");
    }
}
