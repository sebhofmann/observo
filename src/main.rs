use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use log::warn;

use observo::app::ObservoApp;
use observo::settings::Settings;

fn window_icon() -> Option<Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../resources/observo.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(Arc::new(image))
}

fn print_help() {
    println!(
        "Observo {}\n\n\
         A native Zabbix problem monitor.\n\n\
         Usage: observo [--config PATH]\n\n\
         Options:\n\
         \x20 --config PATH   Use a specific settings file\n\
         \x20 --version, -V   Print the version\n\
         \x20 --help, -h      Print this help\n\n\
         Settings default to $XDG_CONFIG_HOME/observo/observo.properties.",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,observo=info"),
    )
    .init();

    let mut config_override = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--config" => match args.next() {
                Some(path) => config_override = Some(std::path::PathBuf::from(path)),
                None => {
                    eprintln!("error: --config requires a path");
                    std::process::exit(2);
                }
            },
            "--version" | "-V" => {
                println!("observo {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("error: unknown argument '{other}' (try --help)");
                std::process::exit(2);
            }
        }
    }

    let (settings, warnings) = match &config_override {
        Some(path) => Settings::load_from(path),
        None => Settings::load(),
    };
    for warning in warnings {
        warn!("settings: {warning}");
    }

    let width = settings.window.width as f32;
    let height = settings.window.height as f32;
    let position = settings
        .window
        .x
        .zip(settings.window.y)
        .map(|(x, y)| (x as f32, y as f32));

    let start_maximized = settings.window.maximized;
    let app_settings = settings.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            observo::app::register_key_bindings(cx);

            let restore_size = size(px(width), px(height));
            let mut bounds = if let Some((x, y)) = position {
                Bounds::new(point(px(x), px(y)), restore_size)
            } else {
                Bounds::centered(None, restore_size, cx)
            };
            // A monitor may have been disconnected since the last run. Fall
            // back to the primary screen when the saved rectangle is entirely
            // off-screen, but keep partially visible windows where they are.
            if !cx
                .displays()
                .iter()
                .any(|display| display.bounds().intersects(&bounds))
            {
                bounds = Bounds::centered(None, restore_size, cx);
            }

            let window_bounds = if start_maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            };

            let options = WindowOptions {
                window_bounds: Some(window_bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some("Observo".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(1000.), px(600.))),
                app_id: Some("de.paschty.observo".to_string()),
                icon: window_icon(),
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, move |window, cx| {
                let app = cx.new(|cx| ObservoApp::new(app_settings, window, cx));
                #[cfg(debug_assertions)]
                if let Ok(which) = std::env::var("OBSERVO_DEBUG_DIALOG") {
                    let app = app.clone();
                    let handle = window.window_handle();
                    cx.spawn(async move |cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(2_500))
                            .await;
                        cx.update(|cx| {
                            handle
                                .update(cx, |_, window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.debug_open_dialog(&which, window, cx);
                                    });
                                })
                                .ok();
                        });
                    })
                    .detach();
                }
                app
            })
            .expect("failed to open Observo window");
        });
}
