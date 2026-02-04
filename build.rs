fn main() {
    // Only compile Windows resources on Windows
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/par-term.ico");
        res.set("ProductName", "par-term");
        res.set("FileDescription", "GPU-accelerated terminal emulator");
        res.set("LegalCopyright", "Copyright (c) Paul Robello");
        res.compile().expect("Failed to compile Windows resources");
    }
}
