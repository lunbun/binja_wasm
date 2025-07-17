use crate::binja::parse::module_data::{ModuleData, MODULE_DATA};
use binaryninja::architecture::{ArchitectureExt, CoreArchitecture};
use binaryninja::binary_view::{BinaryView, BinaryViewBase, BinaryViewExt};
use binaryninja::custom_binary_view::CustomBinaryView;
use binaryninja::interaction::{show_message_box, MessageBoxButtonSet, MessageBoxIcon};
use binaryninja::Endianness;
use log::error;
use std::sync::Mutex;

pub struct WebAssemblyView {
    handle: binaryninja::rc::Ref<BinaryView>,
}

impl AsRef<BinaryView> for WebAssemblyView {
    fn as_ref(&self) -> &BinaryView {
        &self.handle
    }
}

impl BinaryViewBase for WebAssemblyView {
    fn entry_point(&self) -> u64 {
        0
    }

    fn default_endianness(&self) -> Endianness {
        Endianness::LittleEndian
    }

    fn address_size(&self) -> usize {
        4
    }
}

static SHOULD_PARSE: Mutex<bool> = Mutex::new(false);

unsafe impl CustomBinaryView for WebAssemblyView {
    type Args = ();

    fn new(handle: &BinaryView, _args: &Self::Args) -> binaryninja::binary_view::Result<Self> {
        Ok(Self {
            handle: handle.to_owned(),
        })
    }

    fn init(&mut self, _args: Self::Args) -> binaryninja::binary_view::Result<()> {
        let arch = CoreArchitecture::by_name("wasm").ok_or(())?;
        let platform = arch.standalone_platform().ok_or(())?;

        self.set_default_arch(&arch);
        self.set_default_platform(&platform);

        // For some reason, binja will ask us to create a BinaryView twice...
        // but it only expects the second one to actually parse the file.
        let mut should_parse = SHOULD_PARSE.lock().unwrap();
        if !*should_parse {
            *should_parse = true;
            return Ok(());
        }

        let mut module_data_lock = MODULE_DATA.lock().unwrap();
        if module_data_lock.is_some() {
            const ERROR_MSG: &str = concat!(
                "Unfortunately, due to limitations of the Binary Ninja API, ",
                "it is not possible to open multiple WebAssembly files. Please ",
                "restart Binary Ninja to open a new WebAssembly file."
            );
            error!("{ERROR_MSG}");
            show_message_box(
                "WebAssembly Error",
                ERROR_MSG,
                MessageBoxButtonSet::OKButtonSet,
                MessageBoxIcon::ErrorIcon,
            );
            return Err(());
        }
        *module_data_lock = Some(ModuleData::new());
        let module_data = module_data_lock.as_mut().unwrap();
        self.parse_module(module_data)?;

        Ok(())
    }
}
