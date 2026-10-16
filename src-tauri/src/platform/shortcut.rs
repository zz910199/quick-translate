use crate::commands::translation::AppErrorDto;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub trait GlobalShortcut: Send + Sync {
    fn register(
        &self,
        accelerator: &str,
        callback: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), AppErrorDto>;
    fn unregister(&self, accelerator: &str) -> Result<(), AppErrorDto>;
}
#[derive(Default)]
struct Registration {
    current: Option<String>,
    error: Option<AppErrorDto>,
}
pub struct ShortcutController {
    backend: Arc<dyn GlobalShortcut>,
    callback: Arc<dyn Fn() + Send + Sync>,
    registration: Mutex<Registration>,
}
impl ShortcutController {
    pub fn new(backend: Arc<dyn GlobalShortcut>, callback: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            backend,
            callback,
            registration: Mutex::new(Registration::default()),
        }
    }
    pub fn error(&self) -> Option<AppErrorDto> {
        self.registration.lock().ok().and_then(|s| s.error.clone())
    }
    pub fn apply(
        &self,
        accelerator: &str,
        persist: impl FnOnce() -> Result<(), AppErrorDto>,
    ) -> Result<(), AppErrorDto> {
        let mut registration = self.registration.lock().map_err(|_| shortcut_error())?;
        if registration.current.as_deref() == Some(accelerator) {
            persist()?;
            registration.error = None;
            return Ok(());
        }
        if let Err(error) = self.backend.register(accelerator, self.callback.clone()) {
            registration.error = Some(error.clone());
            return Err(error);
        }
        if let Some(old) = &registration.current {
            if let Err(error) = self.backend.unregister(old) {
                let _ = self.backend.unregister(accelerator);
                registration.error = Some(error.clone());
                return Err(error);
            }
        }
        if let Err(error) = persist() {
            let _ = self.backend.unregister(accelerator);
            if let Some(old) = &registration.current {
                if let Err(restore_error) = self.backend.register(old, self.callback.clone()) {
                    registration.current = None;
                    registration.error = Some(restore_error);
                }
            }
            return Err(error);
        }
        registration.current = Some(accelerator.into());
        registration.error = None;
        Ok(())
    }
}
fn shortcut_error() -> AppErrorDto {
    AppErrorDto::new(
        "shortcut_conflict",
        "无法注册快捷键：组合无效或已被占用，请在设置中更换。",
        false,
    )
}
pub struct SystemGlobalShortcut(pub AppHandle);
impl GlobalShortcut for SystemGlobalShortcut {
    fn register(
        &self,
        accelerator: &str,
        callback: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), AppErrorDto> {
        self.0
            .global_shortcut()
            .on_shortcut(accelerator, move |_, _, event| {
                if event.state == ShortcutState::Pressed {
                    callback();
                }
            })
            .map_err(|_| shortcut_error())
    }
    fn unregister(&self, accelerator: &str) -> Result<(), AppErrorDto> {
        self.0
            .global_shortcut()
            .unregister(accelerator)
            .map_err(|_| shortcut_error())
    }
}
