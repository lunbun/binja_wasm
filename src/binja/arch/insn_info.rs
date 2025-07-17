use crate::binja::arch::WebAssemblyArchitecture;
use crate::binja::parse::module_data::{BranchTargetAddr, MODULE_DATA};
use binaryninja::architecture::{BranchInfo, BranchKind, InstructionInfo};
use wasmparser::Operator;

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
            let op = func.ops.get(&addr)?;
            let mut info = InstructionInfo::new(op.size, 0);

            if let Some(target) = &op.target {
                match target {
                    BranchTargetAddr::Unconditional(addr) => {
                        info.add_branch(BranchInfo::new(BranchKind::Unconditional(*addr)));
                    }
                    BranchTargetAddr::Conditional { true_target, false_target } => {
                        info.add_branch(BranchInfo::new(BranchKind::True(*true_target)));
                        info.add_branch(BranchInfo::new(BranchKind::False(*false_target)));
                    }
                    BranchTargetAddr::Table { .. } => {
                        // Unfortunately, there's no way to tell binja about the candidate
                        // addresses...
                        info.add_branch(BranchInfo::new(BranchKind::Indirect));
                    }
                    BranchTargetAddr::FunctionEnd => {
                        info.add_branch(BranchInfo::new(BranchKind::FunctionReturn));
                    }
                }
            }

            // Some additional instructions that binja wants us to tell it about.
            match &op.op {
                Operator::Unreachable => {
                    info.add_branch(BranchInfo::new(BranchKind::Exception));
                }
                Operator::Return => {
                    info.add_branch(BranchInfo::new(BranchKind::FunctionReturn));
                }
                Operator::Call { function_index } => {
                    let addr = *module_data.func_addrs.get(*function_index as usize)?;
                    info.add_branch(BranchInfo::new(BranchKind::Call(addr)));
                }
                Operator::CallIndirect { type_index, table_index } => {
                    // Technically, we should be able to deduce candidate addresses for
                    // the call based off the func type information...
                    //
                    // Don't actually tell binja about the indirect call since
                    // BranchKind::Indirect doesn't know its a call and assumes it won't
                    // return.
                    // info.add_branch(BranchInfo::new(BranchKind::Indirect));
                }
                _ => {}
            }

            Some(info)
        }
    }
}
