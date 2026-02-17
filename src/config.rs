use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

pub fn config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("wakalyze").join("config.json");
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("wakalyze")
        .join("config.json")
}

pub fn load_config_from(path: &std::path::Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Config::default(),
    };
    let raw: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Config::default(),
    };
    let obj = match raw.as_object() {
        Some(o) => o,
        None => return Config::default(),
    };

    let str_field = |key: &str| -> Option<String> {
        let val = obj.get(key)?.as_str()?;
        if val.trim().is_empty() {
            None
        } else {
            Some(val.to_string())
        }
    };

    Config {
        key: str_field("key"),
        user: str_field("user"),
        base_url: str_field("base_url"),
    }
}

pub fn load_config() -> Config {
    load_config_from(&config_path())
}

pub fn save_config_to(path: &std::path::Path, config: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    let content = format!("{json}\n");

    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&temp_path, perms)?;
    }

    std::fs::rename(&temp_path, path)?;
    Ok(())
}

pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.len() <= 4 {
        return "*".repeat(value.len());
    }
    format!(
        "{}{}",
        "*".repeat(value.len() - 4),
        &value[value.len() - 4..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_xdg() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap().to_string();
        std::env::set_var("XDG_CONFIG_HOME", &dir_str);
        let result = config_path();
        std::env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(result, dir.path().join("wakalyze").join("config.json"));
    }

    #[test]
    fn config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wakalyze").join("config.json");
        let config = Config {
            key: Some("tok".into()),
            user: Some("me".into()),
            base_url: None,
        };
        save_config_to(&path, &config).unwrap();
        let loaded = load_config_from(&path);
        assert_eq!(loaded.key.as_deref(), Some("tok"));
        assert_eq!(loaded.user.as_deref(), Some("me"));
    }

    #[test]
    fn load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wakalyze").join("config.json");
        let loaded = load_config_from(&path);
        assert_eq!(loaded, Config::default());
    }

    #[test]
    fn load_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not json").unwrap();
        let loaded = load_config_from(&path);
        assert_eq!(loaded, Config::default());
    }

    #[test]
    fn load_non_dict() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "[1,2,3]").unwrap();
        let loaded = load_config_from(&path);
        assert_eq!(loaded, Config::default());
    }

    #[test]
    fn load_filters_non_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"key":"valid","user":123}"#).unwrap();
        let loaded = load_config_from(&path);
        assert_eq!(loaded.key.as_deref(), Some("valid"));
        assert!(loaded.user.is_none());
    }

    #[test]
    fn mask_secret_empty() {
        assert_eq!(mask_secret(""), "");
    }

    #[test]
    fn mask_secret_short() {
        assert_eq!(mask_secret("abc"), "***");
    }

    #[test]
    fn mask_secret_four() {
        assert_eq!(mask_secret("abcd"), "****");
    }

    #[test]
    fn mask_secret_longer() {
        assert_eq!(mask_secret("abcdef"), "**cdef");
    }
}
