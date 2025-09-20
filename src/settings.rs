use crate::settings::ColorScheme::Egui;
use egui::Context;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use strum_macros::{EnumIter, EnumString};

const BIG_FILE_THRESHOLD: u64 = 10000000;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Settings {
    #[serde(skip)]
    /// Mark the Settings as dirty (need to be saved)
    pub(crate) dirty: bool,
    color_scheme: ColorScheme,
    theme: ThemePreference,
    /// List of paths to ignore (might be cloud drives, etc.
    ignored_path: Vec<PathBuf>,
    /// Ignore common cloud folders like Dropbox, OneDrive, Google Drive, iCloud, etc.
    #[serde(default = "Settings::default_ignore_cloud_mounts")]
    pub(crate) ignore_cloud_mounts: bool,
    /// Threshold for big files (in bytes). Files smaller than this will be displayed as a single block.
    pub(crate) big_file_threshold: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self::settings_file()
            .and_then(|settings_file| File::open(settings_file).ok())
            .and_then(|settings_file| serde_json::from_reader::<File, Settings>(settings_file).ok())
            .unwrap_or(Self {
                dirty: false,
                color_scheme: Egui,
                theme: ThemePreference::System,
                ignored_path: Vec::new(),
                ignore_cloud_mounts: true,
                big_file_threshold: BIG_FILE_THRESHOLD,
            })
    }
}

impl Settings {
    pub fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    pub fn color_scheme_mut(&mut self) -> &mut ColorScheme {
        &mut self.color_scheme
    }

    pub(crate) fn theme(&self) -> ThemePreference {
        self.theme
    }

    pub(crate) fn set_theme(&mut self, theme: ThemePreference) {
        self.theme = theme;
        self.dirty = true;
    }

    pub(crate) fn init(&self, ctx: &Context) {
        ctx.set_theme(self.theme);
        self.color_scheme.apply(ctx);
    }

    pub(crate) fn add_ignored_path(&mut self, path: PathBuf) {
        info!("add ignored path: {path:?}");
        self.ignored_path.push(path);
        self.dirty = true;
    }

    pub(crate) fn is_path_ignored(&self, path: &PathBuf) -> bool {
        if self.ignored_path.contains(path) {
            return true;
        }
        if self.ignore_cloud_mounts && Self::is_common_cloud_path(path.as_path()) {
            return true;
        }
        false
    }

    pub(crate) fn ignored_paths_mut(&mut self) -> &mut Vec<PathBuf> {
        &mut self.ignored_path
    }

    pub(crate) fn big_file_threshold(&self) -> u64 {
        self.big_file_threshold
    }

    pub(crate) fn reset_big_file_threshold(&mut self) {
        self.big_file_threshold = BIG_FILE_THRESHOLD;
        self.dirty = true;
    }

    pub(crate) fn save(&self) -> Result<(), std::io::Error> {
        info!("save");
        if self.dirty
            && let Some(settings_folder) = Self::settings_folder()
        {
            std::fs::create_dir_all(settings_folder)?;
            if let Some(settings_file) = Self::settings_file() {
                serde_json::to_writer(File::create(settings_file)?, self)?;
            }
        }
        Ok(())
    }

    fn default_ignore_cloud_mounts() -> bool {
        true
    }

    fn settings_folder() -> Option<PathBuf> {
        home::home_dir().map(|mut home| {
            home.push(".disk-mosaic");
            home
        })
    }

    fn settings_file() -> Option<PathBuf> {
        Self::settings_folder().map(|mut settings_folder| {
            settings_folder.push("settings.json");
            settings_folder
        })
    }

    fn is_common_cloud_path(path: &std::path::Path) -> bool {
        // Linux absolute mount points that often include cloud/special mounts
        #[cfg(target_os = "linux")]
        {
            if path.starts_with("/run/user")
                || path.starts_with("/media")
                || path.starts_with("/mnt")
                || path.starts_with("/snap")
            {
                return true;
            }
        }
        // On macOS and Windows, many cloud folders are under HOME. We check the first component
        // after HOME against a small set of known names without allocating vectors.
        if let Some(home) = home::home_dir()
            && let Ok(stripped) = path.strip_prefix(&home)
        {
            // iCloud special case is under ~/Library/Mobile Documents/com~apple~CloudDocs
            #[cfg(target_os = "macos")]
            {
                let mut comps = stripped.components();
                if matches!(comps.next(), Some(std::path::Component::Normal(s)) if s == "Library")
                    && matches!(comps.next(), Some(std::path::Component::Normal(s)) if s == "Mobile Documents")
                    && matches!(comps.next(), Some(std::path::Component::Normal(s)) if s == "com~apple~CloudDocs")
                {
                    return true;
                }
            }
            // Check top-level directory under HOME for common cloud providers
            if let Some(first) = stripped.components().next()
                && let std::path::Component::Normal(name) = first
                && let Some(s) = name.to_str()
            {
                // Match a small set of known names
                return matches!(
                    s,
                    "Dropbox"
                        | "OneDrive"
                        | "OneDrive - Personal"
                        | "Google Drive"
                        | "Google Drive (Shared)"
                        | "Box"
                        | "Nextcloud"
                        | "SynologyDrive"
                        | "pCloud Drive"
                        | "MEGA"
                );
            }
        }
        false
    }
}

#[derive(
    Debug, Serialize, Deserialize, EnumIter, EnumString, Clone, Copy, PartialEq, Eq, Hash, Default,
)]
pub enum ColorScheme {
    #[default]
    Egui,
    Solarized,
}

impl ColorScheme {
    pub(crate) fn apply(&self, ctx: &Context) {
        match self {
            Egui => {
                ctx.options_mut(|options| {
                    options.dark_style = Arc::new(egui::Theme::Dark.default_style());
                    options.light_style = Arc::new(egui::Theme::Light.default_style());
                });
            }
            ColorScheme::Solarized => egui_solarized::install(ctx),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

impl From<ThemePreference> for egui::ThemePreference {
    fn from(theme: ThemePreference) -> Self {
        match theme {
            ThemePreference::System => egui::ThemePreference::System,
            ThemePreference::Dark => egui::ThemePreference::Dark,
            ThemePreference::Light => egui::ThemePreference::Light,
        }
    }
}
