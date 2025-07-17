use binaryninja::architecture::{
    Architecture, CoreArchitecture, CoreFlag, CoreFlagClass, CoreFlagGroup, CoreFlagWrite,
    CoreIntrinsic, CoreRegister, CoreRegisterInfo, CoreRegisterStack, CoreRegisterStackInfo,
    CustomArchitectureHandle, InstructionInfo, RegisterId,
};
use binaryninja::disassembly::InstructionTextToken;
use binaryninja::low_level_il::MutableLiftedILFunction;
use binaryninja::Endianness;
use crate::binja::parse::module_data::MODULE_DATA;

#[derive(Clone)]
pub struct WebAssemblyArchitecture {
    handle: CustomArchitectureHandle<Self>,
    core_arch: CoreArchitecture,
}

impl WebAssemblyArchitecture {
    pub fn new(handle: CustomArchitectureHandle<Self>, core_arch: CoreArchitecture) -> Self {
        Self { handle, core_arch }
    }
}

impl AsRef<CoreArchitecture> for WebAssemblyArchitecture {
    fn as_ref(&self) -> &CoreArchitecture {
        &self.core_arch
    }
}

impl Architecture for WebAssemblyArchitecture {
    type Handle = CustomArchitectureHandle<Self>;
    type RegisterInfo = CoreRegisterInfo;
    type Register = CoreRegister;
    type RegisterStackInfo = CoreRegisterStackInfo;
    type RegisterStack = CoreRegisterStack;
    type Flag = CoreFlag;
    type FlagWrite = CoreFlagWrite;
    type FlagClass = CoreFlagClass;
    type FlagGroup = CoreFlagGroup;
    type Intrinsic = CoreIntrinsic;

    fn endianness(&self) -> Endianness {
        Endianness::LittleEndian
    }

    fn address_size(&self) -> usize {
        4
    }

    fn default_integer_size(&self) -> usize {
        4
    }

    fn instruction_alignment(&self) -> usize {
        1
    }

    fn max_instr_len(&self) -> usize {
        256
    }

    fn opcode_display_len(&self) -> usize {
        1
    }

    fn associated_arch_by_addr(&self, _addr: u64) -> CoreArchitecture {
        self.core_arch
    }

    fn instruction_info(&self, data: &[u8], addr: u64) -> Option<InstructionInfo> {
        self._instruction_info(data, addr)
    }

    fn instruction_text(
        &self,
        data: &[u8],
        addr: u64,
    ) -> Option<(usize, Vec<InstructionTextToken>)> {
        self._instruction_text(data, addr)
    }

    fn instruction_llil(
        &self,
        _data: &[u8],
        addr: u64,
        _il: &mut MutableLiftedILFunction<Self>,
    ) -> Option<(usize, bool)> {
        let module_data_lock = MODULE_DATA.lock().unwrap();
        let module_data = module_data_lock.as_ref()?;
        let func = module_data.funcs.get(&addr)?.as_ref();

        if addr == func.size_start {
            Some(((func.locals_start - func.size_start) as usize, false))
        } else if addr == func.locals_start {
            Some(((func.ops_start - func.locals_start) as usize, false))
        } else {
            None
        }
    }

    fn registers_all(&self) -> Vec<Self::Register> {
        Vec::new()
    }

    fn registers_full_width(&self) -> Vec<Self::Register> {
        Vec::new()
    }

    fn stack_pointer_reg(&self) -> Option<Self::Register> {
        None
    }

    fn register_from_id(&self, _id: RegisterId) -> Option<Self::Register> {
        None
    }

    fn handle(&self) -> Self::Handle {
        self.handle
    }
}
