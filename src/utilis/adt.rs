use cilly::{
    call, cil_node::V1Node, cil_root::V1Root, cilnode::MethodKind, eq, gt_un, Assembly, ClassRef,
    Const, FieldDesc, Float, Int, Interned, MethodRef, Type,
};
use rustc_abi::{FieldIdx, FieldsShape, Layout, LayoutData, TagEncoding, VariantIdx, Variants};
use rustc_middle::ty::Ty;

use rustc_codegen_clr_ctx::MethodCompileCtx;

#[derive(Clone, Debug)]
pub(crate) enum FieldOffsetIterator {
    Explicit { offsets: Box<[u32]>, index: usize },
    NoOffset { count: u64 },
    Empty,
}
impl Iterator for FieldOffsetIterator {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Explicit { offsets, index } => {
                let next = offsets.get(*index);
                *index += 1;
                next.copied()
            }
            Self::NoOffset { count } => {
                if *count > 0 {
                    *count -= 1;
                    Some(0)
                } else {
                    None
                }
            }
            Self::Empty => None,
        }
    }
}
impl FieldOffsetIterator {
    pub fn from_fields_shape(fields: &rustc_abi::FieldsShape<FieldIdx>) -> Self {
        match fields {
            FieldsShape::Arbitrary {
                offsets,
                memory_index,
            } => {
                let offsets: Box<[_]> = memory_index
                    .iter()
                    .enumerate()
                    .map(|(index, _mem_idx)| {
                        u32::try_from(
                            offsets[FieldIdx::from(u32::try_from(index).unwrap())].bytes(),
                        )
                        .unwrap()
                    })
                    //TODO: ask what does field offset of 4294967295 means.
                    .map(|offset| {
                        if offset > u32::from(u16::MAX) {
                            0
                        } else {
                            offset
                        }
                    })
                    .collect();
                FieldOffsetIterator::Explicit { offsets, index: 0 }
            }
            FieldsShape::Union(count) => FieldOffsetIterator::NoOffset {
                count: Into::<usize>::into(*count) as u64,
            },
            FieldsShape::Primitive => Self::Empty,
            FieldsShape::Array { stride, count } => {
                let mut curr: u32 = 0;
                let mut offsets = Vec::new();
                for _ in 0..*count {
                    offsets.push(curr);
                    curr += std::convert::TryInto::<u32>::try_into(stride.bytes())
                        .expect("Array stride too large");
                }
                FieldOffsetIterator::Explicit {
                    offsets: offsets.into(),
                    index: 0,
                }
            }
        }
    }
}
/// Takes layout of an enum as input, and returns the type of its tag(Void if no tag) and the size of the tag(0 if no tag).
pub fn enum_tag_info(r#enum: Layout<'_>, asm: &mut Assembly) -> (Type, u32) {
    match r#enum.variants() {
        Variants::Single { .. } => (
            Type::Void,
            FieldOffsetIterator::from_fields_shape(r#enum.fields())
                .next()
                .unwrap_or(0),
        ),
        Variants::Multiple { tag, tag_field, .. } => (
            scalr_to_type(*tag, asm),
            FieldOffsetIterator::from_fields_shape(r#enum.fields())
                .nth((*tag_field).into())
                .unwrap_or(0),
        ),
        Variants::Empty => (Type::Void, 0),
    }
}
fn scalr_to_type(scalar: rustc_abi::Scalar, asm: &mut Assembly) -> Type {
    let primitive = match scalar {
        rustc_abi::Scalar::Union { value } | rustc_abi::Scalar::Initialized { value, .. } => value,
    };
    primitive_to_type(primitive, asm)
}
fn primitive_to_type(primitive: rustc_abi::Primitive, asm: &mut Assembly) -> Type {
    use rustc_abi::Integer;
    use rustc_abi::Primitive;
    match primitive {
        Primitive::Int(int, sign) => match (int, sign) {
            (Integer::I8, true) => Type::Int(Int::I8),
            (Integer::I16, true) => Type::Int(Int::I16),
            (Integer::I32, true) => Type::Int(Int::I32),
            (Integer::I64, true) => Type::Int(Int::I64),
            (Integer::I128, true) => Type::Int(Int::I128),
            (Integer::I8, false) => Type::Int(Int::U8),
            (Integer::I16, false) => Type::Int(Int::U16),
            (Integer::I32, false) => Type::Int(Int::U32),
            (Integer::I64, false) => Type::Int(Int::U64),
            (Integer::I128, false) => Type::Int(Int::U128),
        },
        Primitive::Float(rustc_abi::Float::F16) => Type::Float(Float::F16),
        Primitive::Float(rustc_abi::Float::F32) => Type::Float(Float::F32),
        Primitive::Float(rustc_abi::Float::F64) => Type::Float(Float::F64),
        Primitive::Float(rustc_abi::Float::F128) => todo!("No support for 128 bit floats yet!"),
        Primitive::Pointer(_) => asm.nptr(Type::Void),
    }
}
pub fn get_variant_at_index(
    variant_index: VariantIdx,
    layout: LayoutData<FieldIdx, rustc_abi::VariantIdx>,
) -> LayoutData<FieldIdx, rustc_abi::VariantIdx> {
    match layout.variants {
        Variants::Single { .. } => layout,
        Variants::Multiple { variants, .. } => variants[variant_index].clone(),
        Variants::Empty => todo!("Empty variants have no variants."),
    }
}
pub fn set_discr<'tcx>(
    layout: Layout<'tcx>,
    variant_index: VariantIdx,
    enum_addr: V1Node,
    enum_tpe: Interned<ClassRef>,
    ty: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Root {
    if get_variant_at_index(variant_index, (*layout.0).clone()).is_uninhabited() {
        // Could be skipped, but keeping a throw here can with CIL correctnes. Each block *must* terminate with a jump, return or a throw.
        // By inserting a throw, we are able to remove all code
        // after it safely.
        return V1Root::throw(
            "UB: SetDiscirminant used, but the specified enum variant is not inhabited.",
            ctx,
        );
    }
    match layout.variants {
        Variants::Empty => V1Root::Nop,
        Variants::Single { index } => {
            assert_eq!(index, variant_index);
            V1Root::Nop
        }
        Variants::Multiple {
            tag_encoding: TagEncoding::Direct,
            ..
        } => {
            let (tag_tpe, _) = enum_tag_info(layout, ctx);
            let tag_val = std::convert::TryInto::<u64>::try_into(
                ty.discriminant_for_variant(ctx.tcx(), variant_index)
                    .unwrap()
                    .val,
            )
            .expect("Enum varaint id can't fit in u64.");
            let tag_val = V1Node::V2(ctx.alloc_node(tag_val));
            let tag_val = crate::casts::int_to_int(Type::Int(Int::U64), tag_tpe, tag_val, ctx);
            let enum_tag_name = ctx.alloc_string(crate::ENUM_TAG);
            V1Root::SetField {
                addr: Box::new(enum_addr),
                value: Box::new(tag_val),
                desc: ctx.alloc_field(FieldDesc::new(enum_tpe, enum_tag_name, tag_tpe)),
            }
        }
        Variants::Multiple {
            tag_encoding:
                TagEncoding::Niche {
                    untagged_variant,
                    ref niche_variants,
                    niche_start,
                },
            ..
        } => {
            if variant_index == untagged_variant {
                V1Root::Nop
            } else {
                let (tag_tpe, _) = enum_tag_info(layout, ctx);
                //let niche = self.project_field(bx, tag_field);
                //let niche_llty = bx.cx().immediate_backend_type(niche.layout);
                let niche_value = variant_index.as_u32() - niche_variants.start().as_u32();
                let niche_value = u128::from(niche_value).wrapping_add(niche_start);
                let tag_val = V1Node::V2(
                    ctx.alloc_node(
                        std::convert::TryInto::<u64>::try_into(niche_value)
                            .expect("Enum varaint id can't fit in u64."),
                    ),
                );
                let tag_val = crate::casts::int_to_int(Type::Int(Int::U64), tag_tpe, tag_val, ctx);
                let enum_tag_name = ctx.alloc_string(crate::ENUM_TAG);
                V1Root::SetField {
                    addr: Box::new(enum_addr),
                    value: Box::new(tag_val),
                    desc: ctx.alloc_field(FieldDesc::new(enum_tpe, enum_tag_name, tag_tpe)),
                }
            }
        }
    }
}

pub fn get_discr<'tcx>(
    layout: Layout<'tcx>,
    enum_addr: V1Node,
    enum_tpe: Interned<ClassRef>,
    ty: Ty<'tcx>,
    ctx: &mut MethodCompileCtx<'tcx, '_>,
) -> V1Node {
    //return CILNode::
    assert!(!layout.is_uninhabited(), "UB: enum layout is unanhibited!");
    let (tag_tpe, _) = enum_tag_info(layout, ctx);
    let tag_encoding = match layout.variants {
        Variants::Single { index } => {
            let discr_val = ty
                .discriminant_for_variant(ctx.tcx(), index)
                .map_or(u128::from(index.as_u32()), |discr| discr.val);
            let tag_val = V1Node::V2(
                ctx.alloc_node(
                    std::convert::TryInto::<u64>::try_into(discr_val)
                        .expect("Tag does not fit within a u64"),
                ),
            );
            return crate::casts::int_to_int(Type::Int(Int::U64), tag_tpe, tag_val, ctx);
        }
        Variants::Multiple {
            ref tag_encoding, ..
        } => tag_encoding,
        Variants::Empty => {
            return crate::casts::int_to_int(
                Type::Int(Int::U64),
                tag_tpe,
                V1Node::V2(ctx.alloc_node(Const::U64(0))),
                ctx,
            )
        }
    };

    // Decode the discriminant (specifically if it's niche-encoded).
    let discr = match *tag_encoding {
        TagEncoding::Direct => {
            if tag_tpe == Type::Void {
                //CILNode::LDOb
                todo!();
            } else {
                let enum_tag_name = ctx.alloc_string(crate::ENUM_TAG);
                V1Node::LDField {
                    field: ctx.alloc_field(FieldDesc::new(enum_tpe, enum_tag_name, tag_tpe)),
                    addr: enum_addr.into(),
                }
            }
        }
        TagEncoding::Niche {
            untagged_variant,
            ref niche_variants,
            niche_start,
        } => {
            let (disrc_type, _) = enum_tag_info(layout, ctx);
            let relative_max = niche_variants.end().as_u32() - niche_variants.start().as_u32();
            let enum_tag_name = ctx.alloc_string(crate::ENUM_TAG);
            let tag = V1Node::LDField {
                field: ctx.alloc_field(FieldDesc::new(enum_tpe, enum_tag_name, disrc_type)),
                addr: enum_addr.into(),
            };
            // We have a subrange `niche_start..=niche_end` inside `range`.
            // If the value of the tag is inside this subrange, it's a
            // "niche value", an increment of the discriminant. Otherwise it
            // indicates the untagged variant.
            // A general algorithm to extract the discriminant from the tag
            // is:
            // relative_tag = tag - niche_start
            // is_niche = relative_tag <= (ule) relative_max
            // discr = if is_niche {
            //     cast(relative_tag) + niche_variants.start()
            // } else {
            //     untagged_variant
            // }
            // However, we will likely be able to emit simpler code.
            let (is_niche, tagged_discr, delta) = if relative_max == 0 {
                // Best case scenario: only one tagged variant. This will
                // likely become just a comparison and a jump.
                // The algorithm is:
                // is_niche = tag == niche_start
                // discr = if is_niche {
                //     niche_start
                // } else {
                //     untagged_variant
                // }
                let main_module = *ctx.main_module();
                let is_niche = match tag_tpe {
                    Type::Int(Int::U128) => {
                        let mref = MethodRef::new(
                            main_module,
                            ctx.alloc_string("eq_u128"),
                            ctx.sig([Type::Int(Int::U128), Type::Int(Int::U128)], Type::Bool),
                            MethodKind::Static,
                            vec![].into(),
                        );
                        call!(
                            ctx.alloc_methodref(mref),
                            [
                                tag,
                                V1Node::const_u128(
                                    u128::from(niche_variants.start().as_u32(),),
                                    ctx
                                )
                            ]
                        )
                    }
                    Type::Int(Int::I128) => {
                        let mref = MethodRef::new(
                            main_module,
                            ctx.alloc_string("eq_i128"),
                            ctx.sig([Type::Int(Int::I128), Type::Int(Int::I128)], Type::Bool),
                            MethodKind::Static,
                            vec![].into(),
                        );
                        call!(
                            ctx.alloc_methodref(mref),
                            [
                                tag,
                                V1Node::const_i128(
                                    u128::from(niche_variants.start().as_u32()),
                                    ctx
                                )
                            ]
                        )
                    }

                    _ => eq!(
                        tag,
                        crate::casts::int_to_int(
                            Type::Int(Int::U64),
                            disrc_type,
                            V1Node::V2(
                                ctx.alloc_node(
                                    std::convert::TryInto::<u64>::try_into(niche_start)
                                        .expect("tag is too big to fit within u64")
                                )
                            ),
                            ctx
                        )
                    ),
                }; //bx.icmp(IntPredicate::IntEQ, tag, niche_start);

                let tagged_discr = crate::casts::int_to_int(
                    Type::Int(Int::U64),
                    disrc_type,
                    V1Node::V2(ctx.alloc_node(u64::from(niche_variants.start().as_u32()))),
                    ctx,
                );
                (is_niche, tagged_discr, 0)
            } else {
                // The special cases don't apply, so we'll have to go with
                // the general algorithm.
                //let tag = crate::casts::int_to_int(disrc_type.clone(), &Type::Int(Int::U64), tag);
                let relative_discr = match tag_tpe {
                    Type::Int(Int::I128 | Int::U128) => {
                        todo!("niche encoidng of 128 bit wide tags is not fully supported yet")
                    }
                    _ => V1Node::Sub(
                        Box::new(tag),
                        Box::new(crate::casts::int_to_int(
                            Type::Int(Int::U64),
                            disrc_type,
                            V1Node::V2(
                                ctx.alloc_node(
                                    std::convert::TryInto::<u64>::try_into(niche_start)
                                        .expect("tag is too big to fit within u64"),
                                ),
                            ),
                            ctx,
                        )),
                    ),
                };
                let gt = match tag_tpe {
                    Type::Int(Int::U128) => {
                        let mref = MethodRef::new(
                            ClassRef::uint_128(ctx),
                            ctx.alloc_string("op_GreaterThan"),
                            ctx.sig([Type::Int(Int::U128), Type::Int(Int::U128)], Type::Bool),
                            MethodKind::Static,
                            vec![].into(),
                        );
                        call!(
                            ctx.alloc_methodref(mref),
                            [
                                relative_discr.clone(),
                                V1Node::const_u128(u128::from(relative_max), ctx)
                            ]
                        )
                    }
                    Type::Int(Int::I128) => {
                        let mref = MethodRef::new(
                            ClassRef::int_128(ctx),
                            ctx.alloc_string("op_GreaterThan"),
                            ctx.sig([Type::Int(Int::I128), Type::Int(Int::I128)], Type::Bool),
                            MethodKind::Static,
                            vec![].into(),
                        );
                        call!(
                            ctx.alloc_methodref(mref),
                            [
                                relative_discr.clone(),
                                V1Node::const_i128(u128::from(relative_max), ctx)
                            ]
                        )
                    }

                    _ => gt_un!(
                        relative_discr.clone(),
                        crate::casts::int_to_int(
                            Type::Int(Int::U64),
                            disrc_type,
                            V1Node::V2(ctx.alloc_node(u64::from(relative_max))),
                            ctx
                        )
                    ),
                };
                let is_niche = eq!(gt, V1Node::V2(ctx.alloc_node(false)));
                (
                    is_niche,
                    relative_discr,
                    u128::from(niche_variants.start().as_u32()),
                )
            };

            let tagged_discr = if delta == 0 {
                tagged_discr
            } else {
                let delta = crate::casts::int_to_int(
                    Type::Int(Int::U64),
                    disrc_type,
                    V1Node::V2(
                        ctx.alloc_node(
                            std::convert::TryInto::<u64>::try_into(delta)
                                .expect("Tag does not fit within u64"),
                        ),
                    ),
                    ctx,
                );
                assert!(matches!(
                    disrc_type,
                    Type::Int(
                        Int::U8
                            | Int::I8
                            | Int::U16
                            | Int::I16
                            | Int::U32
                            | Int::I32
                            | Int::U64
                            | Int::I64
                            | Int::USize
                            | Int::ISize
                    ) | Type::Ptr(_)
                ));
                tagged_discr + delta
            };

            // In principle we could insert assumes on the possible range of `discr`, but
            // currently in LLVM this seems to be a pessimization.

            V1Node::select(
                disrc_type,
                tagged_discr,
                crate::casts::int_to_int(
                    Type::Int(Int::U64),
                    disrc_type,
                    V1Node::V2(ctx.alloc_node(u64::from(untagged_variant.as_u32()))),
                    ctx,
                ),
                is_niche,
                ctx,
            )
        }
    };
    discr

    //discr
}
