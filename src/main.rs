use gpui::*;
use gpui_component::*;

mod app;
mod components;
mod git;
use app::{
    App, CloseWindow, NewRequest, OpenFolder, SaveRequest, SendRequest, ToggleSidebar, ToggleTheme,
};

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    // Initialize Tokio runtime for reqwest
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _enter = runtime.enter();

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);

        // Bind keyboard shortcuts (platform-adaptive: cmd for macOS, ctrl for Windows/Linux)
        cx.bind_keys([
            // Send request: Cmd/Ctrl + Enter
            KeyBinding::new("cmd-enter", SendRequest, Some("ApiClient")),
            KeyBinding::new("ctrl-enter", SendRequest, Some("ApiClient")),
            // Save request: Cmd/Ctrl + S
            KeyBinding::new("cmd-s", SaveRequest, Some("ApiClient")),
            KeyBinding::new("ctrl-s", SaveRequest, Some("ApiClient")),
            // New request: Cmd/Ctrl + N
            KeyBinding::new("cmd-n", NewRequest, Some("ApiClient")),
            KeyBinding::new("ctrl-n", NewRequest, Some("ApiClient")),
            // Open folder: Cmd/Ctrl + O
            KeyBinding::new("cmd-o", OpenFolder, Some("ApiClient")),
            KeyBinding::new("ctrl-o", OpenFolder, Some("ApiClient")),
            // Toggle sidebar: Cmd/Ctrl + B
            KeyBinding::new("cmd-b", ToggleSidebar, Some("ApiClient")),
            KeyBinding::new("ctrl-b", ToggleSidebar, Some("ApiClient")),
            // Toggle theme: Cmd/Ctrl + Shift + T
            KeyBinding::new("cmd-shift-t", ToggleTheme, Some("ApiClient")),
            KeyBinding::new("ctrl-shift-t", ToggleTheme, Some("ApiClient")),
            // Close window: Cmd/Ctrl + W
            KeyBinding::new("cmd-w", CloseWindow, Some("ApiClient")),
            KeyBinding::new("ctrl-w", CloseWindow, Some("ApiClient")),
        ]);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(8.0), px(8.0))),
                }),
                focus: true,
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| App::new(window, cx));
                // This first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
