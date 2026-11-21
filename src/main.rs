#![windows_subsystem = "windows"]
//! Halit Changer: League of Legends skin browser that hands picks to LTK Manager.

mod app;
mod data;
mod i18n;
mod images;
mod ltk;

const TITLE: &str = "Halit Changer";

/// Dark Win11 title bar matching the app palette. Returns true once applied.
pub fn dark_titlebar() -> bool {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return false;
        }
        let set = |attr: u32, val: u32| {
            DwmSetWindowAttribute(hwnd, attr, &val as *const u32 as *const _, 4);
        };
        set(20, 1); // DWMWA_USE_IMMERSIVE_DARK_MODE
        set(35, 0x00100A10); // caption colour (BGR) ~ #101018 panel
        set(36, 0x00F8F0F2); // caption text
        set(34, 0x0068223B); // border
    }
    true
}

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../assets/logo.png"))
        .ok()
        .map(|i| {
            let i = i.resize(64, 64, image::imageops::FilterType::Lanczos3).to_rgba8();
            egui::IconData { width: i.width(), height: i.height(), rgba: i.into_raw() }
        });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_inner_size([1380.0, 860.0])
        .with_min_inner_size([980.0, 640.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(TITLE, options, Box::new(|cc| Ok(Box::new(app::HalitApp::new(cc)))))
}
