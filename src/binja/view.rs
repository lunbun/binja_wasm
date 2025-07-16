use crate::binja::func_parse::parse_func;
use crate::binja::module_data::{FunctionData, MODULE_DATA, ModuleData};
use crate::util::arc_identity::ArcIdentity;
use crate::util::bin_util::BinaryReadable;
use binaryninja::Endianness;
use binaryninja::architecture::{ArchitectureExt, CoreArchitecture};
use binaryninja::binary_view::{BinaryView, BinaryViewBase, BinaryViewExt};
use binaryninja::custom_binary_view::CustomBinaryView;
use binaryninja::interaction::{MessageBoxButtonSet, MessageBoxIcon, show_message_box};
use binaryninja::section::{SectionBuilder, Semantics};
use binaryninja::segment::{SegmentBuilder, SegmentFlags};
use binaryninja::symbol::{Symbol, SymbolType};
use log::{error, info, warn};
use std::cmp::min;
use std::collections::BTreeMap;
use std::ops::Range;
use std::pin::Pin;
use std::sync::Mutex;
use wasmparser::{BinaryReader, Chunk, CustomSectionReader, DataSectionReader, ElementSectionReader, ExportSectionReader, ExternalKind, FunctionBody, FunctionSectionReader, GlobalSectionReader, ImportSectionReader, MemorySectionReader, Operator, OperatorsReader, Parser, Payload, TableSectionReader, TypeRef, TypeSectionReader};

fn range_usize_to_u64(range: Range<usize>) -> Range<u64> {
    (range.start as u64)..(range.end as u64)
}

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

impl WebAssemblyView {
    fn add_wasm_section(
        &mut self,
        range: Range<usize>,
        name: String,
        segment_cb: impl FnOnce(SegmentBuilder) -> SegmentBuilder,
        section_cb: impl FnOnce(SectionBuilder) -> SectionBuilder,
    ) {
        let range = range_usize_to_u64(range);
        let segment_builder = SegmentBuilder::new(range.clone())
            .parent_backing(range.clone())
            .is_auto(true);
        self.add_segment(segment_cb(segment_builder));
        let section_builder = SectionBuilder::new(name, range.clone()).is_auto(true);
        self.add_section(section_cb(section_builder));
    }

    fn handle_custom_section(&mut self, reader: CustomSectionReader) {
        self.add_wasm_section(
            reader.range(),
            format!(".custom.{}", reader.name()),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_type_section(&mut self, reader: TypeSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".type".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_import_section(&mut self, reader: ImportSectionReader, func_index: &mut u32) -> Result<(), ()> {
        self.add_wasm_section(
            reader.range(),
            ".import".to_string(),
            std::convert::identity,
            std::convert::identity,
        );

        for import in reader {
            let import = import.map_err(|_| ())?;
            if matches!(import.ty, TypeRef::Func(_)) {
                *func_index += 1;
            }
        }
        Ok(())
    }

    fn handle_function_section(&mut self, reader: FunctionSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".function".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_table_section(&mut self, reader: TableSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".table".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_memory_section(&mut self, reader: MemorySectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".memory".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_global_section(&mut self, reader: GlobalSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".global".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_export_section(
        &mut self,
        reader: ExportSectionReader,
        func_exports: &mut BTreeMap<u32, String>,
    ) {
        self.add_wasm_section(
            reader.range(),
            ".export".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
        for export in reader {
            if let Ok(export) = export {
                if export.kind == ExternalKind::Func {
                    func_exports.insert(export.index, export.name.to_string());
                }
            }
        }
    }

    fn handle_element_section(&mut self, reader: ElementSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".element".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_data_section(&mut self, reader: DataSectionReader) {
        self.add_wasm_section(
            reader.range(),
            ".data".to_string(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_code_section_start(&mut self, _count: u32, range: Range<usize>, _size: u32) {
        self.add_wasm_section(
            range,
            ".code".to_string(),
            |sb| {
                sb.flags(
                    SegmentFlags::new()
                        .contains_data(false)
                        .contains_code(true)
                        .readable(true)
                        .writable(false)
                        .executable(true)
                        .deny_write(true)
                        .deny_execute(false),
                )
            },
            |sb| sb.semantics(Semantics::ReadOnlyCode),
        );
    }

    fn handle_code_section_entry(
        &mut self,
        view: &BinaryView,
        module_data: &mut ModuleData,
        size_start: u64,
        locals_start: u64,
        end: u64,
        func_exports: &BTreeMap<u32, String>,
        func_index: u32,
    ) -> Result<(), ()> {
        // Sanity check that the address is within a code segment; if we try to
        // add a function in a segment that is not a code segment, binja will crash.
        let segment = self.segment_at(size_start);
        if segment.is_none() || !segment.unwrap().contains_code() {
            warn!("Function at address {size_start:#x} is not in a code segment");
            return Err(());
        }

        // SAFETY: `raw` will be filled with the function body bytes, and it is
        // checked that the read operation fills the entire buffer.
        let mut raw = Pin::new(unsafe {
            Box::new_uninit_slice((end - locals_start) as usize).assume_init()
        });
        let n_read = view.read(&mut raw, locals_start);
        if n_read != raw.len() {
            warn!(
                "Failed to read function at address {size_start:#x}: expected {} bytes, got {n_read}",
                raw.len()
            );
            return Err(());
        }

        module_data.funcs.insert(
            size_start..end,
            ArcIdentity::new(parse_func(size_start, locals_start, end, raw).map_err(|_| ())?),
        );
        self.add_auto_function(&self.default_platform().unwrap(), size_start)
            .ok_or(())?;

        if let Some(name) = func_exports.get(&func_index) {
            let symbol = Symbol::builder(SymbolType::Function, name.as_str(), size_start).create();
            self.define_auto_symbol(&symbol);
        }
        Ok(())
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
        let parent = self.parent_view().ok_or(())?;

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

        {
            const BUF_SIZE: usize = 1024;
            let mut buf = Vec::new();
            let mut i = 0u64;
            let mut eof = false;

            let mut parser = Parser::new(0);
            let mut func_exports = BTreeMap::new();
            let mut func_index = 0u32;
            loop {
                let (payload, consumed) = match parser.parse(&buf, eof).map_err(|_| ())? {
                    Chunk::NeedMoreData(hint) => {
                        assert!(!eof);
                        let n_read =
                            parent.read_into_vec(&mut buf, i, min(hint as usize, BUF_SIZE));
                        i += n_read as u64;
                        eof = n_read == 0;
                        continue;
                    }
                    Chunk::Parsed { consumed, payload } => (payload, consumed),
                };

                if let Payload::CodeSectionStart { count, range, size } = payload {
                    // Parse the code section ourselves since we don't actually use the
                    // result of the `wasmparser` code section parser.
                    self.handle_code_section_start(count, range.clone(), size);
                    parser.skip_section();

                    let mut addr = range.start as u64;
                    let (count_2, n_bytes) = parent.read_u32_leb128(addr)?;
                    assert_eq!(count, count_2);
                    addr += n_bytes as u64;

                    for _ in 0..count {
                        let size_start = addr;
                        let (size, n_bytes) = parent.read_u32_leb128(addr)?;
                        addr += n_bytes as u64;
                        let locals_start = addr;
                        addr += size as u64;
                        let end = addr;

                        info!(
                            "Found function at {size_start:#x} with locals at {locals_start:#x} and end at {end:#x}",
                            size_start = size_start,
                            locals_start = locals_start,
                            end = end
                        );

                        self.handle_code_section_entry(
                            &parent,
                            module_data,
                            size_start,
                            locals_start,
                            end,
                            &func_exports,
                            func_index,
                        )?;

                        module_data.func_addrs.push(size_start);
                        func_index += 1;
                    }

                    if addr != range.end as u64 {
                        warn!(
                            "Code section start address {addr:#x} does not match range end {}",
                            range.end
                        );
                        return Err(());
                    }

                    i = range.end as u64;
                    buf.clear();
                } else {
                    match payload {
                        Payload::CustomSection(reader) => self.handle_custom_section(reader),
                        Payload::TypeSection(reader) => self.handle_type_section(reader),
                        Payload::ImportSection(reader) => {
                            self.handle_import_section(reader, &mut func_index)?
                        },
                        Payload::FunctionSection(reader) => self.handle_function_section(reader),
                        Payload::TableSection(reader) => self.handle_table_section(reader),
                        Payload::MemorySection(reader) => self.handle_memory_section(reader),
                        Payload::GlobalSection(reader) => self.handle_global_section(reader),
                        Payload::ExportSection(reader) => {
                            self.handle_export_section(reader, &mut func_exports)
                        }
                        Payload::ElementSection(reader) => self.handle_element_section(reader),
                        Payload::DataSection(reader) => self.handle_data_section(reader),

                        Payload::End(_) => break,
                        _ => {
                            info!("Parsing WebAssembly payload: {payload:?}");
                        }
                    }

                    buf.drain(..consumed);
                }
            }
        }

        Ok(())
    }
}
