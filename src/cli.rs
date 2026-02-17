use std::io::IsTerminal;

use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};

use crate::client::{encode_api_key, WakapiClient, DEFAULT_BASE_URL};
use crate::config::{
    config_path, load_config, load_config_from, mask_secret, save_config_to, Config,
};
use crate::core::{
    build_sessions, filter_sessions, iter_dates, month_last_day, parse_month, week_range,
    DaySessions, DEFAULT_MAX_GAP_SECONDS,
};
use crate::error::{Result, WakalyzeError};
use crate::format::build_lines;

#[derive(Parser)]
#[command(
    name = "wakalyze",
    version,
    about = "List Wakapi working hours per day"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze Wakapi heartbeats for a month/week
    Analyze(AnalyzeArgs),
    /// Manage wakalyze stored config
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Month in YYYY/MM format
    pub month: String,

    /// Week of month (1-6)
    pub week: Option<u32>,

    /// Filter by project substring (comma-separated terms = OR)
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Wakapi user (or env WAKAPI_USER)
    #[arg(long)]
    pub user: Option<String>,

    /// Wakapi base URL (or env WAKAPI_BASE_URL)
    #[arg(long)]
    pub base_url: Option<String>,

    /// HTTP request timeout in seconds
    #[arg(long, default_value_t = 15.0)]
    pub timeout: f64,

    /// Max gap in minutes between heartbeats to treat as continuous work
    #[arg(long, default_value_t = DEFAULT_MAX_GAP_SECONDS as f64 / 60.0)]
    pub max_gap_minutes: f64,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Print config file path
    Path,
    /// Show stored config values
    Show,
    /// Set stored config values
    Set(ConfigSetArgs),
}

#[derive(Args)]
pub struct ConfigSetArgs {
    /// Wakapi API token
    #[arg(long)]
    pub key: Option<String>,

    /// Wakapi user
    #[arg(long)]
    pub user: Option<String>,

    /// Wakapi base URL
    #[arg(long)]
    pub base_url: Option<String>,

    /// Remove stored key
    #[arg(long)]
    pub clear_key: bool,

    /// Remove stored user
    #[arg(long)]
    pub clear_user: bool,

    /// Remove stored base URL
    #[arg(long)]
    pub clear_base_url: bool,
}

fn non_empty(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

pub fn resolve_basic_auth(config: &Config) -> Result<String> {
    if let Some(key) = config.key.as_deref().and_then(non_empty) {
        return Ok(encode_api_key(key));
    }
    if let Some(key) = resolve_from_env("WAKAPI_KEY") {
        return Ok(encode_api_key(&key));
    }
    Err(WakalyzeError::MissingAuth)
}

fn resolve_from_env(env_var: &str) -> Option<String> {
    std::env::var(env_var)
        .ok()
        .as_deref()
        .and_then(non_empty)
        .map(str::to_owned)
}

fn resolve_field(arg: Option<&str>, config_val: Option<&str>, env_var: &str) -> Option<String> {
    arg.and_then(non_empty)
        .or_else(|| config_val.and_then(non_empty))
        .map(str::to_owned)
        .or_else(|| resolve_from_env(env_var))
}

fn resolve_user(args_user: Option<&str>, config: &Config) -> Result<String> {
    resolve_field(args_user, config.user.as_deref(), "WAKAPI_USER")
        .ok_or(WakalyzeError::MissingUser)
}

fn resolve_base_url(args_url: Option<&str>, config: &Config) -> String {
    resolve_field(args_url, config.base_url.as_deref(), "WAKAPI_BASE_URL")
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

fn update_field(
    target: &mut Option<String>,
    value: Option<&str>,
    clear: bool,
    label: &str,
) -> Result<bool> {
    if clear && value.is_some() {
        return Err(WakalyzeError::ConflictingFlags(format!(
            "cannot use --{label} and --clear-{label} together"
        )));
    }
    if clear {
        *target = None;
        return Ok(true);
    }
    match value {
        None => Ok(false),
        Some(v) => {
            *target = non_empty(v).map(str::to_owned);
            Ok(true)
        }
    }
}

pub fn handle_config(action: ConfigAction) -> Result<()> {
    handle_config_with_path(action, &config_path())
}

pub fn handle_config_with_path(action: ConfigAction, path: &std::path::Path) -> Result<()> {
    match action {
        ConfigAction::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigAction::Show => {
            let config = load_config_from(path);
            println!("path: {}", path.display());
            println!("user: {}", config.user.as_deref().unwrap_or("(unset)"));
            println!(
                "base_url: {}",
                config.base_url.as_deref().unwrap_or("(unset)")
            );
            let key_display = config
                .key
                .as_deref()
                .map(mask_secret)
                .unwrap_or_else(|| "(unset)".to_string());
            println!("key: {key_display}");
            Ok(())
        }
        ConfigAction::Set(args) => {
            let mut config = load_config_from(path);
            let mut updated = false;
            updated |= update_field(&mut config.key, args.key.as_deref(), args.clear_key, "key")?;
            updated |= update_field(
                &mut config.user,
                args.user.as_deref(),
                args.clear_user,
                "user",
            )?;
            updated |= update_field(
                &mut config.base_url,
                args.base_url.as_deref(),
                args.clear_base_url,
                "base-url",
            )?;
            if !updated {
                return Err(WakalyzeError::NothingToUpdate);
            }
            save_config_to(path, &config)?;
            Ok(())
        }
    }
}

pub fn handle_analyze(args: AnalyzeArgs) -> Result<()> {
    let config = load_config();
    let base_url = resolve_base_url(args.base_url.as_deref(), &config);

    let first_day = parse_month(&args.month)?;
    let (start, end, label) = if let Some(week) = args.week {
        let (s, e) = week_range(first_day, week)?;
        (s, e, format!("{} week {week}", first_day.format("%Y/%m")))
    } else {
        let last = month_last_day(first_day);
        (first_day, last, first_day.format("%Y/%m").to_string())
    };

    let user = resolve_user(args.user.as_deref(), &config)?;
    let auth = resolve_basic_auth(&config)?;

    let max_gap_seconds = (args.max_gap_minutes * 60.0) as i64;
    if max_gap_seconds <= 0 {
        return Err(WakalyzeError::InvalidMaxGap);
    }

    let client = WakapiClient::new(&base_url, &user, &auth, args.timeout);
    let dates = iter_dates(start, end);

    let is_terminal = std::io::stderr().is_terminal();
    let pb = if is_terminal {
        let pb = ProgressBar::new(dates.len() as u64).with_finish(ProgressFinish::AndClear);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner} Loading heartbeats {bar:30} {pos}/{len} [{elapsed}]",
            )
            .unwrap(),
        );
        pb
    } else {
        ProgressBar::hidden()
    };

    let mut days = Vec::with_capacity(dates.len());
    for date in &dates {
        let heartbeats = client.fetch_heartbeats(*date)?;
        days.push(DaySessions {
            date: *date,
            sessions: build_sessions(&heartbeats, max_gap_seconds),
        });
        pb.inc(1);
    }
    pb.finish_and_clear();

    let days = filter_sessions(&days, args.filter.as_deref());

    for line in build_lines(&days, &label) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn resolve_auth_env_key() {
        std::env::set_var("WAKAPI_KEY", "envtok");
        let result = resolve_basic_auth(&Config::default()).unwrap();
        std::env::remove_var("WAKAPI_KEY");
        let expected = encode_api_key("envtok");
        assert_eq!(result, expected);
    }

    #[test]
    #[serial]
    fn resolve_auth_config_key() {
        std::env::remove_var("WAKAPI_KEY");
        let config = Config {
            key: Some("cfgtok".into()),
            ..Default::default()
        };
        let result = resolve_basic_auth(&config).unwrap();
        let expected = encode_api_key("cfgtok");
        assert_eq!(result, expected);
    }

    #[test]
    #[serial]
    fn resolve_auth_missing() {
        std::env::remove_var("WAKAPI_KEY");
        let result = resolve_basic_auth(&Config::default());
        assert!(result.is_err());
    }

    #[test]
    fn update_field_set_value() {
        let mut field = None;
        let result = update_field(&mut field, Some("val"), false, "key").unwrap();
        assert!(result);
        assert_eq!(field.as_deref(), Some("val"));
    }

    #[test]
    fn update_field_clear() {
        let mut field = Some("val".to_string());
        let result = update_field(&mut field, None, true, "key").unwrap();
        assert!(result);
        assert!(field.is_none());
    }

    #[test]
    fn update_field_noop() {
        let mut field = None;
        let result = update_field(&mut field, None, false, "key").unwrap();
        assert!(!result);
    }

    #[test]
    fn update_field_clear_and_value_errors() {
        let mut field = None;
        let result = update_field(&mut field, Some("val"), true, "key");
        assert!(result.is_err());
    }

    #[test]
    fn config_set_and_show() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wakalyze").join("config.json");

        let set_args = ConfigSetArgs {
            key: None,
            user: Some("testuser".into()),
            base_url: None,
            clear_key: false,
            clear_user: false,
            clear_base_url: false,
        };
        handle_config_with_path(ConfigAction::Set(set_args), &path).unwrap();

        let loaded = load_config_from(&path);
        assert_eq!(loaded.user.as_deref(), Some("testuser"));
    }

    #[test]
    fn config_no_updates_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wakalyze").join("config.json");
        let set_args = ConfigSetArgs {
            key: None,
            user: None,
            base_url: None,
            clear_key: false,
            clear_user: false,
            clear_base_url: false,
        };
        let result = handle_config_with_path(ConfigAction::Set(set_args), &path);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_auth_config_overrides_env() {
        std::env::set_var("WAKAPI_KEY", "envtok");
        let config = Config {
            key: Some("cfgtok".into()),
            ..Default::default()
        };
        let result = resolve_basic_auth(&config).unwrap();
        std::env::remove_var("WAKAPI_KEY");
        let expected = encode_api_key("cfgtok");
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_user_config_overrides_env() {
        std::env::set_var("WAKAPI_USER", "envuser");
        let config = Config {
            user: Some("cfguser".into()),
            ..Default::default()
        };
        let result = resolve_user(None, &config).unwrap();
        std::env::remove_var("WAKAPI_USER");
        assert_eq!(result, "cfguser");
    }

    #[test]
    fn resolve_base_url_config_overrides_env() {
        std::env::set_var("WAKAPI_BASE_URL", "https://env.example.com");
        let config = Config {
            base_url: Some("https://cfg.example.com".into()),
            ..Default::default()
        };
        let result = resolve_base_url(None, &config);
        std::env::remove_var("WAKAPI_BASE_URL");
        assert_eq!(result, "https://cfg.example.com");
    }

    #[test]
    fn resolve_user_args_overrides_config() {
        let config = Config {
            user: Some("cfguser".into()),
            ..Default::default()
        };
        let result = resolve_user(Some("arguser"), &config).unwrap();
        assert_eq!(result, "arguser");
    }

    #[test]
    fn resolve_user_missing() {
        std::env::remove_var("WAKAPI_USER");
        let result = resolve_user(None, &Config::default());
        assert!(result.is_err());
    }

    #[test]
    fn resolve_base_url_args_overrides_config() {
        let config = Config {
            base_url: Some("https://cfg.example.com".into()),
            ..Default::default()
        };
        let result = resolve_base_url(Some("https://arg.example.com"), &config);
        assert_eq!(result, "https://arg.example.com");
    }

    #[test]
    fn resolve_base_url_default_fallback() {
        std::env::remove_var("WAKAPI_BASE_URL");
        let result = resolve_base_url(None, &Config::default());
        assert_eq!(result, DEFAULT_BASE_URL);
    }
}
