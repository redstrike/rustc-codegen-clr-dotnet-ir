use crate::bimap::Interned;
use crate::cilnode::IsPure;
use crate::cilnode::MethodKind;
use crate::v2::method::LocalDef;

use crate::FieldDesc;
use crate::{
    call,
    cil_root::V1Root,
    hashable::{HashableF32, HashableF64},
    IString,
};
use crate::{Assembly, ClassRef, FnSig, Int, MethodRef, StaticFieldDesc, Type};
use serde::{Deserialize, Serialize};
/// A container for the arguments of a call, callvirt, or newobj instruction.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Hash, Debug)]
pub struct CallOpArgs {
    pub args: Box<[V1Node]>,
    pub site: Interned<MethodRef>,
    pub is_pure: IsPure,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Hash)]
pub enum V1Node {
    /// A translated V2 node.
    V2(Interned<crate::v2::CILNode>),
    /// Loads the value of local variable number `n`.
    LDLoc(u32),

    /// A black box that prevents the bulit-in optimization engine from doing any optimizations.
    BlackBox(Box<Self>),
    /// Converts the signed inner value to a 32 bit floating-point number.
    ConvF32(Box<Self>),
    /// Converts the signed inner value to a 64 bit floating-point number.
    ConvF64(Box<Self>),
    /// Converts the unsigned inner value to a 64 bit floating-point number.
    ConvF64Un(Box<Self>),

    /// Loads a i8 from a pointer
    LDIndI8 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a bool from a pointer
    LDIndBool {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a i16 from a pointer
    LDIndI16 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a i32 from a pointer
    LDIndI32 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a i64 from a pointer
    LDIndI64 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a isize from a pointer
    LDIndISize {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a isize from a pointer
    // 24 bytes, consider shrinking?
    LDIndPtr {
        /// Address of the value
        ptr: Box<Self>,
        loaded_ptr: Box<Type>,
    },
    /// Loads a isize from a pointer
    LDIndUSize {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads an object from a pointer
    // 24 bytes, consider shrinking?
    LdObj {
        /// Address of the value
        ptr: Box<Self>,
        /// Type of the loaded value
        obj: Box<Type>,
    },
    /// Loads a f32 from a pointer
    LDIndF32 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads a f64 from a pointer
    LDIndF64 {
        /// Address of the value
        ptr: Box<Self>,
    },
    /// Loads the address of `field` of the object `addr` points to
    LDFieldAdress {
        /// Address of the object
        addr: Box<Self>,
        field: Interned<FieldDesc>,
    },
    /// Loads the value of `field` of the object `addr` points to
    LDField {
        /// Address of the object
        addr: Box<Self>,
        field: Interned<FieldDesc>,
    },
    /// Adds 2 values together
    Add(Box<Self>, Box<Self>),
    /// Binary-ands 2 values together
    And(Box<Self>, Box<Self>),
    /// Subtracts lhs from rhs
    Sub(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    DivUn(Box<Self>, Box<Self>),
    Rem(Box<Self>, Box<Self>),
    RemUn(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    XOr(Box<Self>, Box<Self>),
    Shr(Box<Self>, Box<Self>),
    Shl(Box<Self>, Box<Self>),
    ShrUn(Box<Self>, Box<Self>),
    Call(Box<CallOpArgs>),
    CallVirt(Box<CallOpArgs>),

    ConvU8(Box<Self>),
    ConvU16(Box<Self>),
    ConvU32(Box<Self>),
    ZeroExtendToU64(Box<Self>),
    ZeroExtendToUSize(Box<Self>),
    ZeroExtendToISize(Box<Self>),
    MRefToRawPtr(Box<Self>),
    ConvI8(Box<Self>),
    ConvI16(Box<Self>),
    ConvI32(Box<Self>),
    SignExtendToI64(Box<Self>),
    SignExtendToU64(Box<Self>),
    SignExtendToISize(Box<Self>),
    SignExtendToUSize(Box<Self>),
    Neg(Box<Self>),
    Not(Box<Self>),
    Eq(Box<Self>, Box<Self>),
    Lt(Box<Self>, Box<Self>),
    /// Compares two operands, returning true if lhs < rhs. Unsigned for intigers, unordered(in respect to NaNs) for floats.
    LtUn(Box<Self>, Box<Self>),
    Gt(Box<Self>, Box<Self>),
    /// Compares two operands, returning true if lhs < rhs. Unsigned for intigers, unordered(in respect to NaNs) for floats.
    GtUn(Box<Self>, Box<Self>),

    LDFtn(Interned<MethodRef>),
    LDTypeToken(Box<Type>),
    NewObj(Box<CallOpArgs>),
    // 24 bytes - too big!
    LdStr(IString),
    CallI(Box<(FnSig, Self, Box<[Self]>)>),
    LDIndU8 {
        ptr: Box<Self>,
    },
    LDIndU16 {
        ptr: Box<Self>,
    },
    LDIndU32 {
        ptr: Box<Self>,
    },
    LDIndU64 {
        ptr: Box<Self>,
    },
    /// Loads the length of an array - as a nint.
    LDLen {
        arr: Box<Self>,
    },
    /// Loads an object reference from a managed array
    // 24 bytes - too big!
    LDElelemRef {
        arr: Box<Self>,
        idx: Box<Self>,
    },

    /// Tells the codegen a pointer value type is changed. Used during verification, to implement things like`transmute`.
    CastPtr {
        val: Box<Self>,
        new_ptr: Box<Type>,
    },

    /// Allocates a buffer of size at least `sizeof(tpe)` with aligement of `align`
    LocAllocAligned {
        tpe: Box<Type>,
        align: u64,
    },
    /// Allocates a local buffer of size
    LocAlloc {
        size: Box<Self>,
    },
    /// Gets the exception. Can only be used in handlers, only once per handler.
    GetException,
    /// Checks if `lhs` is of type `rhs`. If not, throws.
    CheckedCast(Box<(V1Node, Interned<ClassRef>)>),
    // Checks if `lhs` is of type `rhs`.  Returns a boolean.
    IsInst(Box<(V1Node, Interned<ClassRef>)>),
    /// Marks the inner pointer operation as volatile.
    Volatile(Box<Self>),
    UnboxAny(Box<Self>, Box<Type>),
}
impl From<Interned<crate::v2::CILNode>> for V1Node {
    fn from(value: Interned<crate::v2::CILNode>) -> Self {
        Self::V2(value)
    }
}
impl V1Node {
    pub fn stack_addr(val: Self, _: Interned<Type>, asm: &mut Assembly) -> Self {
        let val = crate::v2::CILNode::from_v1(&val, asm);
        let sfld = asm.annon_const(val);
        V1Node::V2(asm.alloc_node(crate::v2::CILNode::LdStaticFieldAddress(sfld)))
    }
    pub fn ovf_check_tuple(
        asm: &mut Assembly,
        tuple: Interned<ClassRef>,
        out_of_range: Self,
        val: Self,
        tpe: Type,
    ) -> V1Node {
        let main = asm.main_module();
        let sig = asm.sig([tpe, Type::Bool], Type::ClassRef(tuple));
        let site = asm.new_methodref(*main, "ovf_check_tuple", sig, MethodKind::Static, []);
        V1Node::Call(Box::new(CallOpArgs {
            args: vec![val, out_of_range].into(),
            site,
            is_pure: IsPure::PURE,
        }))
    }
    pub fn create_slice(
        slice_tpe: Interned<ClassRef>,
        asm: &mut Assembly,
        metadata: Self,
        ptr: Self,
    ) -> Self {
        let void_ptr = asm.nptr(Type::Void);
        let main = asm.main_module();
        let sig = asm.sig([void_ptr, Type::Int(Int::USize)], Type::ClassRef(slice_tpe));
        let create_slice = asm.new_methodref(*main, "create_slice", sig, MethodKind::Static, []);
        V1Node::Call(Box::new(CallOpArgs {
            args: vec![ptr, metadata].into(),
            site: create_slice,
            is_pure: IsPure::PURE,
        }))
    }
    pub fn const_u128(value: u128, asm: &mut Assembly) -> V1Node {
        V1Node::V2(asm.alloc_node(value))
    }
    pub fn const_i128(value: u128, asm: &mut Assembly) -> V1Node {
        V1Node::V2(asm.alloc_node(value as i128))
    }
    /// Allocates a GC handle to the object, and converts that handle to a nint sized handleID.
    pub fn managed_ref_to_handle(self, asm: &mut Assembly) -> Self {
        let gc_handle_class = Type::ClassRef(ClassRef::gc_handle(asm));
        let mref = MethodRef::new(
            ClassRef::gc_handle(asm),
            asm.alloc_string("Alloc"),
            asm.sig([Type::PlatformObject], gc_handle_class),
            MethodKind::Static,
            vec![].into(),
        );
        let gc_handle = call!(asm.alloc_methodref(mref), [self]);
        let mref = MethodRef::new(
            ClassRef::gc_handle(asm),
            asm.alloc_string("op_Explicit"),
            asm.sig([gc_handle_class], Type::Int(Int::ISize)),
            MethodKind::Instance,
            vec![].into(),
        );
        call!(asm.alloc_methodref(mref), [gc_handle])
    }

    #[must_use]
    pub fn select(tpe: Type, a: Self, b: Self, predictate: Self, asm: &mut Assembly) -> Self {
        match tpe {
            Type::Int(
                int @ (Int::I8
                | Int::U8
                | Int::I16
                | Int::U16
                | Int::I32
                | Int::U32
                | Int::I64
                | Int::U64
                | Int::I128
                | Int::U128
                | Int::ISize
                | Int::USize),
            ) => {
                let select = MethodRef::new(
                    *asm.main_module(),
                    asm.alloc_string(format!("select_{}", int.name())),
                    asm.sig([Type::Int(int), Type::Int(int), Type::Bool], Type::Int(int)),
                    MethodKind::Static,
                    vec![].into(),
                );
                V1Node::Call(Box::new(crate::cil_node::CallOpArgs {
                    args: [a, b, predictate].into(),
                    site: (asm.alloc_methodref(select)),
                    is_pure: crate::cilnode::IsPure::PURE,
                }))
            }
            Type::Ptr(_) => {
                let int = Int::USize;
                let select = MethodRef::new(
                    *asm.main_module(),
                    asm.alloc_string(format!("select_{}", int.name())),
                    asm.sig([Type::Int(int), Type::Int(int), Type::Bool], Type::Int(int)),
                    MethodKind::Static,
                    vec![].into(),
                );
                V1Node::Call(Box::new(crate::cil_node::CallOpArgs {
                    args: [
                        a.cast_ptr(Type::Int(int)),
                        b.cast_ptr(Type::Int(int)),
                        predictate,
                    ]
                    .into(),
                    site: (asm.alloc_methodref(select)),
                    is_pure: crate::cilnode::IsPure::PURE,
                }))
                .cast_ptr(tpe)
            }
            _ => todo!("Can't select {tpe:?}"),
        }
    }

    /// Creates an uninitialized value of type *tpe*.
    pub fn uninit_val(tpe: Type, asm: &mut Assembly) -> Self {
        if tpe == Type::Void {
            let gv = asm.global_void();
            return V1Node::V2(asm.load_static(gv));
        }
        let main = asm.main_module();
        let sig = asm.sig([], tpe);
        let uninit_val = asm.new_methodref(*main, "uninit_val", sig, MethodKind::Static, []);
        V1Node::Call(Box::new(CallOpArgs {
            args: [].into(),
            site: uninit_val,
            is_pure: IsPure::PURE,
        }))
    }
    pub fn transmute_on_stack(self, src: Type, target: Type, asm: &mut Assembly) -> Self {
        if src == target {
            return self;
        }
        let main_module = *asm.main_module();

        let sig = asm.sig([src], target);
        let mref = asm.new_methodref(main_module, "transmute", sig, MethodKind::Static, vec![]);
        V1Node::Call(Box::new(CallOpArgs {
            args: Box::new([self]),
            site: mref,
            is_pure: crate::cilnode::IsPure::NOT,
        }))
    }
    pub fn cxchng_res_val(
        old_val: Self,
        expected: Self,
        destination_addr: Self,
        val_desc: Interned<FieldDesc>,
        flag_desc: Interned<FieldDesc>,
    ) -> [V1Root; 2] {
        // Set the value of the result.
        let set_val = V1Root::SetField {
            addr: Box::new(destination_addr.clone()),
            value: Box::new(old_val),
            desc: val_desc,
        };
        // Get the result back
        let val = V1Node::LDField {
            addr: Box::new(destination_addr.clone()),
            field: val_desc,
        };

        let cmp = V1Node::Eq(val.into(), expected.into());

        [
            set_val,
            V1Root::SetField {
                addr: Box::new(destination_addr.clone()),
                value: Box::new(cmp),
                desc: flag_desc,
            },
        ]
    }
    #[track_caller]
    pub fn cast_ptr(self, new_ptr: Type) -> Self {
        assert!(
            matches!(
                new_ptr,
                Type::Ptr(_)
                    | Type::Ref(_)
                    | Type::FnPtr(_)
                    | Type::Int(Int::USize)
                    | Type::Int(Int::ISize)
            ),
            "Invalid new ptr {new_ptr:?}"
        );

        Self::CastPtr {
            val: Box::new(self),
            new_ptr: Box::new(new_ptr),
        }
    }
}

#[macro_export]
macro_rules! and {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::And($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! shr {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Shr($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! shl {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Shl($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! shr_un {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::ShrUn($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! or {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Or($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! xor {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::XOr($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! div {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Div($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! rem {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Rem($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! rem_un {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::RemUn($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! eq {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Eq($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! lt {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Lt($a.into(), $b.into())
    };
}

#[macro_export]
macro_rules! lt_un {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::LtUn($a.into(), $b.into())
    };
}
#[macro_export]
macro_rules! gt {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::Gt($a.into(), $b.into())
    };
}

#[macro_export]
macro_rules! gt_un {
    ($a:expr,$b:expr) => {
        $crate::cil_node::V1Node::GtUn($a.into(), $b.into())
    };
}

#[macro_export]
macro_rules! ld_field {
    ($addr_calc:expr,$field:expr) => {
        $crate::cil_node::V1Node::LDField {
            addr: $addr_calc.into(),
            field: $field.into(),
        }
    };
}
#[macro_export]
macro_rules! ld_field_address {
    ($addr_calc:expr,$field:expr) => {
        V1Node::LDFieldAdress {
            addr: $addr_calc.into(),
            field: $field.into(),
        }
    };
}
#[macro_export]
macro_rules! call {
    ($call_site:expr,$args:expr) => {
        $crate::cil_node::V1Node::Call(Box::new($crate::cil_node::CallOpArgs {
            args: $args.into(),
            site: $call_site.into(),
            is_pure: $crate::cilnode::IsPure::NOT,
        }))
    };
}

#[macro_export]
macro_rules! call_virt {
    ($call_site:expr,$args:expr) => {
        V1Node::CallVirt(Box::new($crate::cil_node::CallOpArgs {
            args: $args.into(),
            site: $call_site.into(),
            is_pure: $crate::cilnode::IsPure::NOT,
        }))
    };
}
#[macro_export]
macro_rules! conv_usize {
    ($a:expr) => {
        $crate::cil_node::V1Node::ZeroExtendToUSize($a.into())
    };
}
#[macro_export]
macro_rules! conv_isize {
    ($a:expr) => {
        $crate::cil_node::V1Node::SignExtendToISize($a.into())
    };
}
#[macro_export]
macro_rules! conv_u64 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ZeroExtendToU64($a.into())
    };
}
#[macro_export]
macro_rules! conv_i64 {
    ($a:expr) => {
        $crate::cil_node::V1Node::SignExtendToI64($a.into())
    };
}
#[macro_export]
macro_rules! conv_u32 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvU32($a.into())
    };
}
#[macro_export]
macro_rules! conv_i32 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvI32($a.into())
    };
}
#[macro_export]
macro_rules! conv_u16 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvU16($a.into())
    };
}
#[macro_export]
macro_rules! conv_i16 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvI16($a.into())
    };
}
#[macro_export]
macro_rules! conv_i8 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvI8($a.into())
    };
}
#[macro_export]
macro_rules! conv_u8 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvU8($a.into())
    };
}

#[macro_export]
macro_rules! conv_f32 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvF32($a.into())
    };
}

#[macro_export]
macro_rules! conv_f64 {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvF64($a.into())
    };
}
#[macro_export]
macro_rules! conv_f_un {
    ($a:expr) => {
        $crate::cil_node::V1Node::ConvF64Un($a.into())
    };
}

impl std::ops::Add<Self> for V1Node {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Add(self.into(), rhs.into())
    }
}
impl std::ops::Sub<Self> for V1Node {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Sub(self.into(), rhs.into())
    }
}
impl std::ops::Mul<Self> for V1Node {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::Mul(self.into(), rhs.into())
    }
}
impl std::ops::BitOr<Self> for V1Node {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        or!(self, rhs)
    }
}
impl std::ops::BitAnd<Self> for V1Node {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        and!(self, rhs)
    }
}
impl std::ops::Neg for V1Node {
    fn neg(self) -> Self::Output {
        Self::Neg(self.into())
    }

    type Output = Self;
}
#[derive(Clone, Copy)]
pub struct ValidationContext<'a> {
    sig: &'a FnSig,
    locals: &'a [(Option<IString>, Type)],
}

impl<'a> ValidationContext<'a> {
    pub fn new(sig: &'a FnSig, locals: &'a [(Option<IString>, Type)]) -> Self {
        Self { sig, locals }
    }

    pub fn sig(&self) -> &FnSig {
        self.sig
    }

    pub fn locals(&self) -> &[(Option<IString>, Type)] {
        self.locals
    }
}
