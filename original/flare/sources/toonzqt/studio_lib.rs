#![forbid(unsafe_code)]

pub mod app;
pub mod commands;
pub mod document;
pub mod i18n;
pub mod icon;
pub mod theme;
pub mod widgets;

pub use app::Studio;

/// Match native and offscreen renderer enumeration. Enumerating unused Windows
/// backends has hung startup and faulted on teardown on the development machine.
/// The override is process-local; no system graphics settings are changed.
pub fn configure_renderer(setup: &mut eframe::egui_wgpu::WgpuSetup) {
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(config) = setup {
        config.instance_descriptor.backends =
            eframe::wgpu::Backends::from_env().unwrap_or_else(|| {
                if cfg!(target_os = "windows") {
                    eframe::wgpu::Backends::VULKAN
                } else {
                    config.instance_descriptor.backends
                }
            });
    }
}
