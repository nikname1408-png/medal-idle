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
