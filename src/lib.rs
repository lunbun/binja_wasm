mod binja;
mod util;

use crate::binja::view_type::WebAssemblyViewType;
use binaryninja::architecture::register_architecture;
use binaryninja::custom_binary_view::register_view_type;
use binaryninja::logger::Logger;
use binja::arch::WebAssemblyArchitecture;
use log::LevelFilter;

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn CorePluginInit() -> bool {
    Logger::new("WebAssembly Plugin")
        .with_level(LevelFilter::Trace)
        .init();
    register_architecture("wasm", WebAssemblyArchitecture::new);
    register_view_type("wasm", "WebAssembly", WebAssemblyViewType::new);
    true
}
