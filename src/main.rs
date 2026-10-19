fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let _ = cosmic_redroid::i18n::language_loader();

    let settings = cosmic::app::Settings::default();
    cosmic::app::run::<cosmic_redroid::app::AppModel>(settings, ())?;
    Ok(())
}
