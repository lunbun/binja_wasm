use crate::binja::parse::module_data::FunctionData;
use binaryninja::architecture::BranchKind;
use std::collections::BTreeMap;
use std::pin::Pin;
use wasmparser::{BinaryReader, FunctionBody, Operator};

pub(crate) fn parse_func(
    size_start: u64,
    locals_start: u64,
    end: u64,
    raw: Pin<Box<[u8]>>,
) -> Result<FunctionData, ()> {
    let body = FunctionBody::new(BinaryReader::new(&raw, locals_start as usize));
    let mut ops_reader = body.get_operators_reader().map_err(|_| ())?;
    let ops_start = ops_reader.original_position() as u64;

    enum BreakKind {
        Br { from: u64 },
        BrIf { from: u64, else_: u64 }
    }
    enum BlockKind {
        Normal,
        Function,
        Loop,
        If { if_start: u64 },
        IfElse { if_start: u64, if_end: u64, else_start: u64 },
    }
    struct Block {
        pub start: u64,
        pub brs: Vec<BreakKind>,
        pub kind: BlockKind
    }
    impl Block {
        fn new(start: u64, kind: BlockKind) -> Self {
            Block {
                start,
                brs: Vec::new(),
                kind,
            }
        }
    }

    let mut block_stack = vec![Block::new(ops_start, BlockKind::Function)];
    let mut ops = BTreeMap::new();
    let mut branches: BTreeMap<u64, Vec<BranchKind>> = BTreeMap::new();
    let mut add_branch = |addr: u64, branch: BranchKind| {
        branches.entry(addr).or_default().push(branch);
    };
    while !ops_reader.eof() {
        let offset = ops_reader.original_position() as u64;
        let op = ops_reader.read().map_err(|_| ())?;
        let next_offset = ops_reader.original_position() as u64;

        match &op {
            // Defer insertion of branches for the following operators
            // until the end of the block.
            Operator::Block { blockty } => {
                block_stack.push(Block::new(offset, BlockKind::Normal));
            }
            Operator::Loop { blockty } => {
                block_stack.push(Block::new(offset, BlockKind::Loop));
            }
            Operator::If { blockty } => {
                block_stack.push(Block::new(offset, BlockKind::If {
                    if_start: next_offset
                }));
            }
            Operator::Else => {
                let block = block_stack.last_mut().ok_or(())?;
                if let BlockKind::If { if_start } = &block.kind {
                    block.kind = BlockKind::IfElse {
                        if_start: *if_start,
                        if_end: offset,
                        else_start: next_offset,
                    };
                } else {
                    return Err(());
                }
            }
            Operator::Br { relative_depth } => {
                let index = block_stack.len() - *relative_depth as usize - 1;
                let block = block_stack.get_mut(index).ok_or(())?;
                block.brs.push(BreakKind::Br { from: offset });
            }
            Operator::BrIf { relative_depth } => {
                let index = block_stack.len() - *relative_depth as usize - 1;
                let block = block_stack.get_mut(index).ok_or(())?;
                block.brs.push(BreakKind::BrIf { from: offset, else_: next_offset });
            }

            // The following operators insert branches immediately.
            Operator::Unreachable => {
                add_branch(offset, BranchKind::Exception);
            }
            Operator::Return => {
                add_branch(offset, BranchKind::FunctionReturn);
            }
            Operator::End => {
                let block = block_stack.pop().ok_or(())?;

                for br in &block.brs {
                    match br {
                        BreakKind::Br { from } => {
                            add_branch(*from, BranchKind::Unconditional(next_offset));
                        }
                        BreakKind::BrIf { from, else_ } => {
                            add_branch(*from, BranchKind::True(next_offset));
                            add_branch(*from, BranchKind::False(*else_));
                        }
                    }
                }

                match block.kind {
                    BlockKind::Normal => {}
                    BlockKind::Function => {
                        add_branch(offset, BranchKind::FunctionReturn);
                    }
                    BlockKind::Loop => {
                        add_branch(offset, BranchKind::Unconditional(block.start));
                    }
                    BlockKind::If { if_start } => {
                        add_branch(block.start, BranchKind::True(if_start));
                        add_branch(block.start, BranchKind::False(next_offset));
                    }
                    BlockKind::IfElse { if_start, if_end, else_start } => {
                        add_branch(block.start, BranchKind::True(if_start));
                        add_branch(block.start, BranchKind::False(else_start));
                        add_branch(if_end, BranchKind::Unconditional(next_offset));
                    }
                }
            }
            _ => {}
        }

        // SAFETY: See the comment in `FunctionData` about the lifetime of `Operator`.
        let op = unsafe { std::mem::transmute::<Operator<'_>, Operator<'static>>(op) };

        let size = (ops_reader.original_position() as u64 - offset) as usize;
        ops.insert(offset, (op, size));
    }

    Ok(FunctionData::new(
        size_start,
        locals_start,
        ops_start,
        end,
        ops,
        branches,
        raw,
    ))
}
