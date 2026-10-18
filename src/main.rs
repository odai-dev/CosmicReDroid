mod app;
pub mod backend;
pub mod config;
pub mod i18n;
pub mod models;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let _ = i18n::language_loader();

    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<app::AppModel>(settings, ())?;
    Ok(())
}
