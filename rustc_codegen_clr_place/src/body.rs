use super::{PlaceTy, array_get_address, array_get_item, pointed_type};
use crate::{body_ty_is_by_address, deref_op};
use cilly::{
    BinOp, Const, FieldDesc, Int, Interned, IntoAsmIndex, Type, call, conv_usize, ld_field,
    v2::CILNode,
};
use rustc_codegen_clr_ctx::MethodCompileCtx;
use rustc_codegen_clr_type::{
    GetTypeExt,
    adt::{FieldOffsetIterator, enum_field_descriptor, field_descrptor},
    r#type::fat_ptr_to,
    utilis::pointer_to_is_fat,
};
use rustc_middle::mir::{Local, PlaceElem};
use rustc_middle::ty::{Ty, TyKind};
pub fn local_body<'tcx>(
    local: usize,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> (Interned<cilly::v2::CILNode>, Ty<'tcx>) {
    let ty = ctx.body().local_decls[Local::from_usize(local)].ty;
    let ty = ctx.monomorphize(ty);
    if body_ty_is_by_address(ty, ctx) {
        (super::address::local_address(local, ctx.body(), ctx), ty)
    } else {
        (super::get::local_get(local, ctx.body(), ctx), ty)
    }
}
fn body_field<'a>(
    curr_type: super::PlaceTy<'a>,
    ctx: &mut MethodCompileCtx<'a, '_>,
    field_index: u32,
    field_ty: Ty<'a>,
    parrent_node: Interned<cilly::v2::CILNode>,
) -> (PlaceTy<'a>, Interned<cilly::v2::CILNode>) {
    match curr_type {
        super::PlaceTy::Ty(curr_type) => {
            let curr_type = ctx.monomorphize(curr_type);
            let field_type = ctx.monomorphize(field_ty);
            match (
                pointer_to_is_fat(curr_type, ctx.tcx(), ctx.instance()),
                pointer_to_is_fat(field_type, ctx.tcx(), ctx.instance()),
            ) {
                (false, false) => {
                    let field_desc = field_descrptor(curr_type, field_index, ctx);
                    if body_ty_is_by_address(field_type, ctx) {
                        (
                            (field_type).into(),
                            ctx.ld_field_addr(parrent_node, field_desc),
                        )
                    } else {
                        ((field_type).into(), ctx.ld_field(parrent_node, field_desc))
                    }
                }
                (false, true) => panic!(
                    "Sized type {curr_type:?} contains an unsized field of type {field_type}. This is a bug."
                ),
                (true, false) => {
                    let mut explicit_offset_iter =
                        FieldOffsetIterator::fields(ctx.layout_of(curr_type).layout.0.0.clone());
                    let offset = explicit_offset_iter
                        .nth(field_index as usize)
                        .expect("Field index not in field offset iterator");
                    let curr_type_fat_ptr = ctx.type_from_cache(Ty::new_ptr(
                        ctx.tcx(),
                        curr_type,
                        rustc_middle::ty::Mutability::Mut,
                    ));
                    let addr_descr = FieldDesc::new(
                        curr_type_fat_ptr.as_class_ref().unwrap(),
                        ctx.alloc_string(cilly::DATA_PTR),
                        ctx.nptr(Type::Void),
                    );
                    // Get the address of the unsized object.
                    let obj_addr = ctx.ld_field(parrent_node, addr_descr);
                    let obj = ctx.type_from_cache(field_type);
                    // Add the offset to the object.
                    let field_addr =
                        ctx.biop(obj_addr, Const::USize(u64::from(offset)), BinOp::Add);
                    let field_addr = ctx.cast_ptr(field_addr, obj);
                    if body_ty_is_by_address(field_type, ctx) {
                        (field_type.into(), field_addr)
                    } else {
                        (field_type.into(), ctx.load(field_addr, obj))
                    }
                }
                (true, true) => {
                    assert_eq!(field_index, 0, "Can't handle DST with more than 1 field.");
                    let field_type = ctx.type_from_cache(Ty::new_ptr(
                        ctx.tcx(),
                        field_type,
                        rustc_middle::ty::Mutability::Mut,
                    ));
                    (field_ty.into(), ctx.cast_ptr(parrent_node, field_type))
                }
            }
        }
        super::PlaceTy::EnumVariant(enm, var_idx) => {
            let owner = ctx.monomorphize(enm);
            let field_desc = enum_field_descriptor(owner, field_index, var_idx, ctx);
            (field_ty.into(), ctx.ld_field_addr(parrent_node, field_desc))
        }
    }
}
pub fn place_elem_body_index<'tcx>(
    curr_ty: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    parrent_node: Interned<cilly::v2::CILNode>,
    index: rustc_middle::mir::Local,
) -> (PlaceTy<'tcx>, Interned<cilly::v2::CILNode>) {
    let index = crate::local_get(index.as_usize(), ctx.body(), ctx);
    match curr_ty.kind() {
        TyKind::Slice(inner) => {
            let inner = ctx.monomorphize(*inner);
            let inner_type = ctx.type_from_cache(inner);
            let slice = fat_ptr_to(Ty::new_slice(ctx.tcx(), inner), ctx);
            let desc = FieldDesc::new(
                slice,
                ctx.alloc_string(cilly::DATA_PTR),
                ctx.nptr(Type::Void),
            );
            let size = ctx.size_of(inner_type);
            let size = size.into_idx(ctx);
            let size = ctx.alloc_node(cilly::CILNode::IntCast {
                input: size,
                target: Int::USize,
                extend: cilly::cilnode::ExtendKind::ZeroExtend,
            });
            let offset = ctx.biop(index, size, cilly::BinOp::Mul);
            let addr = ctx.ld_field(parrent_node, desc);
            let addr = ctx.cast_ptr(addr, inner_type);
            let addr = ctx.biop(addr, offset, BinOp::Add);

            if body_ty_is_by_address(inner, ctx) {
                (inner.into(), addr)
            } else {
                (
                    inner.into(),
                    super::deref_op(super::PlaceTy::Ty(inner), ctx, addr),
                )
            }
        }
        TyKind::Array(element, _length) => {
            let index = ctx.alloc_node(cilly::CILNode::IntCast {
                input: index,
                target: Int::USize,
                extend: cilly::cilnode::ExtendKind::ZeroExtend,
            });
            let element_tpe = ctx.type_from_cache(*element);
            let parrent_node = ctx.cast_ptr(parrent_node, element_tpe);
            let addr = ctx.offset(parrent_node, index, element_tpe);
            if body_ty_is_by_address(*element, ctx) {
                ((*element).into(), addr)
            } else {
                ((*element).into(), ctx.load(addr, element_tpe))
            }
        }
        _ => {
            rustc_middle::ty::print::with_no_trimmed_paths! {todo!("Can't index into {curr_ty}!")}
        }
    }
}
pub fn place_elem_body<'tcx>(
    place_elem: &PlaceElem<'tcx>,
    curr_type: PlaceTy<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    parrent_node: Interned<cilly::v2::CILNode>,
) -> (PlaceTy<'tcx>, Interned<cilly::v2::CILNode>) {
    let curr_ty = match curr_type {
        PlaceTy::Ty(ty) => PlaceTy::Ty(ctx.monomorphize(ty)),
        PlaceTy::EnumVariant(enm, idx) => PlaceTy::EnumVariant(ctx.monomorphize(enm), idx),
    };
    match place_elem {
        PlaceElem::Deref => {
            let pointed = pointed_type(curr_ty);
            if body_ty_is_by_address(pointed, ctx) {
                (pointed.into(), parrent_node)
            } else {
                (pointed.into(), deref_op(pointed.into(), ctx, parrent_node))
            }
        }
        PlaceElem::Field(field_index, field_ty) => body_field(
            curr_type,
            ctx,
            field_index.as_u32(),
            *field_ty,
            parrent_node,
        ),
        PlaceElem::Downcast(_, variant) => {
            let curr_type = curr_ty
                .as_ty()
                .expect("Can't get enum variant of an enum varaint!");
            let curr_type = ctx.monomorphize(curr_type);
            if matches!(curr_ty.as_ty().unwrap().kind(), TyKind::Coroutine(_, _)) {
                eprintln!(
                    "UNTESTED:  downcaststing coroutines is not fully supported, and the behaviour of corrutines is not yet fully tested! variant:{variant:?} curr_type:{curr_type:?}"
                );
                return (curr_type.into(), parrent_node);
            }
            let variant_type = PlaceTy::EnumVariant(curr_type, variant.as_u32());

            (variant_type, parrent_node)
        }
        PlaceElem::Index(index) => place_elem_body_index(
            curr_type
                .as_ty()
                .expect("INVALID PLACE: Indexing into enum variant???"),
            ctx,
            parrent_node,
            *index,
        ),
        PlaceElem::Subtype(tpe) => {
            if body_ty_is_by_address(curr_type.as_ty().unwrap(), ctx) {
                (
                    PlaceTy::Ty(*tpe),
                    super::deref_op((*tpe).into(), ctx, parrent_node),
                )
            } else {
                (PlaceTy::Ty(*tpe), parrent_node)
            }
        }
        PlaceElem::ConstantIndex {
            offset,
            min_length: _,
            from_end,
        } => {
            let curr_ty = curr_ty
                .as_ty()
                .expect("INVALID PLACE: Indexing into enum variant???");
            let index = ctx.alloc_node(Const::USize(*offset));
            assert!(!from_end);
            match curr_ty.kind() {
                TyKind::Slice(inner) => {
                    let inner = ctx.monomorphize(*inner);
                    let inner_type = ctx.type_from_cache(inner);
                    let slice = fat_ptr_to(Ty::new_slice(ctx.tcx(), inner), ctx);
                    let desc = FieldDesc::new(
                        slice,
                        ctx.alloc_string(cilly::DATA_PTR),
                        ctx.nptr(Type::Void),
                    );

                    let addr = ctx.ld_field(parrent_node, desc);
                    let addr = ctx.cast_ptr(addr, inner_type);
                    let addr = ctx.offset(addr, index, inner_type);

                    if body_ty_is_by_address(inner, ctx) {
                        (inner.into(), addr)
                    } else {
                        (
                            inner.into(),
                            super::deref_op(super::PlaceTy::Ty(inner), ctx, addr),
                        )
                    }
                }
                TyKind::Array(element, _length) => {
                    let element_tpe = ctx.type_from_cache(*element);
                    if body_ty_is_by_address(*element, ctx) {
                        let parrent_node = ctx.cast_ptr(parrent_node, element_tpe);
                        let addr = ctx.offset(parrent_node, index, element_tpe);
                        if body_ty_is_by_address(*element, ctx) {
                            ((*element).into(), addr)
                        } else {
                            ((*element).into(), ctx.load(addr, element_tpe))
                        }
                    } else {
                        let parrent_node = ctx.cast_ptr(parrent_node, element_tpe);
                        let addr = ctx.offset(parrent_node, index, element_tpe);
                        if body_ty_is_by_address(*element, ctx) {
                            ((*element).into(), addr)
                        } else {
                            ((*element).into(), ctx.load(addr, element_tpe))
                        }
                    }
                }
                _ => {
                    rustc_middle::ty::print::with_no_trimmed_paths! { todo!("Can't index into {curr_ty}!")}
                }
            }
        }
        _ => todo!("Can't handle porojection {place_elem:?} in body"),
    }
}
