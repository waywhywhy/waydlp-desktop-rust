//! Embed the icon and version information into the Windows executable.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icons/waydlp.ico")
            .set("ProductName", "waydlp")
            .set("FileDescription", "waydlp media downloader")
            .set("LegalCopyright", "waydlp contributors, GPL-3.0-or-later");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows resources: {e}");
        }
    }
    println!("cargo:rerun-if-changed=../../assets/icons/waydlp.ico");
}
