use crate::util::arc_identity::ArcIdentity;
use once_cell::sync::Lazy;
use rangemap::RangeMap;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Mutex;
use wasmparser::Operator;

// Unfortunately, due to limitations of the binja rust API, we need to store module data
// in a global static variable...
#[derive(Debug)]
pub enum BranchTarget<T> {
    Unconditional(T),
    Conditional {
        true_target: T,
        false_target: T,
    },
    Table {
        targets: Vec<T>,
        default_target: T,
    },
    FunctionEnd
}

pub type BranchTargetAddr = BranchTarget<u64>;

#[derive(Debug)]
pub struct OperatorData<'a> {
    pub op: Operator<'a>,
    pub size: usize,

    // pub stack_height: usize,    // Stack height before the operator is executed.
    pub target: Option<BranchTargetAddr>
}

#[derive(Debug)]
pub struct FunctionData {
    // Address of the size:u32 field in the function header.
    pub size_start: u64,

    // Address of the vec(locals) field in the function header.
    pub locals_start: u64,

    // Address of the expr field in the function header.
    pub ops_start: u64,

    // Address of the end of the function (exclusive).
    pub end: u64,

    // NB: Unfortunately `Operator` references the raw function bytes, so we need to store
    // the entire function body in memory.
    //
    // In addition, safe Rust will not allow us to use self-referential structs, so we
    // declare the `Operator` with a lifetime parameter of `'static`, when it actually
    // references the `raw` field of this struct.
    //
    // `ops` and `ops_raw` must be declared in this order to ensure correct drop order.
    pub ops: BTreeMap<u64, OperatorData<'static>>,
    pub _raw: Pin<Box<[u8]>>,
}

impl FunctionData {
    pub fn new(
        size_start: u64,
        locals_start: u64,
        ops_start: u64,
        end: u64,
        ops: BTreeMap<u64, OperatorData<'static>>,
        raw: Pin<Box<[u8]>>,
    ) -> Self {
        Self {
            size_start,
            locals_start,
            ops_start,
            end,
            ops,
            _raw: raw,
        }
    }
}

pub struct ModuleData {
    pub funcs: RangeMap<u64, ArcIdentity<FunctionData>>,
    pub func_addrs: Vec<u64>
}

impl ModuleData {
    pub fn new() -> Self {
        Self {
            funcs: RangeMap::new(),
            func_addrs: Vec::new(),
        }
    }
}

pub static MODULE_DATA: Lazy<Mutex<Option<ModuleData>>> = Lazy::new(|| Mutex::new(None));
