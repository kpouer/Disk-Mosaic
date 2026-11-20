use crate::settings::ColorScheme::Egui;
use disk_mosaic_core::directory_scanner::config::ScanConfig;
use disk_mosaic_core::scan_settings::{BIG_FILE_THRESHOLD, ScanSettings};
use log::info;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Default, Debug, Clone)]
pub(crate) struct Settings {
    inner: Arc<RwLock<InnerSettings>>,
}

impl ScanConfig for Settings {
    fn big_file_threshold(&self) -> u64 {
        self.read().scan_settings.big_file_threshold
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.read().is_path_ignored(path)
    }

    #[cfg(unix)]
    fn ignore_different_device(&self) -> bool {
        self.read().ignore_different_device()
    }
}

impl Settings {
    /// A poisoned lock still holds valid settings: recover it instead of panicking.
    fn read(&self) -> RwLockReadGuard<'_, InnerSettings> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, InnerSettings> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn big_file_threshold_mut(&self) -> SettingsBigFileThresholdMut<'_> {
        SettingsBigFileThresholdMut {
            settings: self.write(),
        }
    }

    pub(crate) fn reset_big_file_threshold(&self) {
        let mut settings = self.write();
        settings.scan_settings.big_file_threshold = BIG_FILE_THRESHOLD;
        settings.dirty = true;
    }

    pub(crate) fn color_scheme(&self) -> ColorScheme {
        self.read().color_scheme
    }

    pub(crate) fn color_scheme_mut(&self) -> SettingsColorSchemeMut<'_> {
        SettingsColorSchemeMut {
            settings: self.write(),
        }
    }

    /// A copy of the ignored paths, so that no lock is held while the user edits them.
    pub(crate) fn ignored_paths(&self) -> Vec<PathBuf> {
        self.read().scan_settings.ignored_path.clone()
    }

    pub(crate) fn set_ignored_paths(&self, paths: Vec<PathBuf>) {
        let mut settings = self.write();
        settings.scan_settings.ignored_path = paths;
        settings.dirty = true;
    }

    pub(crate) fn ignored_paths_mut(&self) -> SettingsIgnoredPathMut<'_> {
        SettingsIgnoredPathMut {
            settings: self.write(),
        }
    }

    pub(crate) fn ignore_cloud_mounts_mut(&self) -> SettingsIgnoreCloudMountsMut<'_> {
        SettingsIgnoreCloudMountsMut {
            settings: self.write(),
        }
    }

    pub(crate) fn save(&self) -> Result<(), std::io::Error> {
        self.read().save()
    }

    pub(crate) fn set_dirty(&self, dirty: bool) {
        self.write().dirty = dirty
    }

    pub(crate) fn theme(&self) -> ThemePreference {
        self.read().theme
    }

    pub(crate) fn set_theme(&self, theme: ThemePreference) {
        let mut settings = self.write();
        settings.theme = theme;
        settings.dirty = true;
    }

    pub(crate) fn init(&self, ctx: &egui::Context) {
        self.read().init(ctx);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InnerSettings {
    #[serde(skip)]
    /// Mark the Settings as dirty (need to be saved)
    dirty: bool,
    color_scheme: ColorScheme,
    theme: ThemePreference,
    #[serde(flatten)]
    scan_settings: ScanSettings,
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
                scan_settings: ScanSettings::default(),
            })
    }
}

static HOME: LazyLock<Option<PathBuf>> = LazyLock::new(home::home_dir);

impl InnerSettings {
    pub(crate) fn init(&self, ctx: &egui::Context) {
        ctx.set_theme(self.theme);
        self.color_scheme.apply(ctx);
    }

    pub(crate) fn is_path_ignored(&self, path: &Path) -> bool {
        self.scan_settings.is_explicitly_ignored(path)
            || (self.scan_settings.ignore_cloud_mounts && Self::is_common_cloud_path(path))
    }

    #[cfg(unix)]
    pub(crate) fn ignore_different_device(&self) -> bool {
        self.scan_settings.ignore_different_device
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
        // On macOS and Windows, many cloud folders are under HOME. We check the first component
        // after HOME against a small set of known names without allocating vectors.
        if let Some(home) = HOME.as_deref()
            && let Ok(stripped) = path.strip_prefix(home)
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
    ($name:ident, $($field:ident).+ => $target:ty) => {
        pub(crate) struct $name<'a> {
            settings: RwLockWriteGuard<'a, InnerSettings>,
        }

        impl Deref for $name<'_> {
            type Target = $target;

            fn deref(&self) -> &Self::Target {
                &self.settings.$($field).+
            }
        }

        impl DerefMut for $name<'_> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.settings.dirty = true;
                &mut self.settings.$($field).+
            }
        }
    };
}

settings_mut_guard!(SettingsColorSchemeMut, color_scheme => ColorScheme);
settings_mut_guard!(SettingsIgnoredPathMut, scan_settings.ignored_path => Vec<PathBuf>);
settings_mut_guard!(SettingsBigFileThresholdMut, scan_settings.big_file_threshold => u64);
settings_mut_guard!(SettingsIgnoreCloudMountsMut, scan_settings.ignore_cloud_mounts => bool);

#[cfg(test)]
mod tests {
    use super::*;

    fn inner() -> InnerSettings {
        InnerSettings {
            dirty: false,
            color_scheme: ColorScheme::Egui,
            theme: ThemePreference::System,
            scan_settings: ScanSettings::default(),
        }
    }

    fn settings() -> Settings {
        Settings {
            inner: Arc::new(RwLock::new(inner())),
        }
    }

    fn dirty(settings: &Settings) -> bool {
        settings.read().dirty
    }

    #[test]
    fn reading_does_not_set_dirty() {
        let s = settings();
        let _ = s.color_scheme();
        let _ = s.theme();
        let _ = s.big_file_threshold();
        assert!(!dirty(&s));
    }

    #[test]
    fn immutable_deref_of_guard_does_not_set_dirty() {
        let s = settings();
        {
            let guard = s.big_file_threshold_mut();
            assert_eq!(*guard, BIG_FILE_THRESHOLD);
        }
        assert!(!dirty(&s));
    }

    #[test]
    fn mutating_through_guard_sets_dirty() {
        let s = settings();
        *s.big_file_threshold_mut() = 42;
        assert_eq!(s.big_file_threshold(), 42);
        assert!(dirty(&s));
    }

    #[test]
    fn reset_big_file_threshold_restores_default_and_marks_dirty() {
        let s = settings();
        *s.big_file_threshold_mut() = 1;
        s.set_dirty(false);
        s.reset_big_file_threshold();
        assert_eq!(s.big_file_threshold(), BIG_FILE_THRESHOLD);
        assert!(dirty(&s));
    }

    #[test]
    fn set_theme_updates_and_marks_dirty() {
        let s = settings();
        s.set_theme(ThemePreference::Dark);
        assert_eq!(s.theme(), ThemePreference::Dark);
        assert!(dirty(&s));
    }

    #[test]
    fn color_scheme_mut_updates() {
        let s = settings();
        *s.color_scheme_mut() = ColorScheme::Solarized;
        assert_eq!(s.color_scheme(), ColorScheme::Solarized);
        assert!(dirty(&s));
    }

    #[test]
    fn set_dirty_false_clears_flag() {
        let s = settings();
        s.set_theme(ThemePreference::Light);
        s.set_dirty(false);
        assert!(!dirty(&s));
    }

    #[test]
    fn ignored_path_exact_match() {
        let s = settings();
        s.ignored_paths_mut().push(PathBuf::from("/a/b"));
        assert!(s.is_path_ignored(Path::new("/a/b")));
        assert!(!s.is_path_ignored(Path::new("/a")));
        assert!(!s.is_path_ignored(Path::new("/a/b/c")));
    }

    #[test]
    fn cloud_mounts_ignored_only_when_enabled() {
        let Some(home) = home::home_dir() else { return };
        let dropbox = home.join("Dropbox").join("file");
        let s = settings();
        assert!(s.is_path_ignored(&dropbox));
        *s.ignore_cloud_mounts_mut() = false;
        assert!(!s.is_path_ignored(&dropbox));
    }

    #[test]
    fn common_cloud_path_detection() {
        let Some(home) = home::home_dir() else { return };
        for name in ["Dropbox", "OneDrive", "Google Drive", "Box", "MEGA"] {
            assert!(
                InnerSettings::is_common_cloud_path(&home.join(name)),
                "{name}"
            );
        }
        assert!(!InnerSettings::is_common_cloud_path(
            &home.join("Documents")
        ));
        assert!(!InnerSettings::is_common_cloud_path(&home));
    }

    /// Scanning a disk mounted under these folders must not ignore its whole content.
    #[test]
    fn mount_folders_are_not_cloud_paths() {
        for path in [
            "/media/user/usb/photos",
            "/mnt/data/backup",
            "/run/user/1000/doc",
            "/snap/core/current",
        ] {
            assert!(
                !InnerSettings::is_common_cloud_path(Path::new(path)),
                "{path}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn icloud_path_detected_on_macos() {
        let Some(home) = home::home_dir() else { return };
        let p = home.join("Library/Mobile Documents/com~apple~CloudDocs/x");
        assert!(InnerSettings::is_common_cloud_path(&p));
        assert!(!InnerSettings::is_common_cloud_path(
            &home.join("Library/Other")
        ));
    }

    #[test]
    fn serde_roundtrip() {
        let mut i = inner();
        i.theme = ThemePreference::Dark;
        i.color_scheme = ColorScheme::Solarized;
        i.scan_settings.ignored_path.push(PathBuf::from("/z"));
        i.scan_settings.big_file_threshold = 123;
        i.dirty = true;
        let json = serde_json::to_string(&i).unwrap();
        let back: InnerSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.theme, ThemePreference::Dark);
        assert_eq!(back.color_scheme, ColorScheme::Solarized);
        assert_eq!(back.scan_settings.ignored_path, vec![PathBuf::from("/z")]);
        assert_eq!(back.scan_settings.big_file_threshold, 123);
        assert!(!back.dirty, "dirty is skipped by serde");
    }

    #[test]
    fn missing_ignore_cloud_mounts_defaults_to_true() {
        let json =
            r#"{"color_scheme":"Egui","theme":"System","ignored_path":[],"big_file_threshold":10}"#;
        let i: InnerSettings = serde_json::from_str(json).unwrap();
        assert!(i.scan_settings.ignore_cloud_mounts);
    }

    #[test]
    fn save_without_dirty_is_noop_ok() {
        assert!(inner().save().is_ok());
    }

    #[test]
    fn poisoned_lock_is_recovered() {
        let s = settings();
        let s2 = s.clone();
        let _ = std::thread::spawn(move || {
            let _guard = s2.inner.write().unwrap();
            panic!("poison");
        })
        .join();
        assert_eq!(s.theme(), ThemePreference::System);
        s.set_theme(ThemePreference::Dark);
        assert_eq!(s.theme(), ThemePreference::Dark);
    }

    #[test]
    fn theme_preference_converts_to_egui() {
        assert_eq!(
            egui::ThemePreference::from(ThemePreference::Dark),
            egui::ThemePreference::Dark
        );
        assert_eq!(
            egui::ThemePreference::from(ThemePreference::Light),
            egui::ThemePreference::Light
        );
        assert_eq!(
            egui::ThemePreference::from(ThemePreference::System),
            egui::ThemePreference::System
        );
    }
}
