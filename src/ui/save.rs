//! Сохранение прогресса в localStorage через WebView.
//!
//! Работает и на десктопе (WebView2), и на Android (WebView).

use dioxus::prelude::*;

use crate::game::GameState;

/// Ключ, под которым лежит JSON сохранения.
const SAVE_KEY: &str = "pole_save";

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
