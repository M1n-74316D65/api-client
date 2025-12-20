fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        // Point to the .ico file that will be generated in the CI pipeline
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
}
