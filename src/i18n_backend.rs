use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct FileBackend {
    translations: HashMap<String, HashMap<String, String>>,
}

impl FileBackend {
    pub fn new() -> Self {
        let mut translations = HashMap::new();

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let locales_dir = exe_dir.join("locales");

        for (locale, file_name) in [("en", "en.toml"), ("ru", "ru.toml"), ("fr", "fr.toml")] {
            let path = locales_dir.join(file_name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(parsed) = toml::from_str::<HashMap<String, String>>(&content) {
                    translations.insert(locale.to_string(), parsed);
                }
            }
        }

        Self { translations }
    }
}

impl rust_i18n::Backend for FileBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        self.translations
            .keys()
            .map(|k| Cow::from(k.as_str()))
            .collect()
    }

    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        self.translations
            .get(locale)?
            .get(key)
            .map(|v| Cow::from(v.as_str()))
    }

    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>> {
        let map = self.translations.get(locale)?;
        Some(
            map.iter()
                .map(|(k, v)| (Cow::from(k.as_str()), Cow::from(v.as_str())))
                .collect(),
        )
    }
}
