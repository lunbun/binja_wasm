use std::cell::OnceCell;
use crate::binja::parse::module_data::{BranchTarget, BranchTargetAddr, FunctionData, OperatorData};
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

    type BlockId = usize;
    enum LabelKind {
        Resolved(u64),  // Known address.

        // Refer to the operator after the end of a block.
        After(BlockId),

        // Refer to the "break" address of a label. For a loop block, this is
        // the start of the loop. For all other blocks, this is the operator after
        // the end of the block.
        Break(BlockId),

        // Refer to the "else" branch of an "if" block. If the block is just an "if"
        // block (not an "if-else" block), this is just the operator after the
        // end of the block.
        Else(BlockId),
    }
    enum BlockKind {
        Normal,
        Function,
        Loop,
        If,
        IfElse { else_start: u64 },
    }
    struct Block {
        pub start: u64,
        pub after: OnceCell<u64>,   // Address of the next operator after the end of this block.
        pub kind: BlockKind
    }

    let mut blocks = Vec::new();
    let mut block_stack = Vec::new();
    fn push_block(blocks: &mut Vec<Block>, block_stack: &mut Vec<BlockId>, start: u64, kind: BlockKind) -> BlockId {
        let block_id = blocks.len() as BlockId;
        blocks.push(Block {
            start,
            after: OnceCell::new(),
            kind,
        });
        block_stack.push(block_id);
        block_id
    }
    fn get_nth_block_id(block_stack: &[BlockId], n: u32) -> Result<BlockId, ()> {
        Ok(*block_stack.get(block_stack.len() - n as usize - 1).ok_or(())?)
    }
    push_block(&mut blocks, &mut block_stack, ops_start, BlockKind::Function);

    // Initial parsing phase.
    let mut ops = BTreeMap::new();
    let mut unpatched_branches: BTreeMap<u64, BranchTarget<LabelKind>> = BTreeMap::new();
    while !ops_reader.eof() {
        let offset = ops_reader.original_position() as u64;
        let op = ops_reader.read().map_err(|_| ())?;
        let next_offset = ops_reader.original_position() as u64;

        match &op {
            Operator::Block { .. } => {
                push_block(&mut blocks, &mut block_stack, offset, BlockKind::Normal);
            }
            Operator::Loop { .. } => {
                push_block(&mut blocks, &mut block_stack, offset, BlockKind::Loop);
            }
            Operator::If { .. } => {
                let block_id = push_block(&mut blocks, &mut block_stack, offset, BlockKind::If);
                unpatched_branches.insert(offset, BranchTarget::Conditional{
                    true_target: LabelKind::Resolved(next_offset),
                    false_target: LabelKind::Else(block_id)
                });
            }
            Operator::Else => {
                let block_id = *block_stack.last().ok_or(())?;
                let block = blocks.get_mut(block_id as usize).ok_or(())?;
                if !matches!(block.kind, BlockKind::If) {
                    return Err(());
                }
                block.kind = BlockKind::IfElse {
                    else_start: next_offset
                };
                unpatched_branches.insert(offset, BranchTarget::Unconditional(LabelKind::After(block_id)));
            }
            Operator::Br { relative_depth } => {
                let block_id = get_nth_block_id(&block_stack, *relative_depth)?;
                unpatched_branches.insert(offset, BranchTarget::Unconditional(LabelKind::Break(block_id)));
            }
            Operator::BrIf { relative_depth } => {
                let block_id = get_nth_block_id(&block_stack, *relative_depth)?;
                unpatched_branches.insert(
                    offset,
                    BranchTarget::Conditional {
                        true_target: LabelKind::Break(block_id),
                        false_target: LabelKind::Resolved(next_offset)
                    }
                );
            }
            Operator::BrTable { targets } => {
                let target_labels = targets.targets().map(|target| {
                    let target = target.map_err(|_| ())?;
                    let block_id = get_nth_block_id(&block_stack, target)?;
                    Ok(LabelKind::Break(block_id))
                }).collect::<Result<Vec<_>, _>>()?;
                let default_id = get_nth_block_id(&block_stack, targets.default())?;
                unpatched_branches.insert(offset, BranchTarget::Table {
                    targets: target_labels,
                    default_target: LabelKind::Break(default_id)
                });
            }
            Operator::End => {
                let block_id = block_stack.pop().ok_or(())?;
                let block = blocks.get_mut(block_id as usize).ok_or(())?;
                block.after.set(next_offset).map_err(|_| ())?;

                if matches!(block.kind, BlockKind::Function) {
                    unpatched_branches.insert(offset, BranchTarget::FunctionEnd);
                }
            }
            _ => {}
        }

        // SAFETY: See the comment in `FunctionData` about the lifetime of `Operator`.
        let op = unsafe { std::mem::transmute::<Operator<'_>, Operator<'static>>(op) };

        let size = (ops_reader.original_position() as u64 - offset) as usize;
        ops.insert(offset, OperatorData {
            op,
            size,
            target: None
        });
    }

    // Now that we know the addresses of all blocks, patch the branch
    // targets.
    let patch_label = |label: &LabelKind| {
        Ok(match label {
            LabelKind::Resolved(addr) => *addr,
            LabelKind::After(block_id) => {
                let block = blocks.get(*block_id as usize).ok_or(())?;
                *block.after.get().ok_or(())?
            },
            LabelKind::Break(block_id) => {
                let block = blocks.get(*block_id as usize).ok_or(())?;
                if matches!(block.kind, BlockKind::Loop) {
                    block.start
                } else {
                    *block.after.get().ok_or(())?
                }
            },
            LabelKind::Else(block_id) => {
                let block = blocks.get(*block_id as usize).ok_or(())?;
                if let BlockKind::IfElse { else_start, .. } = &block.kind {
                    *else_start
                } else {
                    *block.after.get().ok_or(())?
                }
            }
        })
    };
    for (offset, unpatched_branch) in &unpatched_branches {
        let branch = match unpatched_branch {
            BranchTarget::Unconditional(label) => {
                BranchTargetAddr::Unconditional(patch_label(label)?)
            }
            BranchTarget::Conditional { true_target, false_target } => {
                BranchTargetAddr::Conditional {
                    true_target: patch_label(true_target)?,
                    false_target: patch_label(false_target)?
                }
            }
            BranchTarget::Table { targets, default_target } => {
                let targets = targets.into_iter()
                    .map(patch_label)
                    .collect::<Result<Vec<_>, _>>()?;
                BranchTargetAddr::Table {
                    targets,
                    default_target: patch_label(default_target)?
                }
            }
            BranchTarget::FunctionEnd => BranchTargetAddr::FunctionEnd
        };
        ops.get_mut(offset).ok_or(())?.target = Some(branch);
    }

    Ok(FunctionData::new(
        size_start,
        locals_start,
        ops_start,
        end,
        ops,
        raw,
    ))
}
