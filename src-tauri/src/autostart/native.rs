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
    tauri_plugin_autostart::Builder::new()
        .args([HIDDEN_ARG])
        .macos_launcher(MacosLauncher::LaunchAgent)
        .build()
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
