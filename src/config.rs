use crate::library::{Library, LibraryFlavor, LibraryType};
use anyhow::{anyhow, Result};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[cfg(feature = "secrets")]
use oo7::Keyring;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Default)]
pub struct Configuration {
    #[serde(default)]
    pub libraries: Vec<ConfigLibrary>,
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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ConfigLibrary {
    pub fs_type: LibraryType,
    pub flavor: LibraryFlavor,
    pub name: String,
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Credentials,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Credentials {
    #[default]
    None,
    #[cfg(feature = "secrets")]
    Keyring,
    #[cfg(feature = "secrets")]
    ToKeyring(String),
    Clear(String),
}

struct CredentialsVisitor;

impl<'de> de::Visitor<'de> for CredentialsVisitor {
    type Value = Credentials;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("credentials in its self-describing format, for instance: `None`, `Clear(password)`,...")
    }

    #[cfg(feature = "secrets")]
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let cmp = value.to_lowercase();
        if cmp == "none" {
            Ok(Credentials::None)
        } else if cmp == "keyring" {
            Ok(Credentials::Keyring)
        } else if let Some(v) = value.strip_suffix(")") {
            if cmp.starts_with("tokeyring(") {
                Ok(Credentials::ToKeyring(
                    v.get(10..)
                        .ok_or(de::Error::invalid_value(de::Unexpected::Str(value), &self))?
                        .to_string(),
                ))
            } else if cmp.starts_with("clear(") {
                Ok(Credentials::Clear(
                    v.get(6..)
                        .ok_or(de::Error::invalid_value(de::Unexpected::Str(value), &self))?
                        .to_string(),
                ))
            } else {
                Err(de::Error::invalid_value(
                    de::Unexpected::Other("unknown credentials variant (as a string)"),
                    &self,
                ))
            }
        } else {
            Err(de::Error::invalid_value(
                de::Unexpected::Other("unknown credentials variant (as a string)"),
                &self,
            ))
        }
    }

    #[cfg(not(feature = "secrets"))]
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let cmp = value.to_lowercase();
        if cmp == "none" {
            Ok(Credentials::None)
        } else if cmp == "keyring" {
            Err(de::Error::invalid_value(
                de::Unexpected::Other(
                    "unsupported credentials variant: enable feature `secrets` to use Keyring",
                ),
                &self,
            ))
        } else if let Some(v) = value.strip_suffix(")") {
            if cmp.starts_with("tokeyring(") {
                Err(de::Error::invalid_value(
                    de::Unexpected::Other("unsupported credentials variant: enable feature `secrets` to use ToKeyring"),
                    &self,
                ))
            } else if cmp.starts_with("clear(") {
                Ok(Credentials::Clear(
                    v.get(6..)
                        .ok_or(de::Error::invalid_value(de::Unexpected::Str(value), &self))?
                        .to_string(),
                ))
            } else {
                Err(de::Error::invalid_value(
                    de::Unexpected::Other("unknown credentials variant (as a string)"),
                    &self,
                ))
            }
        } else {
            Err(de::Error::invalid_value(
                de::Unexpected::Other("unknown credentials variant (as a string)"),
                &self,
            ))
        }
    }
}

impl Serialize for Credentials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Credentials::None => "None".into(),
            #[cfg(feature = "secrets")]
            Credentials::Keyring => "Keyring".into(),
            #[cfg(feature = "secrets")]
            Credentials::ToKeyring(s) => format!("ToKeyring({})", s),
            Credentials::Clear(s) => format!("Clear({})", s),
        };
        serializer.serialize_str(&value)
    }
}

impl<'de> Deserialize<'de> for Credentials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CredentialsVisitor)
    }
}

impl<T: Into<String>> From<Option<T>> for Credentials {
    fn from(s: Option<T>) -> Self {
        match s {
            Some(c) => Self::Clear(c.into()),
            None => Self::None,
        }
    }
}

#[cfg(not(feature = "secrets"))]
impl From<Credentials> for Option<String> {
    fn from(creds: Credentials) -> Option<String> {
        match creds {
            Credentials::None => None,
            Credentials::Clear(s) => Some(s),
        }
    }
}

#[cfg(not(feature = "secrets"))]
impl From<ConfigLibrary> for Library {
    fn from(lib: ConfigLibrary) -> Library {
        Library {
            fs_type: lib.fs_type,
            flavor: lib.flavor,
            name: lib.name,
            host: lib.host,
            username: lib.username,
            password: lib.password.into(),
            path: lib.path,
        }
    }
}

impl From<Library> for ConfigLibrary {
    fn from(lib: Library) -> ConfigLibrary {
        ConfigLibrary {
            fs_type: lib.fs_type,
            flavor: lib.flavor,
            name: lib.name,
            host: lib.host,
            username: lib.username,
            password: lib.password.into(),
            path: lib.path,
        }
    }
}

#[cfg(feature = "secrets")]
impl ConfigLibrary {
    pub async fn try_into_with_keyring(self, keyring: &Keyring) -> Result<Library> {
        let path = self.path.display().to_string();
        let password = match self.password {
            Credentials::Keyring => {
                let attributes = HashMap::from([
                    ("fs_type", self.fs_type.to_scheme()),
                    ("host", self.host.as_deref().unwrap_or("")),
                    ("username", self.username.as_deref().unwrap_or("")),
                    ("path", &path),
                ]);
                let secret = keyring
                    .search_items(attributes)
                    .await?
                    .get(0)
                    .map(|item| item.secret())
                    .ok_or(anyhow!("Password not found in keyring."))?
                    .await?;
                Some(String::from_utf8_lossy(&secret).into_owned())
            }
            Credentials::None => None,
            Credentials::ToKeyring(s) => Some(s),
            Credentials::Clear(s) => Some(s),
        };

        Ok(Library {
            fs_type: self.fs_type,
            flavor: self.flavor,
            name: self.name,
            host: self.host,
            username: self.username,
            password,
            path: self.path,
        })
    }

    pub async fn from_with_keyring(lib: Library, keyring: &Keyring) -> ConfigLibrary {
        let path = lib.path.display().to_string();
        let password = match lib.password {
            Some(c) => {
                let attributes = HashMap::from([
                    ("fs_type", lib.fs_type.to_scheme()),
                    ("host", lib.host.as_deref().unwrap_or("")),
                    ("username", lib.username.as_deref().unwrap_or("")),
                    ("path", &path),
                ]);
                match keyring.create_item(&lib.name, attributes, &c, true).await {
                    Ok(()) => Credentials::Keyring,
                    Err(err) => {
                        log::error!("Failed to save credentials to keyring, the credentials will be saved as clear text temporary. Cause:\n{:?}", err);
                        Credentials::ToKeyring(c)
                    }
                }
            }
            None => Credentials::None,
        };
        ConfigLibrary {
            fs_type: lib.fs_type,
            flavor: lib.flavor,
            name: lib.name,
            host: lib.host,
            username: lib.username,
            password,
            path: lib.path,
        }
    }
}
