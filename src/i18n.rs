use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _ = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

pub fn language_loader() -> &'static FluentLanguageLoader {
    &LANGUAGE_LOADER
}

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {
        i18n_embed_fl::fl!($crate::i18n::language_loader(), $message_id)
    };
    ($message_id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::language_loader(), $message_id, $($args)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_catalog_load() {
        let _ = language_loader();
        let title = crate::fl!("app-title");
        assert_eq!(title, "CosmicReDroid");
    }

    #[test]
    fn test_i18n_message_with_args() {
        let _ = language_loader();
        let active = crate::fl!("active-containers", count = 3);
        assert!(active.contains("Active Containers:"));
        assert!(active.contains('3'));
    }

    #[test]
    fn test_i18n_common_keys() {
        let _ = language_loader();
        assert_eq!(crate::fl!("nav-instances"), "Instances");
        assert_eq!(crate::fl!("btn-start"), "Start");
        assert_eq!(crate::fl!("host-health-title"), "Host System Diagnostics");
    }
}
