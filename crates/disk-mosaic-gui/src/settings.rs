use crate::settings::ColorScheme::Egui;
use disk_mosaic_core::directory_scanner::{ScanConfig, BIG_FILE_THRESHOLD};
use log::info;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Default, Debug, Clone)]
pub(crate) struct Settings {
    inner: Arc<RwLock<InnerSettings>>,
}

impl ScanConfig for Settings {
    fn big_file_threshold(&self) -> u64 {
        self.inner.read().unwrap().big_file_threshold
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.inner.read().unwrap().is_path_ignored(path)
    }
}

impl Settings {
    pub(crate) fn big_file_threshold_mut(&self) -> SettingsBigFileThresholdMut<'_> {
        SettingsBigFileThresholdMut {
            settings: self.inner.write().unwrap(),
        }
    }

    pub(crate) fn reset_big_file_threshold(&self) {
        let mut settings = self.inner.write().unwrap();
        settings.big_file_threshold = BIG_FILE_THRESHOLD;
        settings.dirty = true;
    }

    pub(crate) fn color_scheme(&self) -> ColorScheme {
        self.inner.read().unwrap().color_scheme
    }

    pub(crate) fn color_scheme_mut(&self) -> SettingsColorSchemeMut<'_> {
        SettingsColorSchemeMut {
            settings: self.inner.write().unwrap(),
        }
    }

    pub(crate) fn ignored_paths_mut(&self) -> SettingsIgnoredPathMut<'_> {
        SettingsIgnoredPathMut {
            settings: self.inner.write().unwrap(),
        }
    }

    pub(crate) fn ignore_cloud_mounts_mut(&self) -> SettingsIgnoreCloudMountsMut<'_> {
        SettingsIgnoreCloudMountsMut {
            settings: self.inner.write().unwrap(),
        }
    }

    pub(crate) fn save(&self) -> Result<(), std::io::Error> {
        self.inner.read().unwrap().save()
    }

    pub(crate) fn set_dirty(&self, dirty: bool) {
        self.inner.write().unwrap().dirty = dirty
    }

    pub(crate) fn theme(&self) -> ThemePreference {
        self.inner.read().unwrap().theme
    }

    pub(crate) fn set_theme(&self, theme: ThemePreference) {
        let mut settings = self.inner.write().unwrap();
        settings.theme = theme;
        settings.dirty = true;
    }

    pub(crate) fn init(&self, ctx: &egui::Context) {
        self.inner.read().unwrap().init(ctx);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InnerSettings {
    #[serde(skip)]
    /// Mark the Settings as dirty (need to be saved)
    dirty: bool,
    color_scheme: ColorScheme,
    theme: ThemePreference,
    /// List of paths to ignore (might be cloud drives, etc.
    ignored_path: Vec<PathBuf>,
    /// Ignore common cloud folders like Dropbox, OneDrive, Google Drive, iCloud, etc.
    #[serde(default = "InnerSettings::default_ignore_cloud_mounts")]
    ignore_cloud_mounts: bool,
    /// Threshold for big files (in bytes). Files smaller than this will be displayed as a single block.
    big_file_threshold: u64,
}

impl Default for InnerSettings {
    fn default() -> Self {
        Self::settings_file()
            .and_then(|settings_file| File::open(settings_file).ok())
            .and_then(|settings_file| serde_json::from_reader::<File, Self>(settings_file).ok())
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

impl InnerSettings {
    pub(crate) fn init(&self, ctx: &egui::Context) {
        ctx.set_theme(self.theme);
        self.color_scheme.apply(ctx);
    }

    pub(crate) fn is_path_ignored(&self, path: &Path) -> bool {
        if self
            .ignored_path
            .iter()
            .any(|ignored_path| ignored_path == path)
        {
            return true;
        }
        if self.ignore_cloud_mounts && Self::is_common_cloud_path(path) {
            return true;
        }
        false
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

    const fn default_ignore_cloud_mounts() -> bool {
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

    fn is_common_cloud_path(path: &Path) -> bool {
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
pub(crate) enum ColorScheme {
    #[default]
    Egui,
    Solarized,
}

impl ColorScheme {
    pub(crate) fn apply(&self, ctx: &egui::Context) {
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

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Display, EnumIter,
)]
pub(crate) enum ThemePreference {
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

impl PartialEq for SettingsColorSchemeMut<'_> {
    fn eq(&self, other: &Self) -> bool {
        let color_scheme: &ColorScheme = self.deref();
        color_scheme == other.deref()
    }
}

macro_rules! settings_mut_guard {
    ($name:ident, $field:ident, $target:ty) => {
        pub(crate) struct $name<'a> {
            settings: RwLockWriteGuard<'a, InnerSettings>,
        }

        impl Deref for $name<'_> {
            type Target = $target;

            fn deref(&self) -> &Self::Target {
                &self.settings.$field
            }
        }

        impl DerefMut for $name<'_> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.settings.dirty = true;
                &mut self.settings.$field
            }
        }
    };
}

settings_mut_guard!(SettingsColorSchemeMut, color_scheme, ColorScheme);
settings_mut_guard!(SettingsIgnoredPathMut, ignored_path, Vec<PathBuf>);
settings_mut_guard!(SettingsBigFileThresholdMut, big_file_threshold, u64);
settings_mut_guard!(SettingsIgnoreCloudMountsMut, ignore_cloud_mounts, bool);
