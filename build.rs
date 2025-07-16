fn main() {
    let link_path =
        std::env::var_os("DEP_BINARYNINJACORE_PATH").expect("DEP_BINARYNINJACORE_PATH not specified");

    println!(r"cargo:rustc-link-lib=binaryninjacore");
    println!(r"cargo:rustc-link-search={}", link_path.to_str().unwrap());

    #[cfg(not(target_os = "windows"))]
    {
        println!(
            "cargo::rustc-link-arg=-Wl,-rpath,{0},-L{0}",
            link_path.to_string_lossy()
        );
    }
}
