#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod activity_ui;
mod app;
mod auto_backup;
mod auto_backup_ui;
mod auto_lock;
mod auto_lock_ui;
mod backup;
mod backup_ui;
mod catalog_ui;
mod discount_ui;
mod history_ui;
mod home_ui;
mod i18n;
mod import_ui;
mod kitchen_ui;
#[cfg(debug_assertions)]
mod preview;
mod print;
mod print_ui;
mod promo_ui;
mod report_ui;
mod sale_ui;
mod settings_ui;
mod shell;
mod shift_ui;
mod shop;
mod staff_ui;
mod tables_ui;
mod tax_ui;
mod templates_ui;
mod ui;
mod window;
mod worker;

use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut demo = false;
    let mut explicit = None;
    let mut screenshot = None;
    let mut window_size = [1280.0, 820.0];
    let mut page = None;
    #[allow(unused_mut)]
    let mut preview: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => demo = true,
            "--data-dir" => {
                explicit = Some(PathBuf::from(
                    args.next().ok_or("--data-dir requires a path")?,
                ))
            }
            "--screenshot" => {
                screenshot = Some(PathBuf::from(
                    args.next().ok_or("--screenshot requires a path")?,
                ))
            }
            "--window-size" => {
                let value = args.next().ok_or("--window-size requires WIDTHxHEIGHT")?;
                let (width, height) = value.split_once('x').ok_or("Use WIDTHxHEIGHT")?;
                let (width, height) = (width.parse::<u16>()?, height.parse::<u16>()?);
                if width < 1000 || height < 700 {
                    return Err("Minimum window size is 1000x700".into());
                }
                window_size = [f32::from(width), f32::from(height)];
            }
            #[cfg(debug_assertions)]
            "--preview" => {
                preview = Some(args.next().ok_or("--preview requires a name")?);
            }
            "--page" => {
                let name = args.next().ok_or("--page requires a workspace name")?;
                page = Some(app::Page::from_name(&name).ok_or("Unknown page")?);
            }
            "--help" => {
                println!(
                    "pos-heart [--demo] [--data-dir PATH] [--screenshot PNG] [--window-size WIDTHxHEIGHT] [--page home|sell|history|shift|products|reports|settings]\nDemo uses separate data. No network is required."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let dir = explicit
        .or_else(|| {
            directories::ProjectDirs::from("com", "pos-heart", "POS Heart")
                .map(|p| p.data_local_dir().to_path_buf())
        })
        .ok_or("Cannot locate app data directory")?;
    let dir = if demo { dir.join("demo") } else { dir };
    backup::activate_pending(&dir)?;
    std::fs::create_dir_all(&dir)?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(if demo {
                "POS Heart — Demo"
            } else {
                "POS Heart"
            })
            .with_inner_size(window_size)
            .with_min_inner_size(window::MIN_SIZE),
        ..Default::default()
    };
    eframe::run_native(
        "POS Heart",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc, dir, demo, screenshot, page)?;
            app.preview = preview;
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}
