use crate::binja::arch::WebAssemblyArchitecture;
use crate::binja::parse::module_data::MODULE_DATA;
use binaryninja::disassembly::{InstructionTextToken, InstructionTextTokenKind};
use wasmparser::Operator;

// https://github.com/Vector35/binaryninja-api/blob/99ed22fd9799ccfa0367b03de4d04d3b9ab26cd5/arch/x86/arch_x86.cpp#L743
fn padding(insn_name_length: usize) -> InstructionTextToken {
    let min = if 7 < insn_name_length {
        7
    } else {
        insn_name_length
    };
    InstructionTextToken::new(" ".repeat(8 - min), InstructionTextTokenKind::Text)
}

macro_rules! vec_with_opcode {
    ($opcode_name:expr) => {{
        vec![
            InstructionTextToken::new($opcode_name, InstructionTextTokenKind::Instruction)
        ]
    }};

    ($opcode_name:expr, $($x:expr),* $(,)?) => {{
        vec![
            InstructionTextToken::new($opcode_name, InstructionTextTokenKind::Instruction),
            padding($opcode_name.len()),
            $($x),*
        ]
    }};
}

impl WebAssemblyArchitecture {
    pub(crate) fn _instruction_text(
        &self,
        _data: &[u8],
        addr: u64,
    ) -> Option<(usize, Vec<InstructionTextToken>)> {
        let module_data_lock = MODULE_DATA.lock().unwrap();
        let module_data = module_data_lock.as_ref()?;
        let func = module_data.funcs.get(&addr)?.as_ref();

        if addr == func.size_start {
            let size = func.end - func.locals_start;
            Some((
                (func.locals_start - func.size_start) as usize,
                vec_with_opcode!(
                    "_funchdr.size",
                    InstructionTextToken::new(
                        format!("{size:#x}"),
                        InstructionTextTokenKind::Integer {
                            value: size,
                            size: Some(4),
                        },
                    ),
                ),
            ))
        } else if addr == func.locals_start {
            Some((
                (func.ops_start - func.locals_start) as usize,
                vec_with_opcode!("_funchdr.locals"),
            ))
        } else {
            let op = func.ops.get(&addr)?;
            Some((
                op.size,
                match &op.op {
                    // Control instructions
                    Operator::Unreachable => vec_with_opcode!("unreachable"),
                    Operator::Nop => vec_with_opcode!("nop"),
                    Operator::Block { blockty } => vec_with_opcode!("block"),
                    Operator::Loop { blockty } => vec_with_opcode!("loop"),
                    Operator::If { blockty } => vec_with_opcode!("if"),
                    Operator::Else => vec_with_opcode!("else"),
                    Operator::End => vec_with_opcode!("end"),
                    Operator::Br { relative_depth } => vec_with_opcode!(
                        "br",
                        InstructionTextToken::new(
                            format!("{relative_depth}"),
                            InstructionTextTokenKind::Integer {
                                value: *relative_depth as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::BrIf { relative_depth } => vec_with_opcode!(
                        "br_if",
                        InstructionTextToken::new(
                            format!("{relative_depth}"),
                            InstructionTextTokenKind::Integer {
                                value: *relative_depth as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::BrTable { targets } => vec_with_opcode!(
                        "br_table",
                        InstructionTextToken::new(
                            format!("{targets:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::Return => vec_with_opcode!("return"),
                    Operator::Call { function_index } => vec_with_opcode!(
                        "call",
                        InstructionTextToken::new(
                            format!("{function_index}"),
                            InstructionTextTokenKind::Integer {
                                value: *function_index as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::CallIndirect {
                        type_index,
                        table_index,
                    } => vec_with_opcode!(
                        "call_indirect",
                        InstructionTextToken::new(
                            format!("{type_index}"),
                            InstructionTextTokenKind::Integer {
                                value: *type_index as u64,
                                size: Some(4),
                            },
                        ),
                    ),

                    // Parametric instructions
                    Operator::Drop => vec_with_opcode!("drop"),
                    Operator::Select => vec_with_opcode!("select"),

                    // Variable instructions
                    Operator::LocalGet { local_index } => vec_with_opcode!(
                        "local.get",
                        InstructionTextToken::new(
                            format!("{local_index}"),
                            InstructionTextTokenKind::Integer {
                                value: *local_index as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::LocalSet { local_index } => vec_with_opcode!(
                        "local.set",
                        InstructionTextToken::new(
                            format!("{local_index}"),
                            InstructionTextTokenKind::Integer {
                                value: *local_index as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::LocalTee { local_index } => vec_with_opcode!(
                        "local.tee",
                        InstructionTextToken::new(
                            format!("{local_index}"),
                            InstructionTextTokenKind::Integer {
                                value: *local_index as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::GlobalGet { global_index } => {
                        vec_with_opcode![
                            "global.get",
                            InstructionTextToken::new(
                                format!("{global_index}"),
                                InstructionTextTokenKind::Integer {
                                    value: *global_index as u64,
                                    size: Some(4),
                                },
                            ),
                        ]
                    }
                    Operator::GlobalSet { global_index } => {
                        vec_with_opcode![
                            "global.set",
                            InstructionTextToken::new(
                                format!("{global_index}"),
                                InstructionTextTokenKind::Integer {
                                    value: *global_index as u64,
                                    size: Some(4),
                                },
                            ),
                        ]
                    }

                    // Memory instructions
                    Operator::I32Load { memarg } => vec_with_opcode!(
                        "i32.load",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load { memarg } => vec_with_opcode!(
                        "i64.load",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::F32Load { memarg } => vec_with_opcode!(
                        "f32.load",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::F64Load { memarg } => vec_with_opcode!(
                        "f64.load",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Load8S { memarg } => vec_with_opcode!(
                        "i32.load8_s",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Load8U { memarg } => vec_with_opcode!(
                        "i32.load8_u",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Load16S { memarg } => vec_with_opcode!(
                        "i32.load16_s",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Load16U { memarg } => vec_with_opcode!(
                        "i32.load16_u",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load8S { memarg } => vec_with_opcode!(
                        "i64.load8_s",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load8U { memarg } => vec_with_opcode!(
                        "i64.load8_u",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load16S { memarg } => vec_with_opcode!(
                        "i64.load16_s",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load16U { memarg } => vec_with_opcode!(
                        "i64.load16_u",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load32S { memarg } => vec_with_opcode!(
                        "i64.load32_s",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Load32U { memarg } => vec_with_opcode!(
                        "i64.load32_u",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Store { memarg } => vec_with_opcode!(
                        "i32.store",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Store { memarg } => vec_with_opcode!(
                        "i64.store",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::F32Store { memarg } => vec_with_opcode!(
                        "f32.store",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::F64Store { memarg } => vec_with_opcode!(
                        "f64.store",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Store8 { memarg } => vec_with_opcode!(
                        "i32.store8",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I32Store16 { memarg } => vec_with_opcode!(
                        "i32.store16",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Store8 { memarg } => vec_with_opcode!(
                        "i64.store8",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Store16 { memarg } => vec_with_opcode!(
                        "i64.store16",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::I64Store32 { memarg } => vec_with_opcode!(
                        "i64.store32",
                        InstructionTextToken::new(
                            format!("{memarg:?}"),
                            InstructionTextTokenKind::Text
                        ),
                    ),
                    Operator::MemorySize { mem } => vec_with_opcode!(
                        "memory.size",
                        InstructionTextToken::new(
                            format!("{mem}"),
                            InstructionTextTokenKind::Integer {
                                value: *mem as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::MemoryGrow { mem } => vec_with_opcode!(
                        "memory.grow",
                        InstructionTextToken::new(
                            format!("{mem}"),
                            InstructionTextTokenKind::Integer {
                                value: *mem as u64,
                                size: Some(4),
                            },
                        ),
                    ),

                    // Numeric instructions
                    Operator::I32Const { value } => vec_with_opcode!(
                        "i32.const",
                        InstructionTextToken::new(
                            format!("{value:#x}"),
                            InstructionTextTokenKind::Integer {
                                value: *value as u64,
                                size: Some(4),
                            },
                        ),
                    ),
                    Operator::I64Const { value } => vec_with_opcode!(
                        "i64.const",
                        InstructionTextToken::new(
                            format!("{value:#x}"),
                            InstructionTextTokenKind::Integer {
                                value: *value as u64,
                                size: Some(8),
                            },
                        ),
                    ),
                    Operator::F32Const { value } => {
                        let value: f32 = (*value).into();
                        vec_with_opcode!(
                            "f32.const",
                            InstructionTextToken::new(
                                format!("{value}"),
                                InstructionTextTokenKind::FloatingPoint {
                                    value: value as f64,
                                    size: Some(4),
                                },
                            ),
                        )
                    }
                    Operator::F64Const { value } => {
                        let value: f64 = (*value).into();
                        vec_with_opcode!(
                            "f64.const",
                            InstructionTextToken::new(
                                format!("{value}"),
                                InstructionTextTokenKind::FloatingPoint {
                                    value,
                                    size: Some(8),
                                },
                            ),
                        )
                    }
                    Operator::I32Eqz => vec_with_opcode!("i32.eqz"),
                    Operator::I32Eq => vec_with_opcode!("i32.eq"),
                    Operator::I32Ne => vec_with_opcode!("i32.ne"),
                    Operator::I32LtS => vec_with_opcode!("i32.lt_s"),
                    Operator::I32LtU => vec_with_opcode!("i32.lt_u"),
                    Operator::I32GtS => vec_with_opcode!("i32.gt_s"),
                    Operator::I32GtU => vec_with_opcode!("i32.gt_u"),
                    Operator::I32LeS => vec_with_opcode!("i32.le_s"),
                    Operator::I32LeU => vec_with_opcode!("i32.le_u"),
                    Operator::I32GeS => vec_with_opcode!("i32.ge_s"),
                    Operator::I32GeU => vec_with_opcode!("i32.ge_u"),
                    Operator::I64Eqz => vec_with_opcode!("i64.eqz"),
                    Operator::I64Eq => vec_with_opcode!("i64.eq"),
                    Operator::I64Ne => vec_with_opcode!("i64.ne"),
                    Operator::I64LtS => vec_with_opcode!("i64.lt_s"),
                    Operator::I64LtU => vec_with_opcode!("i64.lt_u"),
                    Operator::I64GtS => vec_with_opcode!("i64.gt_s"),
                    Operator::I64GtU => vec_with_opcode!("i64.gt_u"),
                    Operator::I64LeS => vec_with_opcode!("i64.le_s"),
                    Operator::I64LeU => vec_with_opcode!("i64.le_u"),
                    Operator::I64GeS => vec_with_opcode!("i64.ge_s"),
                    Operator::I64GeU => vec_with_opcode!("i64.ge_u"),
                    Operator::F32Eq => vec_with_opcode!("f32.eq"),
                    Operator::F32Ne => vec_with_opcode!("f32.ne"),
                    Operator::F32Lt => vec_with_opcode!("f32.lt"),
                    Operator::F32Gt => vec_with_opcode!("f32.gt"),
                    Operator::F32Le => vec_with_opcode!("f32.le"),
                    Operator::F32Ge => vec_with_opcode!("f32.ge"),
                    Operator::F64Eq => vec_with_opcode!("f64.eq"),
                    Operator::F64Ne => vec_with_opcode!("f64.ne"),
                    Operator::F64Lt => vec_with_opcode!("f64.lt"),
                    Operator::F64Gt => vec_with_opcode!("f64.gt"),
                    Operator::F64Le => vec_with_opcode!("f64.le"),
                    Operator::F64Ge => vec_with_opcode!("f64.ge"),
                    Operator::I32Clz => vec_with_opcode!("i32.clz"),
                    Operator::I32Ctz => vec_with_opcode!("i32.ctz"),
                    Operator::I32Popcnt => vec_with_opcode!("i32.popcnt"),
                    Operator::I32Add => vec_with_opcode!("i32.add"),
                    Operator::I32Sub => vec_with_opcode!("i32.sub"),
                    Operator::I32Mul => vec_with_opcode!("i32.mul"),
                    Operator::I32DivS => vec_with_opcode!("i32.div_s"),
                    Operator::I32DivU => vec_with_opcode!("i32.div_u"),
                    Operator::I32RemS => vec_with_opcode!("i32.rem_s"),
                    Operator::I32RemU => vec_with_opcode!("i32.rem_u"),
                    Operator::I32And => vec_with_opcode!("i32.and"),
                    Operator::I32Or => vec_with_opcode!("i32.or"),
                    Operator::I32Xor => vec_with_opcode!("i32.xor"),
                    Operator::I32Shl => vec_with_opcode!("i32.shl"),
                    Operator::I32ShrS => vec_with_opcode!("i32.shr_s"),
                    Operator::I32ShrU => vec_with_opcode!("i32.shr_u"),
                    Operator::I32Rotl => vec_with_opcode!("i32.rotl"),
                    Operator::I32Rotr => vec_with_opcode!("i32.rotr"),
                    Operator::I64Clz => vec_with_opcode!("i64.clz"),
                    Operator::I64Ctz => vec_with_opcode!("i64.ctz"),
                    Operator::I64Popcnt => vec_with_opcode!("i64.popcnt"),
                    Operator::I64Add => vec_with_opcode!("i64.add"),
                    Operator::I64Sub => vec_with_opcode!("i64.sub"),
                    Operator::I64Mul => vec_with_opcode!("i64.mul"),
                    Operator::I64DivS => vec_with_opcode!("i64.div_s"),
                    Operator::I64DivU => vec_with_opcode!("i64.div_u"),
                    Operator::I64RemS => vec_with_opcode!("i64.rem_s"),
                    Operator::I64RemU => vec_with_opcode!("i64.rem_u"),
                    Operator::I64And => vec_with_opcode!("i64.and"),
                    Operator::I64Or => vec_with_opcode!("i64.or"),
                    Operator::I64Xor => vec_with_opcode!("i64.xor"),
                    Operator::I64Shl => vec_with_opcode!("i64.shl"),
                    Operator::I64ShrS => vec_with_opcode!("i64.shr_s"),
                    Operator::I64ShrU => vec_with_opcode!("i64.shr_u"),
                    Operator::I64Rotl => vec_with_opcode!("i64.rotl"),
                    Operator::I64Rotr => vec_with_opcode!("i64.rotr"),
                    Operator::F32Abs => vec_with_opcode!("f32.abs"),
                    Operator::F32Neg => vec_with_opcode!("f32.neg"),
                    Operator::F32Ceil => vec_with_opcode!("f32.ceil"),
                    Operator::F32Floor => vec_with_opcode!("f32.floor"),
                    Operator::F32Trunc => vec_with_opcode!("f32.trunc"),
                    Operator::F32Nearest => vec_with_opcode!("f32.nearest"),
                    Operator::F32Sqrt => vec_with_opcode!("f32.sqrt"),
                    Operator::F32Add => vec_with_opcode!("f32.add"),
                    Operator::F32Sub => vec_with_opcode!("f32.sub"),
                    Operator::F32Mul => vec_with_opcode!("f32.mul"),
                    Operator::F32Div => vec_with_opcode!("f32.div"),
                    Operator::F32Min => vec_with_opcode!("f32.min"),
                    Operator::F32Max => vec_with_opcode!("f32.max"),
                    Operator::F32Copysign => vec_with_opcode!("f32.copysign"),
                    Operator::F64Abs => vec_with_opcode!("f64.abs"),
                    Operator::F64Neg => vec_with_opcode!("f64.neg"),
                    Operator::F64Ceil => vec_with_opcode!("f64.ceil"),
                    Operator::F64Floor => vec_with_opcode!("f64.floor"),
                    Operator::F64Trunc => vec_with_opcode!("f64.trunc"),
                    Operator::F64Nearest => vec_with_opcode!("f64.nearest"),
                    Operator::F64Sqrt => vec_with_opcode!("f64.sqrt"),
                    Operator::F64Add => vec_with_opcode!("f64.add"),
                    Operator::F64Sub => vec_with_opcode!("f64.sub"),
                    Operator::F64Mul => vec_with_opcode!("f64.mul"),
                    Operator::F64Div => vec_with_opcode!("f64.div"),
                    Operator::F64Min => vec_with_opcode!("f64.min"),
                    Operator::F64Max => vec_with_opcode!("f64.max"),
                    Operator::F64Copysign => vec_with_opcode!("f64.copysign"),
                    Operator::I32WrapI64 => vec_with_opcode!("i32.wrap_i64"),
                    Operator::I32TruncF32S => vec_with_opcode!("i32.trunc_f32_s"),
                    Operator::I32TruncF32U => vec_with_opcode!("i32.trunc_f32_u"),
                    Operator::I32TruncF64S => vec_with_opcode!("i32.trunc_f64_s"),
                    Operator::I32TruncF64U => vec_with_opcode!("i32.trunc_f64_u"),
                    Operator::I64ExtendI32S => vec_with_opcode!("i64.extend_i32_s"),
                    Operator::I64ExtendI32U => vec_with_opcode!("i64.extend_i32_u"),
                    Operator::I64TruncF32S => vec_with_opcode!("i64.trunc_f32_s"),
                    Operator::I64TruncF32U => vec_with_opcode!("i64.trunc_f32_u"),
                    Operator::I64TruncF64S => vec_with_opcode!("i64.trunc_f64_s"),
                    Operator::I64TruncF64U => vec_with_opcode!("i64.trunc_f64_u"),
                    Operator::F32ConvertI32S => vec_with_opcode!("f32.convert_i32_s"),
                    Operator::F32ConvertI32U => vec_with_opcode!("f32.convert_i32_u"),
                    Operator::F32ConvertI64S => vec_with_opcode!("f32.convert_i64_s"),
                    Operator::F32ConvertI64U => vec_with_opcode!("f32.convert_i64_u"),
                    Operator::F32DemoteF64 => vec_with_opcode!("f32.demote_f64"),
                    Operator::F64ConvertI32S => vec_with_opcode!("f64.convert_i32_s"),
                    Operator::F64ConvertI32U => vec_with_opcode!("f64.convert_i32_u"),
                    Operator::F64ConvertI64S => vec_with_opcode!("f64.convert_i64_s"),
                    Operator::F64ConvertI64U => vec_with_opcode!("f64.convert_i64_u"),
                    Operator::F64PromoteF32 => vec_with_opcode!("f64.promote_f32"),
                    Operator::I32ReinterpretF32 => vec_with_opcode!("i32.reinterpret_f32"),
                    Operator::I64ReinterpretF64 => vec_with_opcode!("i64.reinterpret_f64"),
                    Operator::F32ReinterpretI32 => vec_with_opcode!("f32.reinterpret_i32"),
                    Operator::F64ReinterpretI64 => vec_with_opcode!("f64.reinterpret_i64"),
                    Operator::I32Extend8S => vec_with_opcode!("i32.extend8_s"),
                    Operator::I32Extend16S => vec_with_opcode!("i32.extend16_s"),
                    Operator::I64Extend8S => vec_with_opcode!("i64.extend8_s"),
                    Operator::I64Extend16S => vec_with_opcode!("i64.extend16_s"),
                    Operator::I64Extend32S => vec_with_opcode!("i64.extend32_s"),
                    Operator::I32TruncSatF32S => vec_with_opcode!("i32.trunc_sat_f32_s"),
                    Operator::I32TruncSatF32U => vec_with_opcode!("i32.trunc_sat_f32_u"),
                    Operator::I32TruncSatF64S => vec_with_opcode!("i32.trunc_sat_f64_s"),
                    Operator::I32TruncSatF64U => vec_with_opcode!("i32.trunc_sat_f64_u"),
                    Operator::I64TruncSatF32S => vec_with_opcode!("i64.trunc_sat_f32_s"),
                    Operator::I64TruncSatF32U => vec_with_opcode!("i64.trunc_sat_f32_u"),
                    Operator::I64TruncSatF64S => vec_with_opcode!("i64.trunc_sat_f64_s"),
                    Operator::I64TruncSatF64U => vec_with_opcode!("i64.trunc_sat_f64_u"),

                    _ => {
                        return None;
                    }
                },
            ))
        }
    }
}
