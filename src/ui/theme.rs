//! Application theme preference.
//!
//! krusty follows the operating system appearance unless the user picks an
//! explicit light or dark override. The preference is stored next to the
//! cluster store so it survives restarts.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, BorrowAppContext as _, Global, Window, WindowAppearance};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PREFERENCES_FILE: &str = "preferences.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn resolve(self, system: ThemeMode) -> ThemeMode {
        match self {
            Self::System => system,
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
        }
    }

    fn native_appearance(self) -> Option<WindowAppearance> {
        match self {
            Self::System => None,
            Self::Light => Some(WindowAppearance::Light),
            Self::Dark => Some(WindowAppearance::Dark),
        }
    }
}

#[derive(Clone)]
pub struct AppearanceState {
    preference: ThemePreference,
    system: ThemeMode,
}

impl Global for AppearanceState {}

impl AppearanceState {
    pub fn preference(&self) -> ThemePreference {
        self.preference
    }

    pub fn system_mode(&self) -> ThemeMode {
        self.system
    }
}

/// Load the saved preference and apply it before the first window opens.
pub fn init(cx: &mut App) {
    let system = ThemeMode::from(cx.window_appearance());
    let preference = Preferences::load().theme;
    cx.set_global(AppearanceState { preference, system });
    apply(cx, preference, system);
    tracing::debug!(
        preference = preference.label(),
        system = system.name(),
        "ui: theme preference applied"
    );
}

/// Switch the preference, persist it, and update the window chrome.
pub fn set_preference(cx: &mut App, preference: ThemePreference) {
    let system = cx.global::<AppearanceState>().system_mode();
    if cx.global::<AppearanceState>().preference() == preference {
        return;
    }
    cx.update_global::<AppearanceState, _>(|state, _| state.preference = preference);
    Preferences { theme: preference }.save();
    apply(cx, preference, system);
    tracing::info!(
        preference = preference.label(),
        "ui: theme preference changed"
    );
}

/// Track the system appearance. An explicit light/dark choice ignores it.
pub fn sync_system_appearance(window: &mut Window, cx: &mut App) {
    let system = ThemeMode::from(window.appearance());
    let preference = cx.global::<AppearanceState>().preference();
    if preference != ThemePreference::System {
        return;
    }
    if cx.global::<AppearanceState>().system_mode() == system {
        return;
    }
    cx.update_global::<AppearanceState, _>(|state, _| state.system = system);
    Theme::change(system, Some(window), cx);
    tracing::debug!(system = system.name(), "ui: system appearance changed");
}

fn apply(cx: &mut App, preference: ThemePreference, stored_system: ThemeMode) {
    let system = if preference == ThemePreference::System {
        cx.set_window_appearance(None);
        ThemeMode::from(cx.window_appearance())
    } else {
        cx.set_window_appearance(preference.native_appearance());
        stored_system
    };
    if preference == ThemePreference::System && system != stored_system {
        cx.update_global::<AppearanceState, _>(|state, _| state.system = system);
    }
    Theme::change(preference.resolve(system), None, cx);
}

#[derive(Serialize, Deserialize, Default)]
struct Preferences {
    #[serde(default)]
    theme: ThemePreference,
}

impl Preferences {
    /// Path under the OS app config dir, e.g. ~/.config/krusty/preferences.json.
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("krusty").join(PREFERENCES_FILE))
    }

    fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "ui: invalid preferences file, using defaults"
                );
                Self::default()
            }),
            Err(error) => {
                tracing::debug!(
                    path = %path.display(),
                    %error,
                    "ui: no preferences file yet"
                );
                Self::default()
            }
        }
    }

    fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!(
                path = %path.display(),
                %error,
                "ui: failed to save theme preference"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_preference_follows_the_system_mode() {
        assert_eq!(
            ThemePreference::System.resolve(ThemeMode::Light),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::System.resolve(ThemeMode::Dark),
            ThemeMode::Dark
        );
    }

    #[test]
    fn explicit_preferences_ignore_the_system_mode() {
        assert_eq!(
            ThemePreference::Light.resolve(ThemeMode::Dark),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::Dark.resolve(ThemeMode::Light),
            ThemeMode::Dark
        );
    }

    #[test]
    fn preference_defaults_to_following_the_system() {
        let preferences: Preferences = serde_json::from_str("{}").expect("empty preferences");
        assert_eq!(preferences.theme, ThemePreference::System);
    }

    #[test]
    fn preference_round_trips_through_json() {
        for preference in ThemePreference::ALL {
            let encoded = serde_json::to_string(&Preferences { theme: preference })
                .expect("serialize preference");
            let decoded: Preferences = serde_json::from_str(&encoded).expect("parse preference");
            assert_eq!(decoded.theme, preference);
        }
    }
}
