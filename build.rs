fn main() {
    if cfg!(target_os = "windows") {
        // FFmpeg's DirectShow and Media Foundation backends reference GUIDs
        // supplied by these Windows SDK import libraries.
        println!("cargo:rustc-link-lib=strmiids");
        println!("cargo:rustc-link-lib=mfuuid");

        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.compile().unwrap();
    }
}
