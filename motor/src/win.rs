//! Llamadas Win32 sueltas.

use anyhow::Result;
use std::path::{Path, PathBuf};
use windows::core::HSTRING;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SystemParametersInfoW, HWND_BROADCAST, SMTO_ABORTIFHUNG,
    SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_SETTINGCHANGE,
};

/// Avisa a todas las ventanas que cambió una configuración (p. ej. "ImmersiveColorSet").
pub fn avisar_cambio(area: &str) {
    let h = HSTRING::from(area);
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(h.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
}

pub fn poner_fondo(ruta: &Path) -> Result<()> {
    // SPI_SETDESKWALLPAPER exige una ruta absoluta.
    let h = HSTRING::from(std::path::absolute(ruta)?.as_path());
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(h.as_ptr() as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )?;
    }
    Ok(())
}

pub fn fondo_actual() -> Result<PathBuf> {
    let mut buf = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Inicializa WinRT en el hilo actual (necesario antes de usar APIs WinRT desde hilos propios).
/// Si ya estaba inicializado, no pasa nada.
pub fn iniciar_winrt() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}
