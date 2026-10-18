//! On Windows, builds the icon and version information into whiplash.exe:
//! Windows shows them in Explorer, the taskbar, Task Manager and the media
//! panel, which names the app by its "FileDescription".

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=packaging/windows/whiplash.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        // Resource 1, which the windows load as their icon (src/app/mod.rs).
        resource.set_icon_with_id("packaging/windows/whiplash.ico", "1");
        resource.set("FileDescription", "Whiplash Player");
        resource.set("ProductName", "Whiplash Player");
        resource.set("LegalCopyright", "Luke Hindman. GNU GPL 3.0 or later.");
        resource.compile().expect("building the Windows resources");
    }
}
