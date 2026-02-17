use std::io;

#[derive(Debug, thiserror::Error)]
pub enum WakalyzeError {
    #[error("month must be in YYYY/MM format")]
    InvalidMonth,

    #[error("week must be between 1 and 6")]
    InvalidWeek,

    #[error("week is out of range for the month")]
    WeekOutOfRange(u32),

    #[error("missing auth: set WAKAPI_KEY or run `wakalyze config set --key <token>`")]
    MissingAuth,

    #[error("missing user: use --user, set WAKAPI_USER, or run `wakalyze config set --user`")]
    MissingUser,

    #[error("--max-gap-minutes must be greater than 0")]
    InvalidMaxGap,

    #[error("{0}")]
    ConflictingFlags(String),

    #[error("nothing to update: provide --key/--user/--base-url")]
    NothingToUpdate,

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("config I/O error: {0}")]
    ConfigIo(#[from] io::Error),

    #[error("config parse error: {0}")]
    ConfigParse(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, WakalyzeError>;
