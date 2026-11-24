//! The system's login registration, through the autostart plugin. The only
//! code that touches the plugin.
//!
//! macOS: a launch agent file in `~/Library/LaunchAgents`. "In force" is
//! whether that file exists; an item switched off in System Settings →
//! Login Items cannot be seen from here. Windows: the `Run` registry key;
//! an entry disabled in Task Manager is reported as not in force, and
//! making a registration clears that mark, which is why `reconcile`
//! refreshes only an entry that is in force.

use std::sync::Arc;

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::{AutoLaunchManager, MacosLauncher};

use super::{HIDDEN_ARG, Inert, LoginItem};

/// The plugin, set to pass `HIDDEN_ARG` to the app at login.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    // `init`, not the builder: the builder's launcher setting exists on
    // macOS only, and this must compile everywhere.
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG]))
}

/// Whether this build registers itself. A development build never does.
pub fn registers() -> bool {
    !tauri::is_dev()
}

/// The registration this build uses: the system's, or one that does
/// nothing in a development build or if the plugin did not start.
pub fn login_item(app: &AppHandle) -> Arc<dyn LoginItem> {
    if registers() && app.try_state::<AutoLaunchManager>().is_some() {
        Arc::new(SystemLoginItem(app.clone()))
    } else {
        Arc::new(Inert)
    }
}

/// The registration an earlier name of the app left, so that it can be
/// read and removed (see `handover`). The plugin reaches only the present
/// name, so this one is built directly on the library the plugin uses,
/// which names the launch agent file and the `Run` value after the app.
/// It does nothing in a development build, or if it cannot be built.
pub fn old_login_item(name: &str) -> Arc<dyn LoginItem> {
    if !registers() {
        return Arc::new(Inert);
    }
    // The path is what `enable` would register, and `enable` is never
    // asked of an earlier name; it only has to be there.
    let path = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name(name).set_app_path(&path);
    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    match builder.build() {
        Ok(launch) => Arc::new(OldLoginItem {
            launch,
            #[cfg(windows)]
            name: name.to_string(),
        }),
        Err(_) => Arc::new(Inert),
    }
}

struct OldLoginItem {
    launch: auto_launch::AutoLaunch,
    #[cfg(windows)]
    name: String,
}

impl LoginItem for OldLoginItem {
    /// Never made again: an earlier name is only ever taken out.
    fn enable(&self) -> Result<(), String> {
        Err("a registration under an old name is not made".to_string())
    }

    fn disable(&self) -> Result<(), String> {
        self.launch.disable().map_err(|error| error.to_string())?;
        #[cfg(windows)]
        remove_startup_approved(&self.name)?;
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, String> {
        self.launch.is_enabled().map_err(|error| error.to_string())
    }
}

/// Removes the mark Windows keeps, under the same name, for an entry the
/// user disabled in Task Manager. The library's removal leaves it behind.
/// One that is not there is not an error.
#[cfg(windows)]
fn remove_startup_approved(name: &str) -> Result<(), String> {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    // ERROR_FILE_NOT_FOUND as an HRESULT: no such key, or no such value.
    const NOT_FOUND: i32 = 0x8007_0002_u32 as i32;
    match windows_registry::CURRENT_USER
        .options()
        .write()
        .open(KEY)
        .and_then(|key| key.remove_value(name))
    {
        Ok(()) => Ok(()),
        Err(error) if error.code().0 == NOT_FOUND => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

struct SystemLoginItem(AppHandle);

impl SystemLoginItem {
    fn with<T>(
        &self,
        ask: impl FnOnce(&AutoLaunchManager) -> Result<T, tauri_plugin_autostart::Error>,
    ) -> Result<T, String> {
        let manager = self
            .0
            .try_state::<AutoLaunchManager>()
            .ok_or_else(|| "the login item service is not running".to_string())?;
        ask(&manager).map_err(|error| error.to_string())
    }
}

impl LoginItem for SystemLoginItem {
    fn enable(&self) -> Result<(), String> {
        self.with(AutoLaunchManager::enable)
    }

    fn disable(&self) -> Result<(), String> {
        self.with(AutoLaunchManager::disable)
    }

    fn is_enabled(&self) -> Result<bool, String> {
        self.with(AutoLaunchManager::is_enabled)
    }
}
