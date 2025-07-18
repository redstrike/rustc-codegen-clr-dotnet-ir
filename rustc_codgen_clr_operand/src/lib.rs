#![feature(rustc_private)]
#![feature(f16)]
extern crate rustc_abi;
extern crate rustc_const_eval;
extern crate rustc_driver;
extern crate rustc_middle;
extern crate rustc_span;
pub mod constant;
pub mod static_data;
use cilly::Type;
use cilly::cil_node::V1Node;
use rustc_codegen_clr_ctx::MethodCompileCtx;
use rustc_codegen_clr_place::{PlaceTy, deref_op, place_address, place_get};
use rustc_codegen_clr_type::GetTypeExt;
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::mir::{ConstValue, Operand};
pub fn handle_operand<'tcx>(
    operand: &Operand<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Node {
    let res = ctx.type_from_cache(ctx.monomorphize(operand.ty(ctx.body(), ctx.tcx())));
    if res == Type::Void {
        return V1Node::uninit_val(Type::Void, ctx);
    }
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place_get(place, ctx),
        Operand::Constant(const_val) => crate::constant::handle_constant(const_val, ctx),
    }
}
pub fn operand_address<'tcx>(
    operand: &Operand<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Node {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place_address(place, ctx),
        Operand::Constant(const_val) => {
            let local_type = ctx.type_from_cache(operand.ty(ctx.body(), ctx.tcx()));
            let constant = crate::constant::handle_constant(const_val, ctx);
            let ptr = V1Node::stack_addr(constant, ctx.alloc_type(local_type), ctx);
            V1Node::LdObj {
                ptr: Box::new(ptr),
                obj: Box::new(local_type),
            }
        }
    }
}
/// Checks if this operand is uninitialzed, and assigements using it can safely be skipped.
pub fn is_uninit<'tcx>(operand: &Operand<'tcx>, ctx: &mut MethodCompileCtx<'tcx, '_>) -> bool {
    match operand {
        Operand::Copy(_) | Operand::Move(_) => false,
        Operand::Constant(const_val) => {
            let constant = const_val.const_;
            let constant = ctx.monomorphize(constant);
            let evaluated = constant
                .eval(
                    ctx.tcx(),
                    rustc_middle::ty::TypingEnv::fully_monomorphized(),
                    const_val.span,
                )
                .expect("Could not evaluate constant!");
            match evaluated {
                ConstValue::Scalar(_) => false, // Scalars are never uninitialized.
                ConstValue::ZeroSized => {
                    // ZeroSized has no data, so I guess it has no initialized data, so assiments using it could propably be safely skipped.
                    true
                }
                ConstValue::Slice { data, .. } => {
                    let mask = data.inner().init_mask();
                    let mut chunks =
                        mask.range_as_init_chunks(rustc_const_eval::interpret::AllocRange {
                            start: rustc_abi::Size::ZERO,
                            size: data.0.size(),
                        });
                    let Some(only) = chunks.next() else {
                        return false;
                    };
                    // If this is not the only chunk, then the init mask must not be fully uninitialized
                    if chunks.next().is_some() {
                        return false;
                    }
                    !only.is_init()
                }
                ConstValue::Indirect { alloc_id, .. } => {
                    let data = ctx.tcx().global_alloc(alloc_id);
                    let rustc_middle::mir::interpret::GlobalAlloc::Memory(data) = data else {
                        return false;
                    };
                    let mask = data.0.init_mask();
                    let mut chunks =
                        mask.range_as_init_chunks(rustc_const_eval::interpret::AllocRange {
                            start: rustc_abi::Size::ZERO,
                            size: data.0.size(),
                        });
                    let Some(only) = chunks.next() else {
                        return false;
                    };
                    // If this is not the only chunk, then the init mask must not be fully uninitialized
                    if chunks.next().is_some() {
                        return false;
                    }
                    !only.is_init()
                }
            }
        }
    }
}

pub fn is_const_zero<'tcx>(operand: &Operand<'tcx>, ctx: &mut MethodCompileCtx<'tcx, '_>) -> bool {
    match operand {
        // Copy / Moves are not constants.
        Operand::Copy(_) | Operand::Move(_) => false,
        Operand::Constant(const_val) => {
            let constant = const_val.const_;
            let constant = ctx.monomorphize(constant);
            let evaluated = constant
                .eval(
                    ctx.tcx(),
                    rustc_middle::ty::TypingEnv::fully_monomorphized(),
                    const_val.span,
                )
                .expect("Could not evaluate constant!");
            match evaluated {
                ConstValue::Scalar(scalar) => match scalar {
                    Scalar::Int(int) => int.is_null(),
                    Scalar::Ptr(_, _) => false,
                }, // Scalars are never uninitialized.
                ConstValue::ZeroSized => {
                    // ZeroSized has no data, so it has only 0 values
                    true
                }
                ConstValue::Slice { .. } | ConstValue::Indirect { .. } => false,
            }
        }
    }
}
