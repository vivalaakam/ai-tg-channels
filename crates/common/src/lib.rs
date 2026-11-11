use std::path::PathBuf;

use dotenvy::dotenv;

/// Загружает `.env` из корня репозитория (три уровня вверх от CARGO_MANIFEST_DIR
/// в dev-режиме, либо из cwd) и значения переменных окружения.
///
/// Переменные берутся только из окружения/`.env` — никогда из CLI-аргументов.
pub fn load_env() {
    dotenv().ok();
    if let Some(root) = workspace_root() {
        let _ = dotenvy::from_path_override(root.join(".env"));
        let _ = dotenvy::from_path(root.join(".env"));
    }
}

/// Корень репозитория: три уровня вверх от CARGO_MANIFEST_DIR (даже если
/// крейт живёт в crates/<name> — workspace-корень неизменен).
pub fn workspace_root() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest.parent()?.parent()?.parent()?.to_path_buf();
    Some(dir)
}

pub fn require(name: &str) -> String {
    let val = std::env::var(name).unwrap_or_default();
    if val.is_empty() {
        panic!("Missing required env var {name}. Copy .env.example to .env and fill it.");
    }
    val
}

pub fn optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

pub fn require_i64(name: &str) -> i64 {
    require(name).parse().expect("env var must be an integer")
}

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

#[cfg(test)]
mod tests {
    #[test]
    fn require_panics_on_missing() {
        let name = "AI_TG_CHANNELS_RS_DEFINITELY_NOT_SET_828371";
        // SAFETY: тесты однопоточны, конкурентного чтения переменной нет
        unsafe { std::env::remove_var(name) };
        let result = std::panic::catch_unwind(|| super::require(name));
        assert!(result.is_err());
    }

    #[test]
    fn optional_returns_none_for_empty() {
        let name = "AI_TG_CHANNELS_RS_OPTIONAL_TEST_828371";
        // SAFETY: тесты однопоточны, конкурентного чтения переменной нет
        unsafe { std::env::remove_var(name) };
        assert!(super::optional(name).is_none());
    }
}
