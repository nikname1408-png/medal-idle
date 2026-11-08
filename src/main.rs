mod game;
mod ui;

fn main() {
    #[cfg(feature = "desktop")]
    {
        let config = desktop_config();
        dioxus::LaunchBuilder::desktop()
            .with_cfg(config)
            .launch(ui::App);
    }

    #[cfg(not(feature = "desktop"))]
    {
        dioxus::launch(ui::App);
    }
}

/// Стабильная папка WebView2, чтобы сохранения не терялись между пересборками.
#[cfg(feature = "desktop")]
fn desktop_config() -> dioxus::desktop::Config {
    let config = dioxus::desktop::Config::new();
    match std::env::var_os("LOCALAPPDATA") {
        Some(local) => {
            config.with_data_directory(std::path::PathBuf::from(local).join("medieval-idle"))
        }
        None => config,
    }
}
