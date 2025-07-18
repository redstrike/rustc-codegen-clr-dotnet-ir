use core::f16;
use std::collections::HashSet;

use cilly::{
    Assembly, CILNode, ClassRef, Const, Float, Int, Interned, IntoAsmIndex, MethodRef,
    StaticFieldDesc, Type, call,
    cil_node::{CallOpArgs, V1Node},
    cilnode::{IsPure, MethodKind},
    hashable::{HashableF32, HashableF64},
};
use rustc_codegen_clr_call::CallInfo;
use rustc_codegen_clr_ctx::MethodCompileCtx;
use rustc_codegen_clr_type::{GetTypeExt, r#type::fixed_array, utilis::is_fat_ptr};
use rustc_middle::ty::ExistentialTraitRef;
use rustc_middle::{
    mir::{
        ConstOperand, ConstValue,
        interpret::Scalar,
        interpret::{AllocId, GlobalAlloc},
    },
    ty::{FloatTy, IntTy, Ty, TyCtxt, TyKind, UintTy},
};
use rustc_span::def_id::DefId;

use crate::static_data::add_allocation;
pub fn handle_constant<'tcx>(
    constant_op: &ConstOperand<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Node {
    let constant = constant_op.const_;
    let constant = ctx.monomorphize(constant);
    let evaluated = constant
        .eval(
            ctx.tcx(),
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
            constant_op.span,
        )
        .expect("Could not evaluate constant!");
    load_const_value(evaluated, constant.ty(), ctx)
}

/// Returns the ops neceasry to create constant value of type `ty` with byte values matching the ones in the allocation
fn create_const_from_data<'tcx>(
    ty: Ty<'tcx>,
    alloc_id: AllocId,
    offset_bytes: u64,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> Interned<CILNode> {
    let _ = offset_bytes;
    let ty = ctx.monomorphize(ty);
    let tpe = ctx.type_from_cache(ty);
    let tpe_idx = ctx.alloc_type(tpe);
    // Optimization - check if this can be replaced by a scalar.
    if let GlobalAlloc::Memory(alloc) = ctx.tcx().global_alloc(alloc_id) {
        let const_allocation = alloc.inner();
        let align = const_allocation.align.bytes().max(1);
        let mut bytes: Vec<u8> = const_allocation
            .inspect_with_uninit_and_ptr_outside_interpreter(0..const_allocation.len())
            .into();
        // Right aligment, fits, and has no pointers - can be a scalar.
        if align <= 8 && bytes.len() <= 16 && const_allocation.provenance().ptrs().is_empty() {
            while bytes.len() < 16 {
                bytes.push(0);
            }
            let scalar =
                Scalar::from_u128(u128::from_ne_bytes(bytes.as_slice().try_into().unwrap()));
            return load_const_scalar(scalar, ty, ctx).into();
        }
        let (ptr, align) = alloc_ptr_unaligned(alloc_id, &alloc, ctx, tpe_idx);
        let ty = ctx.monomorphize(ty);

        let tpe = ctx.type_from_cache(ty);
        let ptr = ctx.cast_ptr(ptr, tpe);
        if align.is_none() {
            return ctx.load(ptr, tpe);
        } else {
            let unaligned_read = Interned::unaligned_read(ctx, tpe);
            return ctx.call(unaligned_read, &[ptr], IsPure::NOT);
        }
    }

    let ptr = add_allocation(alloc_id.0.into(), ctx, tpe_idx);
    let ptr = ctx.cast_ptr(ptr, tpe);
    return ctx.load(ptr, tpe);
}
fn alloc_align_size(alloc_id: u64, ctx: &mut MethodCompileCtx<'_, '_>) -> (u64, u64) {
    let global_alloc = ctx
        .tcx()
        .global_alloc(AllocId(alloc_id.try_into().expect("0 alloc id?")));
    match global_alloc {
        GlobalAlloc::Memory(alloc) => (alloc.0.0.align.bytes(), alloc.0.len() as u64),
        _ => todo!(),
    }
}
fn const_slice_backer_type<'tcx>(
    const_ty: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    alloc_id: u64,
    meta: u64,
) -> Type {
    let elem = match const_ty.kind() {
        TyKind::Str => Type::Int(Int::U8),
        TyKind::Adt(def, generics) => {
            assert_eq!(
                def.all_fields().count(),
                1,
                "DSTs in slice constants must have exactly one field!"
            );
            let fld = def.all_fields().next().unwrap();
            return const_slice_backer_type(fld.ty(ctx.tcx(), generics), ctx, alloc_id, meta);
        }
        TyKind::Slice(elem) => ctx.type_from_cache(*elem),
        _ => todo!("Unhandled const {const_ty:?}"),
    };
    if meta == 1 {
        return elem;
    }
    let (align, arr_size) = alloc_align_size(alloc_id, ctx);
    let arr_tpe = fixed_array(ctx, elem, meta, arr_size, align);
    Type::ClassRef(arr_tpe)
}
pub fn load_const_value<'tcx>(
    const_val: ConstValue<'tcx>,
    const_ty: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Node {
    match const_val {
        ConstValue::Scalar(scalar) => load_const_scalar(scalar, const_ty, ctx).into(),
        ConstValue::ZeroSized => {
            let tpe = ctx.monomorphize(const_ty);
            assert!(
                rustc_codegen_clr_type::utilis::is_zst(tpe, ctx.tcx()),
                "Zero sized const with a non-zero size. It is {tpe:?}"
            );
            let tpe = ctx.type_from_cache(tpe);
            V1Node::uninit_val(tpe, ctx)
        }
        ConstValue::Slice { data, meta } => {
            let slice_type = ctx.type_from_cache(const_ty);
            let slice_dotnet = slice_type.as_class_ref().expect("Slice type invalid!");

            let alloc_id = ctx.tcx().reserve_and_set_memory_alloc(data);

            let ptr = if meta == 0 {
                ctx.alloc_node(Const::USize(1 << 30))
            } else {
                let arr_type = const_slice_backer_type(
                    const_ty.builtin_deref(true).unwrap(),
                    ctx,
                    alloc_id.0.get(),
                    meta,
                );

                let arr_tpe = ctx.alloc_type(arr_type);

                alloc_ptr(alloc_id, &data, ctx, arr_tpe)
            };
            let ptr = ctx.cast_ptr(ptr, Type::Void);
            let meta = V1Node::V2(ctx.alloc_node(Const::USize(meta)));
            V1Node::create_slice(slice_dotnet, ctx, meta, ptr.into())
        }
        ConstValue::Indirect { alloc_id, offset } => {
            create_const_from_data(const_ty, alloc_id, offset.bytes(), ctx).into()
            //todo!("Can't handle by-ref allocation {alloc_id:?} {offset:?}")
        } //_ => todo!("Unhandled const value {const_val:?} of type {const_ty:?}"),
    }
}
pub fn static_ty<'tcx>(def_id: DefId, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    tcx.type_of(def_id).instantiate_identity()
}
fn load_scalar_ptr(
    ctx: &mut MethodCompileCtx<'_, '_>,
    ptr: rustc_middle::mir::interpret::Pointer,
    tpe: Interned<Type>,
) -> Interned<CILNode> {
    let (alloc_id, offset) = ptr.into_parts();
    let global_alloc = ctx.tcx().global_alloc(alloc_id.alloc_id());
    let u8_ptr = ctx.nptr(Type::Int(Int::U8));
    let u8_ptr_ptr = ctx.nptr(u8_ptr);
    match global_alloc {
        GlobalAlloc::Static(def_id) => {
            assert!(ctx.tcx().is_static(def_id));
            assert_eq!(offset.bytes(), 0);
            let name = ctx
                .tcx()
                .opt_item_name(def_id)
                .expect("Static without name")
                .to_string();
            /* */
            if name == "__rust_alloc_error_handler_should_panic"
                || name == "__rust_no_alloc_shim_is_unstable"
            {
                let stotic = StaticFieldDesc::new(
                    *ctx.main_module(),
                    ctx.alloc_string(name),
                    Type::Int(Int::U8),
                );
                return ctx.static_addr(stotic);
            }
            if name == "environ" {
                let mref = MethodRef::new(
                    *ctx.main_module(),
                    ctx.alloc_string("get_environ"),
                    ctx.sig([], u8_ptr_ptr),
                    MethodKind::Static,
                    vec![].into(),
                );
                let mref = ctx.alloc_methodref(mref);
                let environ = ctx.alloc_node(cilly::v2::CILNode::call(mref, []));
                let environ = ctx.annon_const(environ);
                return ctx.alloc_node(cilly::v2::CILNode::LdStaticFieldAddress(environ));
            }
            let attrs = ctx.tcx().codegen_fn_attrs(def_id);

            if attrs.import_linkage.is_some() {
                // TODO: this could cause issues if the pointer to the static is not imediatly dereferenced.
                let site = get_fn_from_static_name(&name, ctx);
                let cst = ctx.annon_const(cilly::v2::CILNode::LdFtn(site));
                return ctx.alloc_node(cilly::v2::CILNode::LdStaticFieldAddress(cst));
            }
            if let Some(section) = attrs.link_section {
                panic!("static {name} requires special linkage in section {section:?}");
            }
            let alloc = ctx
                .tcx()
                .eval_static_initializer(def_id)
                .expect("No initializer??");
            //def_id.ty();
            let _memory = ctx.tcx().reserve_and_set_memory_alloc(alloc);
            let alloc_id = alloc_id.alloc_id().0.into();
            add_allocation(alloc_id, ctx, tpe)
        }
        GlobalAlloc::Memory(const_allocation) => {
            let ptr = alloc_ptr(alloc_id.alloc_id(), &const_allocation, ctx, tpe);
            if offset.bytes() != 0 {
                ctx.biop(ptr, cilly::Const::USize(offset.bytes()), cilly::BinOp::Add)
            } else {
                ptr
            }
        }
        GlobalAlloc::Function {
            instance: finstance,
        } => {
            assert_eq!(offset.bytes(), 0);
            // If it is a function, patch its pointer up.
            let call_info = CallInfo::sig_from_instance_(finstance, ctx);
            let function_name =
                rustc_codegen_clr_ctx::function_name(ctx.tcx().symbol_name(finstance));
            let mref = MethodRef::new(
                *ctx.main_module(),
                ctx.alloc_string(function_name),
                ctx.alloc_sig(call_info.sig().clone()),
                MethodKind::Static,
                vec![].into(),
            );
            CILNode::LdFtn(ctx.alloc_methodref(mref)).into_idx(ctx)
        }
        GlobalAlloc::VTable(_, _) => {
            let (ty, polyref) = global_alloc.unwrap_vtable();
            get_vtable(
                ctx,
                ctx.monomorphize(ty),
                polyref.map(|principal| ctx.tcx().instantiate_bound_regions_with_erased(principal)),
            )
        }
    }
    //panic!("alloc_id:{alloc_id:?}")
}
/// Returns a pointer to an immutable buffer, representing a given allocation.
fn alloc_ptr<'tcx>(
    alloc_id: AllocId,
    const_alloc: &rustc_middle::mir::interpret::ConstAllocation,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    tpe: Interned<Type>,
) -> Interned<CILNode> {
    let (ptr, align) = alloc_ptr_unaligned(alloc_id, const_alloc, ctx, tpe);
    // If aligement is small enough to be *guaranteed*, and no pointers are present.
    if align.is_some_and(|align| align <= ctx.const_align()) {
        add_allocation(alloc_id.0.into(), ctx, tpe)
    } else {
        ptr
    }
}
/// Returns a pointer to an immutable buffer, representing a given allocation. Pointer may be underaligned, aligement of `u64::MAX` signals that the pointer
/// will be sufficently aligned for `const_alloc`.
fn alloc_ptr_unaligned<'tcx>(
    alloc_id: AllocId,
    const_alloc: &rustc_middle::mir::interpret::ConstAllocation,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
    tpe: Interned<Type>,
) -> (Interned<CILNode>, Option<u64>) {
    let const_alloc = const_alloc.inner();
    // If aligement is small enough to be *guaranteed*, and no pointers are present.
    if const_alloc.provenance().ptrs().is_empty() {
        if const_alloc.align.bytes() <= ctx.const_align() {
            (
                ctx.bytebuffer(
                    const_alloc
                        .inspect_with_uninit_and_ptr_outside_interpreter(0..const_alloc.len()),
                    Int::U8,
                ),
                None,
            )
        } else {
            //unaligned_read
            (
                ctx.bytebuffer(
                    const_alloc
                        .inspect_with_uninit_and_ptr_outside_interpreter(0..const_alloc.len()),
                    Int::U8,
                ),
                Some(ctx.const_align()),
            )
        }
    } else {
        (add_allocation(alloc_id.0.into(), ctx, tpe), None)
    }
}
fn load_const_scalar<'tcx>(
    scalar: Scalar,
    scalar_type: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> Interned<CILNode> {
    let scalar_ty = ctx.monomorphize(scalar_type);
    let scalar_type = ctx.type_from_cache(scalar_ty);

    let scalar_u128 = match scalar {
        Scalar::Int(scalar_int) => scalar_int.to_uint(scalar.size()),
        Scalar::Ptr(ptr, _size) => {
            let const_type = scalar_ty
                .builtin_deref(true)
                .map(|ty| ctx.type_from_cache(ty))
                .unwrap_or(Int::USize.into());
            let const_type_idx = ctx.alloc_type(const_type);
            let ptr = load_scalar_ptr(ctx, ptr, const_type_idx);

            if matches!(scalar_type, Type::Ptr(_)) {
                return ctx.cast_ptr(ptr, const_type_idx);
            } else if matches!(scalar_type, Type::FnPtr(_)) {
                return ptr;
            }
            let src_ptr = ctx.nptr(Int::U8);
            let ptr = ctx.cast_ptr(ptr, Int::U8);
            return ctx.transmute_on_stack(src_ptr, scalar_type, ptr);
        }
    };

    match scalar_ty.kind() {
        TyKind::Int(int_type) => load_const_int(scalar_u128, *int_type, ctx),
        TyKind::Uint(uint_type) => load_const_uint(scalar_u128, *uint_type, ctx),
        TyKind::Float(ftype) => load_const_float(scalar_u128, *ftype, ctx).into(),
        TyKind::Bool => ctx.alloc_node(scalar_u128 != 0),
        TyKind::RawPtr(..) | TyKind::Ref(..) => {
            if is_fat_ptr(scalar_ty, ctx.tcx(), ctx.instance()) {
                let val = ctx.alloc_node(scalar_u128);
                ctx.transmute_on_stack(Type::Int(Int::U128), scalar_type, val)
            } else {
                let val = ctx.alloc_node(Const::USize(
                    u64::try_from(scalar_u128).expect("pointers must be smaller than 2^64"),
                ));
                let const_type = scalar_ty
                    .builtin_deref(true)
                    .map(|ty| ctx.type_from_cache(ty))
                    .unwrap_or(Int::USize.into());
                ctx.cast_ptr(val, const_type)
            }
        }
        TyKind::Tuple(elements) => {
            if elements.is_empty() {
                let scalar_ptr = ctx.nptr(scalar_type);
                ctx.uninit_val(scalar_ptr)
            } else {
                let val = ctx.alloc_node(scalar_u128);
                ctx.transmute_on_stack(Type::Int(Int::U128), scalar_type, val)
            }
        }
        TyKind::Adt(_, _) | TyKind::Closure(_, _) | TyKind::Array(_, _) => {
            let val = ctx.alloc_node(scalar_u128);
            ctx.transmute_on_stack(Type::Int(Int::U128), scalar_type, val)
        }
        TyKind::Char => ctx.alloc_node(u32::try_from(scalar_u128).unwrap()),
        _ => todo!("Can't load scalar constants of type {scalar_ty:?}!"),
    }
}
fn load_const_float(
    value: u128,
    float_type: FloatTy,
    asm: &mut Assembly,
) -> Interned<cilly::v2::CILNode> {
    match float_type {
        FloatTy::F16 => {
            #[cfg(not(target_family = "windows"))]
            {
                let mref = MethodRef::new(
                    ClassRef::half(asm),
                    asm.alloc_string("op_Explicit"),
                    asm.sig([Type::Float(Float::F32)], Type::Float(Float::F16)),
                    MethodKind::Static,
                    vec![].into(),
                );
                let cst = asm.alloc_node(Const::F32(HashableF32(
                    (f16::from_ne_bytes((u16::try_from(value).unwrap()).to_ne_bytes())) as f32,
                )));
                asm.call(mref, &[cst], IsPure::PURE)
            }
            #[cfg(target_family = "windows")]
            {
                todo!("building a program using 16 bit floats is not supported on windwows yet")
                // TODO: check if this still causes a linker error on windows
            }
        }
        FloatTy::F32 => {
            let value = f32::from_ne_bytes((u32::try_from(value).unwrap()).to_ne_bytes());
            asm.alloc_node(Const::F32(HashableF32(value))).into()
        }
        FloatTy::F64 => {
            let value = f64::from_ne_bytes((u64::try_from(value).unwrap()).to_ne_bytes());
            asm.alloc_node(Const::F64(HashableF64(value))).into()
        }
        FloatTy::F128 => {
            let u128_const = asm.alloc_node(Const::U128(value));
            asm.transmute_on_stack(Type::Int(Int::U128), Type::Float(Float::F128), u128_const)
        }
    }
}
pub fn load_const_int(
    value: u128,
    int_type: IntTy,
    asm: &mut Assembly,
) -> Interned<cilly::v2::CILNode> {
    match int_type {
        IntTy::I8 => asm.alloc_node(i8::from_ne_bytes([u8::try_from(value).unwrap()])),
        IntTy::I16 => asm.alloc_node(i16::from_ne_bytes(
            (u16::try_from(value).unwrap()).to_ne_bytes(),
        )),
        IntTy::I32 => asm.alloc_node(i32::from_ne_bytes(
            (u32::try_from(value).unwrap()).to_ne_bytes(),
        )),
        IntTy::I64 => asm.alloc_node(i64::from_ne_bytes(
            (u64::try_from(value).unwrap()).to_ne_bytes(),
        )),
        IntTy::Isize => asm.alloc_node(cilly::Const::ISize(i64::from_ne_bytes(
            (u64::try_from(value).unwrap()).to_ne_bytes(),
        ))),
        #[allow(clippy::cast_possible_wrap)]
        IntTy::I128 => asm.alloc_node(value as i128),
    }
}
pub fn load_const_uint(
    value: u128,
    int_type: UintTy,
    asm: &mut Assembly,
) -> Interned<cilly::v2::CILNode> {
    match int_type {
        UintTy::U8 => asm.alloc_node(u8::try_from(value).unwrap()),
        UintTy::U16 => asm.alloc_node(u16::try_from(value).unwrap()),
        UintTy::U32 => asm.alloc_node(u32::try_from(value).unwrap()),
        UintTy::U64 => asm.alloc_node(u64::try_from(value).unwrap()),
        UintTy::Usize => asm.alloc_node(cilly::Const::USize(u64::try_from(value).unwrap())),
        UintTy::U128 => asm.alloc_node(value),
    }
}

fn get_fn_from_static_name(name: &str, ctx: &mut MethodCompileCtx<'_, '_>) -> Interned<MethodRef> {
    let int8_ptr = ctx.nptr(Type::Int(Int::I8));
    let int64_ptr = ctx.nptr(Type::Int(Int::I64));
    let void_ptr = ctx.nptr(Type::Void);
    let int8_ptr_ptr = ctx.nptr(int8_ptr);
    let mref = match name {
        "statx" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("statx"),
            ctx.sig(
                [
                    Type::Int(Int::I32),
                    int8_ptr,
                    Type::Int(Int::I32),
                    Type::Int(Int::U32),
                    void_ptr,
                ],
                Type::Int(Int::I32),
            ),
            MethodKind::Static,
            vec![].into(),
        ),
        "getrandom" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("getrandom"),
            ctx.sig(
                [int8_ptr, Type::Int(Int::USize), Type::Int(Int::U32)],
                Type::Int(Int::USize),
            ),
            MethodKind::Static,
            vec![].into(),
        ),
        "posix_spawn" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("posix_spawn"),
            ctx.sig(
                [int8_ptr, int8_ptr, int8_ptr, int8_ptr, int8_ptr, int8_ptr],
                Type::Int(Int::I32),
            ),
            MethodKind::Static,
            vec![].into(),
        ),
        "posix_spawn_file_actions_addchdir_np" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("posix_spawn_file_actions_addchdir_np"),
            ctx.sig([int8_ptr, int8_ptr], Type::Int(Int::I32)),
            MethodKind::Static,
            vec![].into(),
        ),
        "__dso_handle" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("__dso_handle"),
            ctx.sig([], Type::Void),
            MethodKind::Static,
            vec![].into(),
        ),
        "__cxa_thread_atexit_impl" => {
            let fn_ptr_sig = Type::FnPtr(ctx.sig([void_ptr], Type::Void));
            MethodRef::new(
                *ctx.main_module(),
                ctx.alloc_string("__cxa_thread_atexit_impl"),
                ctx.sig([fn_ptr_sig, void_ptr, void_ptr], Type::Void),
                MethodKind::Static,
                vec![].into(),
            )
        }
        "copy_file_range" => {
            let i64_ptr = ctx.nptr(Type::Int(Int::I64));
            MethodRef::new(
                *ctx.main_module(),
                ctx.alloc_string("copy_file_range"),
                ctx.sig(
                    [
                        Type::Int(Int::I32),
                        int64_ptr,
                        Type::Int(Int::I32),
                        i64_ptr,
                        Type::Int(Int::ISize),
                        Type::Int(Int::U32),
                    ],
                    Type::Int(Int::ISize),
                ),
                MethodKind::Static,
                vec![].into(),
            )
        }
        "pidfd_spawnp" => {
            let i32_ptr = ctx.nptr(Type::Int(Int::I32));
            let i8_ptr = ctx.nptr(Type::Int(Int::I8));
            MethodRef::new(
                *ctx.main_module(),
                ctx.alloc_string("pidfd_spawnp"),
                ctx.sig(
                    [
                        i32_ptr,
                        i8_ptr,
                        void_ptr,
                        void_ptr,
                        int8_ptr_ptr,
                        int8_ptr_ptr,
                    ],
                    Type::Int(Int::I32),
                ),
                MethodKind::Static,
                vec![].into(),
            )
        }
        "pidfd_getpid" => MethodRef::new(
            *ctx.main_module(),
            ctx.alloc_string("pidfd_getpid"),
            ctx.sig([Type::Int(Int::I32)], Type::Int(Int::I32)),
            MethodKind::Static,
            vec![].into(),
        ),
        _ => {
            todo!("Unsuported function refered to using a weak static. Function name is {name:?}.")
        }
    };
    ctx.alloc_methodref(mref)
}
pub fn get_vtable<'tcx>(
    fx: &mut MethodCompileCtx<'tcx, '_>,
    ty: Ty<'tcx>,
    trait_ref: Option<ExistentialTraitRef<'tcx>>,
) -> Interned<cilly::v2::CILNode> {
    let ty = fx.monomorphize(ty);

    let alloc_id = fx.tcx().vtable_allocation((ty, trait_ref));
    let vtable_len = fx
        .tcx()
        .vtable_entries(
            trait_ref
                .expect("None trait ref :(")
                .with_self_ty(fx.tcx(), ty),
        )
        .len();
    let tpe = fixed_array(
        fx,
        cilly::Type::Int(Int::USize),
        vtable_len.try_into().unwrap(),
        vtable_len as u64 * 8,
        8,
    );
    let tpe = fx.alloc_type(Type::ClassRef(tpe));
    add_allocation(alloc_id.0.get(), fx, tpe)
}
