use crate::binja::parse::func_parse::parse_func;
use crate::binja::parse::module_data::ModuleData;
use crate::binja::view::WebAssemblyView;
use crate::util::arc_identity::ArcIdentity;
use crate::util::bin_util::BinaryReadable;
use binaryninja::binary_view::{BinaryView, BinaryViewBase, BinaryViewExt};
use binaryninja::section::{SectionBuilder, Semantics};
use binaryninja::segment::{SegmentBuilder, SegmentFlags};
use binaryninja::symbol::{Symbol, SymbolType};
use log::{info, warn};
use std::cmp::min;
use std::collections::BTreeMap;
use std::ops::Range;
use std::pin::Pin;
use wasmparser::{
    Chunk, ExportSectionReader,
    ExternalKind, ImportSectionReader, Parser, Payload, TypeRef,
};

impl WebAssemblyView {
    fn add_wasm_section(
        &mut self,
        range: Range<usize>,
        name: String,
        segment_cb: impl FnOnce(SegmentBuilder) -> SegmentBuilder,
        section_cb: impl FnOnce(SectionBuilder) -> SectionBuilder,
    ) {
        let range = (range.start as u64)..(range.end as u64);
        let segment_builder = SegmentBuilder::new(range.clone())
            .parent_backing(range.clone())
            .is_auto(true);
        self.add_segment(segment_cb(segment_builder));
        let section_builder = SectionBuilder::new(name, range.clone()).is_auto(true);
        self.add_section(section_cb(section_builder));
    }

    fn add_wasm_section_default(&mut self, range: Range<usize>, name: impl Into<String>) {
        self.add_wasm_section(
            range,
            name.into(),
            std::convert::identity,
            std::convert::identity,
        );
    }

    fn handle_import_section(
        &mut self,
        reader: ImportSectionReader,
        func_index: &mut u32,
    ) -> Result<(), ()> {
        self.add_wasm_section_default(reader.range(), ".import");
        for import in reader {
            let import = import.map_err(|_| ())?;
            if matches!(import.ty, TypeRef::Func(_)) {
                *func_index += 1;
            }
        }
        Ok(())
    }

    fn handle_export_section(
        &mut self,
        reader: ExportSectionReader,
        func_exports: &mut BTreeMap<u32, String>,
    ) {
        self.add_wasm_section_default(reader.range(), ".export");
        for export in reader {
            if let Ok(export) = export {
                if export.kind == ExternalKind::Func {
                    func_exports.insert(export.index, export.name.to_string());
                }
            }
        }
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
        let mut raw =
            Pin::new(unsafe { Box::new_uninit_slice((end - locals_start) as usize).assume_init() });
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

    pub(crate) fn parse_module(&mut self, module_data: &mut ModuleData) -> Result<(), ()> {
        let parent = self.parent_view().ok_or(())?;

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
                    let n_read = parent.read_into_vec(&mut buf, i, min(hint as usize, BUF_SIZE));
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
                    Payload::CustomSection(reader) => self.add_wasm_section_default(
                        reader.range(),
                        format!(".custom.{}", reader.name()),
                    ),
                    Payload::TypeSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".type")
                    }
                    Payload::ImportSection(reader) => {
                        self.handle_import_section(reader, &mut func_index)?
                    }
                    Payload::FunctionSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".function")
                    }
                    Payload::TableSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".table")
                    }
                    Payload::MemorySection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".memory")
                    }
                    Payload::GlobalSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".global")
                    }
                    Payload::ExportSection(reader) => {
                        self.handle_export_section(reader, &mut func_exports)
                    }
                    Payload::ElementSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".element")
                    }
                    Payload::DataSection(reader) => {
                        self.add_wasm_section_default(reader.range(), ".data")
                    }

                    Payload::End(_) => break,
                    _ => {
                        info!("Parsing WebAssembly payload: {payload:?}");
                    }
                }

                buf.drain(..consumed);
            }
        }

        Ok(())
    }
}
