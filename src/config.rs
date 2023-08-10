use crate::library::Library;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Default)]
pub struct Configuration {
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub tmdb_preferences: TmdbPreferences,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TmdbPreferences {
    #[serde(default)]
    pub prefered_lang: String,
    #[serde(default)]
    pub prefered_country: String,
}

impl Default for TmdbPreferences {
    fn default() -> Self {
        Self {
            prefered_lang: "en".into(),
            prefered_country: "US".into(),
        }
    }
}
