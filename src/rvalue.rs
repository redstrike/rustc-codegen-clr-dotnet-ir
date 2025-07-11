use crate::{
    assembly::MethodCompileCtx,
    utilis::{adt::get_discr, compiletime_sizeof},
};
use cilly::{
    cil_node::V1Node,
    cil_root::V1Root,
    conv_usize, ld_field, size_of, Const, Type,
    {cilnode::MethodKind, FieldDesc, Float, Int, MethodRef},
};
use rustc_codegen_clr_call::CallInfo;
use rustc_codegen_clr_ctx::function_name;
use rustc_codegen_clr_place::{place_address, place_address_raw, place_get};
use rustc_codegen_clr_type::{
    adt::enum_tag_info,
    r#type::{fat_ptr_to, get_type},
    utilis::{pointer_to_is_fat, try_resolve_const_size},
    GetTypeExt,
};
use rustc_codgen_clr_operand::{
    handle_operand, is_const_zero, is_uninit, operand_address, static_data::add_allocation,
};
use rustc_middle::{
    mir::{CastKind, NullOp, Operand, Place, Rvalue},
    ty::{adjustment::PointerCoercion, GenericArgs, Instance, InstanceKind, Ty, TyKind},
};
macro_rules! cast {
    ($ctx:ident,$operand:ident,$target:ident,$cast_name:path,$asm:expr) => {{
        let target = $ctx.monomorphize(*$target);
        let target = $ctx.type_from_cache(target);
        let src = $operand.ty(&$ctx.body().local_decls, $ctx.tcx());
        let src = $ctx.monomorphize(src);
        let src = $ctx.type_from_cache(src);
        $cast_name(
            src,
            target,
            rustc_codgen_clr_operand::handle_operand($operand, $ctx),
            $asm,
        )
    }};
}
pub fn is_rvalue_unint<'tcx>(rvalue: &Rvalue<'tcx>, ctx: &mut MethodCompileCtx<'tcx, '_>) -> bool {
    match rvalue {
        Rvalue::Repeat(operand, _) | Rvalue::Use(operand) => is_uninit(operand, ctx),
        /* TODO: before enabling this, check if the aggregate is an enum, and if so, check if it has a discriminant.
        Rvalue::Aggregate(_, field_index) => field_index
        .iter()
        .all(|operand| is_uninit(operand, ctx)),*/
        _ => false,
    }
}
pub fn is_rvalue_const_0<'tcx>(
    rvalue: &Rvalue<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> bool {
    match rvalue {
        Rvalue::Repeat(operand, _) | Rvalue::Use(operand) => is_const_zero(operand, ctx),
        _ => false,
    }
}
pub fn handle_rvalue<'tcx>(
    rvalue: &Rvalue<'tcx>,
    target_location: &Place<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> (Vec<V1Root>, V1Node) {
    match rvalue {
        Rvalue::Len(operand) => {
            let ty = ctx.monomorphize(operand.ty(ctx.body(), ctx.tcx()));
            match ty.ty.kind() {
                TyKind::Slice(inner) => {
                    let slice_tpe = fat_ptr_to(*inner, ctx);
                    let descriptor = FieldDesc::new(
                        slice_tpe,
                        ctx.alloc_string(crate::METADATA),
                        cilly::Type::Int(Int::USize),
                    );
                    let addr = place_address_raw(operand, ctx);
                    assert!(
                        !matches!(addr, V1Node::LDLoc(_)),
                        "improper addr {addr:?}. operand:{operand:?}"
                    );
                    (vec![], ld_field!(addr, ctx.alloc_field(descriptor)))
                }
                TyKind::Array(_ty, length) => {
                    let len = try_resolve_const_size(ctx.monomorphize(*length)).unwrap();
                    (vec![], V1Node::V2(ctx.alloc_node(Const::USize(len as u64))))
                }
                _ => todo!("Get length of type {ty:?}"),
            }
        }
        Rvalue::Use(operand) => (vec![], handle_operand(operand, ctx)),
        // TODO: check the exact semantics of `WrapUnsafeBinder` once it has some documentation.
        Rvalue::WrapUnsafeBinder(operand, _unknown_ty) => (vec![], handle_operand(operand, ctx)),
        Rvalue::CopyForDeref(place) => (vec![], place_get(place, ctx)),
        Rvalue::Ref(_region, _borrow_kind, place) => (vec![], place_address(place, ctx)),
        Rvalue::RawPtr(_mutability, place) => (vec![], place_address(place, ctx)),
        Rvalue::Cast(
            CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer, _),
            operand,
            _dst,
        ) => (vec![], handle_operand(operand, ctx)),
        Rvalue::Cast(
            CastKind::PointerCoercion(
                PointerCoercion::MutToConstPointer | PointerCoercion::ArrayToPointer,
                _,
            )
            | CastKind::PtrToPtr,
            operand,
            dst,
        ) => (vec![], ptr_to_ptr(ctx, operand, *dst)),
        Rvalue::Cast(CastKind::PointerCoercion(PointerCoercion::Unsize, _), operand, target) => {
            crate::unsize::unsize2(ctx, operand, *target, *target_location)
        }
        Rvalue::BinaryOp(binop, operands) => (
            vec![],
            crate::binop::binop(*binop, &operands.0, &operands.1, ctx),
        ),
        Rvalue::UnaryOp(binop, operand) => {
            (vec![], crate::unop::unop(*binop, operand, ctx, rvalue))
        }
        Rvalue::Cast(CastKind::IntToInt, operand, target) => (
            vec![],
            cast!(ctx, operand, target, crate::casts::int_to_int, ctx),
        ),
        Rvalue::Cast(CastKind::FloatToInt, operand, target) => (
            vec![],
            cast!(ctx, operand, target, crate::casts::float_to_int, ctx),
        ),
        Rvalue::Cast(CastKind::IntToFloat, operand, target) => (
            vec![],
            cast!(ctx, operand, target, crate::casts::int_to_float, ctx),
        ),
        Rvalue::NullaryOp(op, ty) => match op {
            NullOp::SizeOf => {
                let ty = ctx.type_from_cache(ctx.monomorphize(*ty));
                if ty == Type::Void {
                    let val = ctx.alloc_node(Const::USize(0));
                    (vec![], V1Node::V2(val))
                } else {
                    let val = size_of!(ty)(ctx);
                    (vec![], conv_usize!(V1Node::V2(val)))
                }
            }
            NullOp::AlignOf => {
                let algin = rustc_codegen_clr_type::align_of(ctx.monomorphize(*ty), ctx.tcx());
                (vec![], V1Node::V2(ctx.alloc_node(Const::USize(algin))))
            }
            NullOp::OffsetOf(fields) => {
                let layout = ctx.layout_of(*ty);
                let offset = ctx
                    .tcx()
                    .offset_of_subfield(
                        rustc_middle::ty::TypingEnv::fully_monomorphized(),
                        layout,
                        fields.iter(),
                    )
                    .bytes();
                (vec![], V1Node::V2(ctx.alloc_node(Const::USize(offset))))
            }
            rustc_middle::mir::NullOp::UbChecks => {
                let ub_checks = ctx.tcx().sess.ub_checks();
                (vec![], V1Node::V2(ctx.alloc_node(ub_checks)))
            }
            rustc_middle::mir::NullOp::ContractChecks => {
                let cc_checks = ctx.tcx().sess.contract_checks();
                (vec![], V1Node::V2(ctx.alloc_node(cc_checks)))
            }
        },
        Rvalue::Aggregate(aggregate_kind, field_index) => crate::aggregate::handle_aggregate(
            ctx,
            target_location,
            aggregate_kind.as_ref(),
            field_index,
        ),

        Rvalue::Cast(
            CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer(_), _),
            ref operand,
            to_ty,
        ) => match ctx.monomorphize(operand.ty(ctx.body(), ctx.tcx())).kind() {
            TyKind::Closure(def_id, args) => {
                let instance = Instance::resolve_closure(
                    ctx.tcx(),
                    *def_id,
                    args,
                    rustc_middle::ty::ClosureKind::FnOnce,
                );
                let call_info = CallInfo::sig_from_instance_(instance, ctx);

                let function_name = function_name(ctx.tcx().symbol_name(instance));
                let fn_ptr_sig = ctx.alloc_sig(call_info.sig().clone());
                let call_site = MethodRef::new(
                    *ctx.main_module(),
                    ctx.alloc_string(function_name),
                    fn_ptr_sig,
                    MethodKind::Static,
                    vec![].into(),
                );
                let target_type = ctx.type_from_cache(*to_ty);
                let Type::FnPtr(target_sig) = target_type else {
                    rustc_middle::bug!(
                        "ClosureFnPointer target not a fn ptr. {}",
                        target_type.mangle(ctx)
                    )
                };
                if target_sig == fn_ptr_sig {
                    (vec![], V1Node::LDFtn(ctx.alloc_methodref(call_site)))
                } else {
                    eprintln!(
                        "Possible bug. ClosureFnPointer target:{} and source {} don't match.",
                        target_type.mangle(ctx),
                        Type::FnPtr(fn_ptr_sig).mangle(ctx)
                    );
                    let src = V1Node::LDFtn(ctx.alloc_methodref(call_site));
                    (vec![], V1Node::cast_ptr(src, target_type))
                }
            }
            _ => panic!(
                "{} cannot be cast to a fn ptr",
                operand.ty(ctx.body(), ctx.tcx())
            ),
        },
        Rvalue::Cast(CastKind::Transmute, operand, dst) => {
            let dst = ctx.monomorphize(*dst);
            let dst = ctx.type_from_cache(dst);
            let src = operand.ty(&ctx.body().local_decls, ctx.tcx());
            let src = ctx.monomorphize(src);
            let src = ctx.type_from_cache(src);
            match (&src, &dst) {
                (
                    Type::Int(Int::ISize | Int::USize) | Type::Ptr(_) | Type::FnPtr(_),
                    Type::Int(Int::ISize | Int::USize) | Type::Ptr(_) | Type::FnPtr(_),
                ) => (vec![], handle_operand(operand, ctx).cast_ptr(dst)),

                (Type::Int(Int::U16), Type::PlatformChar) => (vec![], handle_operand(operand, ctx)),
                (_, _) => (
                    vec![],
                    handle_operand(operand, ctx).transmute_on_stack(src, dst, ctx),
                ),
            }
        }
        Rvalue::ShallowInitBox(operand, dst) => {
            let dst = ctx.monomorphize(*dst);
            let boxed_dst = Ty::new_box(ctx.tcx(), dst);
            //let dst = tycache.type_from_cache(dst, tcx, method_instance);
            let src = operand.ty(&ctx.body().local_decls, ctx.tcx());
            let boxed_dst_type = ctx.type_from_cache(boxed_dst);
            let src = ctx.monomorphize(src);
            assert!(
                !pointer_to_is_fat(dst, ctx.tcx(), ctx.instance()),
                "ERROR: shallow init box used to initialze a fat box!"
            );
            let src = ctx.type_from_cache(src);

            (
                vec![],
                handle_operand(operand, ctx).transmute_on_stack(src, boxed_dst_type, ctx),
            )
        }
        Rvalue::Cast(CastKind::PointerWithExposedProvenance, operand, target) => {
            //FIXME: the documentation of this cast(https://doc.rust-lang.org/nightly/std/ptr/fn.from_exposed_addr.html) is a bit confusing,
            //since this seems to be something deeply linked to the rust memory model.
            // I assume this to be ALWAYS equivalent to `usize as *const/mut T`, but this may not always be the case.
            // If something breaks in the fututre, this is a place that needs checking.
            let target = ctx.monomorphize(*target);
            let target = ctx.type_from_cache(target);
            // Cast from usize/isize to any *T is a NOP, so we just have to load the operand.
            (vec![], handle_operand(operand, ctx).cast_ptr(target))
        }
        Rvalue::Cast(CastKind::PointerExposeProvenance, operand, target) => {
            //FIXME: the documentation of this cast(https://doc.rust-lang.org/nightly/std/primitive.pointer.html#method.expose_addrl) is a bit confusing,
            //since this seems to be something deeply linked to the rust memory model.
            // I assume this to be ALWAYS equivalent to `*const/mut T as usize`, but this may not always be the case.
            // If something breaks in the fututre, this is a place that needs checking.
            let target = ctx.monomorphize(*target);
            let target = ctx.type_from_cache(target);
            // Cast to usize/isize from any *T is a NOP, so we just have to load the operand.

            let val = handle_operand(operand, ctx);
            (
                vec![],
                match target {
                    Type::Int(Int::USize | Int::ISize) | Type::Ptr(_) | Type::FnPtr(_) => {
                        val.cast_ptr(target)
                    }
                    Type::Int(Int::U64 | Int::I64) => crate::casts::int_to_int(
                        Type::Int(Int::USize),
                        target,
                        val.cast_ptr(Type::Int(Int::USize)),
                        ctx,
                    ),
                    _ => todo!("Can't cast using `PointerExposeProvenance` to {target:?}"),
                },
            )
        }
        Rvalue::Cast(CastKind::FloatToFloat, operand, target) => {
            let target = ctx.monomorphize(*target);
            let target = ctx.type_from_cache(target);
            let mut ops = handle_operand(operand, ctx);
            match target {
                Type::Float(Float::F32) => ops = V1Node::ConvF32(ops.into()),
                Type::Float(Float::F64) => ops = V1Node::ConvF64(ops.into()),
                _ => panic!("Can't preform a FloatToFloat cast to type {target:?}"),
            }
            (vec![], ops)
        }
        Rvalue::Cast(
            CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer, _),
            operand,
            _target,
        ) => {
            let operand_ty = operand.ty(ctx.body(), ctx.tcx());
            operand
                .constant()
                .expect("function must be constant in order to take its address!");
            let operand_ty = ctx.monomorphize(operand_ty);

            let (instance, _subst_ref) = if let TyKind::FnDef(def_id, subst_ref) = operand_ty.kind()
            {
                let subst = ctx.monomorphize(*subst_ref);
                let env = rustc_middle::ty::TypingEnv::fully_monomorphized();
                let Some(instance) = Instance::try_resolve(ctx.tcx(), env, *def_id, subst)
                    .expect("Invalid function def")
                else {
                    panic!("ERROR: Could not get function instance. fn type:{operand_ty:?}")
                };
                (instance, subst_ref)
            } else {
                todo!("Trying to call a type which is not a function definition!");
            };
            let function_name = function_name(ctx.tcx().symbol_name(instance));
            let function_sig = crate::function_sig::sig_from_instance_(instance, ctx)
                .expect("Could not get function signature when trying to get a function pointer!");
            //FIXME: properly handle `#[track_caller]`
            let call_site = MethodRef::new(
                *ctx.main_module(),
                ctx.alloc_string(function_name),
                ctx.alloc_sig(function_sig),
                MethodKind::Static,
                vec![].into(),
            );
            (vec![], V1Node::LDFtn(ctx.alloc_methodref(call_site)))
        }

        Rvalue::Discriminant(place) => {
            let addr = place_address(place, ctx);
            let owner_ty = ctx.monomorphize(place.ty(ctx.body(), ctx.tcx()).ty);
            let owner = ctx.type_from_cache(owner_ty);

            let layout = ctx.layout_of(owner_ty);
            let target = ctx.type_from_cache(owner_ty.discriminant_ty(ctx.tcx()));
            let (disrc_type, _) = enum_tag_info(layout.layout, ctx);
            let Type::ClassRef(owner) = owner else {
                eprintln!("Can't get the discirminant of type {owner_ty:?}, because it is a zst. Size:{} Discr type:{:?}",layout.layout.size.bytes(), owner_ty.discriminant_ty(ctx.tcx()));
                return (
                    vec![],
                    crate::casts::int_to_int(
                        Type::Int(Int::I32),
                        target,
                        V1Node::V2(ctx.alloc_node(0_i32)),
                        ctx,
                    ),
                );
            };

            if disrc_type == Type::Void {
                // TODO: This always returns 0 if the discriminat type is `()` - this seems to work, but is incorrect. I should be finding the only inhabited variant instead.
                (
                    vec![],
                    crate::casts::int_to_int(
                        Type::Int(Int::I32),
                        target,
                        V1Node::V2(ctx.alloc_node(0_i32)),
                        ctx,
                    ),
                )
            } else {
                (
                    vec![],
                    crate::casts::int_to_int(
                        disrc_type,
                        target,
                        get_discr(layout.layout, addr, owner, owner_ty, ctx),
                        ctx,
                    ),
                )
            }
        }
        Rvalue::Repeat(operand, times) => repeat(rvalue, ctx, operand, *times, target_location),
        Rvalue::ThreadLocalRef(def_id) => {
            if !def_id.is_local() && ctx.tcx().needs_thread_local_shim(*def_id) {
                let _instance = Instance {
                    def: InstanceKind::ThreadLocalShim(*def_id),
                    args: GenericArgs::empty(),
                };
                // Call instance
                todo!("Thread locals with shims unsupported!")
            } else {
                let alloc_id = ctx.tcx().reserve_and_set_static_alloc(*def_id);
                let rvalue_ty = rvalue.ty(ctx.body(), ctx.tcx());
                let rvalue_type = ctx.type_from_cache(rvalue_ty);
                let tpe = ctx.alloc_type(rvalue_type);
                let ptr = add_allocation(alloc_id.0.into(), ctx, tpe);
                let tpe = ctx[tpe].pointed_to().unwrap();
                (vec![], ctx.cast_ptr(ptr, tpe).into())
            }
        }
        Rvalue::Cast(rustc_middle::mir::CastKind::FnPtrToPtr, operand, target) => {
            let target = ctx.type_from_cache(*target);
            (vec![], handle_operand(operand, ctx).cast_ptr(target))
        }
        rustc_middle::mir::Rvalue::Cast(
            rustc_middle::mir::CastKind::PointerCoercion(
                rustc_middle::ty::adjustment::PointerCoercion::DynStar,
                _,
            ),
            _,
            _,
        ) => todo!("Dyn star casts unspoorted"),
    }
}
const SIMPLE_REPEAT_CAP: u64 = 16;
fn repeat<'tcx>(
    rvalue: &Rvalue<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    element: &Operand<'tcx>,
    times: rustc_middle::ty::Const<'tcx>,
    target_location: &Place<'tcx>,
) -> (Vec<V1Root>, V1Node) {
    // Get the type of the operand
    let element_ty = ctx.monomorphize(element.ty(ctx.body(), ctx.tcx()));
    let element_type = ctx.type_from_cache(element_ty);
    let element = handle_operand(element, ctx);
    // Array size
    let times = ctx.monomorphize(times);
    let times = times
        .try_to_target_usize(ctx.tcx())
        .expect("Could not evalute array size as usize.");
    // Array type
    let array = ctx.monomorphize(rvalue.ty(ctx.body(), ctx.tcx()));
    let array = ctx.type_from_cache(array);
    let array_dotnet = array.clone().as_class_ref().expect("Invalid array type.");
    // Check if the element is byte sized. If so, use initblk to quickly initialize this array.
    if compiletime_sizeof(element_ty, ctx.tcx()) == 1 {
        let place_address = place_address(target_location, ctx);
        let val = Box::new(element.transmute_on_stack(element_type, Type::Int(Int::U8), ctx));
        let init = V1Root::InitBlk {
            dst: Box::new(place_address.cast_ptr(ctx.nptr(Type::Int(Int::U8)))),
            val,
            count: Box::new(V1Node::V2(ctx.alloc_node(Const::USize(times)))),
        };
        return (vec![init], place_get(target_location, ctx));
    }
    // Check if there are more than SIMPLE_REPEAT_CAP elements. If so, use mecmpy to accelerate initialzation
    if times > SIMPLE_REPEAT_CAP {
        let place_address = place_address(target_location, ctx);
        let mut branches = Vec::new();
        let arr_ref = ctx.nref(array);
        let mref = MethodRef::new(
            array_dotnet,
            ctx.alloc_string("set_Item"),
            ctx.sig([arr_ref, Type::Int(Int::USize), element_type], Type::Void),
            MethodKind::Instance,
            vec![].into(),
        );
        let mref = ctx.alloc_methodref(mref);
        for idx in 0..SIMPLE_REPEAT_CAP {
            branches.push(V1Root::Call {
                site: mref,
                args: [
                    place_address.clone(),
                    V1Node::V2(ctx.alloc_node(Const::USize(idx))),
                    element.clone(),
                ]
                .into(),
            });
        }
        let mut curr_len = SIMPLE_REPEAT_CAP;
        while curr_len < times {
            // Copy curr_len elements if possible, otherwise this is the last iteration, so copy the reminder.
            let curr_copy_size = curr_len.min(times - curr_len);
            let elem_size: cilly::Interned<cilly::v2::CILNode> = ctx.size_of(element_type);
            // Copy curr_copy_size elements from the start of the array, starting at curr_len(the amount of already initialized buffers)
            branches.push(V1Root::CpBlk {
                dst: Box::new(
                    V1Node::MRefToRawPtr(Box::new(place_address.clone()))
                        + V1Node::V2(ctx.alloc_node(Const::USize(curr_len)))
                            * conv_usize!(V1Node::V2(elem_size)),
                ),
                src: Box::new(place_address.clone()),
                len: Box::new(
                    V1Node::V2(ctx.alloc_node(Const::USize(curr_copy_size)))
                        * conv_usize!(V1Node::V2(elem_size)),
                ),
            });
            curr_len *= 2;
        }
        let branches: Box<_> = branches.into();
        (*branches, place_get(target_location, ctx))
    } else {
        let mut branches = Vec::new();
        let arr_ref = ctx.nref(array);
        let mref = MethodRef::new(
            array_dotnet,
            ctx.alloc_string("set_Item"),
            ctx.sig([arr_ref, Type::Int(Int::USize), element_type], Type::Void),
            MethodKind::Instance,
            vec![].into(),
        );
        let place_address = place_address(target_location, ctx);
        let mref = ctx.alloc_methodref(mref);
        for idx in 0..times {
            branches.push(V1Root::Call {
                site: mref,
                args: [
                    place_address.clone(),
                    V1Node::V2(ctx.alloc_node(Const::USize(idx))),
                    element.clone(),
                ]
                .into(),
            });
        }
        let branches: Box<_> = branches.into();
        (*branches, place_get(target_location, ctx))
    }
}
fn ptr_to_ptr<'tcx>(
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    operand: &Operand<'tcx>,
    dst: Ty<'tcx>,
) -> V1Node {
    let target = ctx.monomorphize(dst);
    let target_pointed_to = match target.kind() {
        TyKind::RawPtr(typ, _) => typ,
        TyKind::Ref(_, inner, _) => inner,
        _ => panic!("Type is not ptr {target:?}."),
    };
    let source = ctx.monomorphize(operand.ty(ctx.body(), ctx.tcx()));
    let source_pointed_to = match source.kind() {
        TyKind::RawPtr(typ, _) => *typ,
        TyKind::Ref(_, inner, _) => *inner,
        _ => panic!("Type is not ptr {target:?}."),
    };
    let source_type = ctx.type_from_cache(source);
    let target_type = ctx.type_from_cache(target);

    let src_fat = pointer_to_is_fat(source_pointed_to, ctx.tcx(), ctx.instance());
    let target_fat = pointer_to_is_fat(*target_pointed_to, ctx.tcx(), ctx.instance());
    match (src_fat, target_fat) {
        (true, true) => {
            V1Node::transmute_on_stack(handle_operand(operand, ctx), source_type, target_type, ctx)
        }
        (true, false) => {
            let field_desc = FieldDesc::new(
                get_type(source, ctx).as_class_ref().unwrap(),
                ctx.alloc_string(crate::DATA_PTR),
                ctx.nptr(cilly::Type::Void),
            );
            ld_field!(operand_address(operand, ctx), ctx.alloc_field(field_desc))
                .cast_ptr(target_type)
        }
        (false, true) => {
            panic!("ERROR: a non-unsizing cast turned a sized ptr into an unsized one")
        }
        _ => handle_operand(operand, ctx).cast_ptr(target_type),
    }
}
