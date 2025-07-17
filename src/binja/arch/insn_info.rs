use binaryninja::architecture::{BranchInfo, BranchKind, InstructionInfo};
use crate::binja::arch::WebAssemblyArchitecture;
use crate::binja::parse::module_data::MODULE_DATA;

impl WebAssemblyArchitecture {
    pub(crate) fn _instruction_info(&self, _data: &[u8], addr: u64) -> Option<InstructionInfo> {
        let module_data_lock = MODULE_DATA.lock().unwrap();
        let module_data = module_data_lock.as_ref()?;
        let func = module_data.funcs.get(&addr)?.as_ref();

        if addr == func.size_start {
            Some(InstructionInfo::new(
                (func.locals_start - func.size_start) as usize,
                0,
            ))
        } else if addr == func.locals_start {
            Some(InstructionInfo::new(
                (func.ops_start - func.locals_start) as usize,
                0,
            ))
        } else {
            let (_op, size) = func.ops.get(&addr)?;
            let mut info = InstructionInfo::new(*size, 0);

            if let Some(branches) = func.branches.get(&addr) {
                for branch in branches {
                    info.add_branch(BranchInfo::new(*branch));
                }
            }

            Some(info)
        }
    }
}
