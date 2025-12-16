use gpui::*;
use gpui_component::*;

mod app;
use app::App;

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    // Initialize Tokio runtime for reqwest
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _enter = runtime.enter();

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| App::new(window, cx));
                // This first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
