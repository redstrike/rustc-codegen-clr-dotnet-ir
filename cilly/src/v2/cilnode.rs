use serde::{Deserialize, Serialize};

use super::bimap::Interned;

use super::{Assembly, Const, Int};
use super::{ClassRef, FieldDesc, Float, FnSig, MethodRef, StaticFieldDesc};
use crate::cil_node::V1Node;
use crate::Type;

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct IsPure(pub bool);
impl IsPure {
    pub const NOT: Self = Self(false);
    pub const PURE: Self = Self(true);
}
#[derive(Hash, PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum CILNode {
    /// A constant IR value.
    Const(Box<Const>),
    /// Binary operation performed on values `lhs` and `rhs`, of kind `op`.
    BinOp(Interned<CILNode>, Interned<CILNode>, BinOp),
    /// A unary operation performed on value `val`, of kind `op`.
    UnOp(Interned<CILNode>, UnOp),
    /// Retrives the value of a local with a given index.
    LdLoc(u32),
    /// Retrives a reference(not a pointer!) to a local with a given index.
    /// See [crate::tpe::Type::Ref].
    LdLocA(u32),
    /// Retrives the value of an argument with a given index.
    LdArg(u32),
    /// Retrives a reference(not a pointer!) to an argument with a given index.
    /// See [crate::tpe::Type::Ref].
    LdArgA(u32),
    /// Calls `method` with, `args`, and a given `pure`-ness.
    /// [`IsPure::PURE`] value marks a call as a pure, side-effect free call.
    Call(Box<(Interned<MethodRef>, Box<[Interned<CILNode>]>, IsPure)>),
    /// A cast to an intiger type.
    IntCast {
        /// The input value.
        input: Interned<CILNode>,
        /// The resulting type
        target: Int,
        /// Is this a signed or zero extension?
        extend: ExtendKind,
    },
    FloatCast {
        input: Interned<CILNode>,
        target: Float,
        is_signed: bool,
    },
    RefToPtr(Interned<CILNode>),
    /// Changes the type of a pointer to `PtrCastRes`
    PtrCast(Interned<CILNode>, Box<PtrCastRes>),
    /// Loads the address of a field at `addr`
    LdFieldAddress {
        addr: Interned<CILNode>,
        field: Interned<FieldDesc>,
    },
    /// Loads the value of a field at `addr`
    LdField {
        addr: Interned<CILNode>,
        field: Interned<FieldDesc>,
    },
    /// Loads a value of `tpe` at `addr`
    LdInd {
        addr: Interned<CILNode>,
        tpe: Interned<Type>,
        volatile: bool,
    },
    /// Calcualtes the size of a type.
    SizeOf(Interned<Type>),
    /// Gets the currenrt exception, if it exisits. UB outside an exception handler.
    GetException,
    /// Checks if the object is an instace of a class.
    IsInst(Interned<CILNode>, Interned<Type>),
    /// Casts  the object to instace of a clsass.
    CheckedCast(Interned<CILNode>, Interned<Type>),
    /// Calls fn pointer with args
    CallI(Box<(Interned<CILNode>, Interned<FnSig>, Box<[Interned<CILNode>]>)>),
    /// Allocates memory from a local pool. It will get freed when this function return
    LocAlloc {
        size: Interned<CILNode>,
    },
    /// Loads a static field at descr
    LdStaticField(Interned<StaticFieldDesc>),
    /// Loads a static field at descr
    LdStaticFieldAddress(Interned<StaticFieldDesc>),
    /// Loads a pointer to a function
    LdFtn(Interned<MethodRef>),
    /// Loads a "type token"
    LdTypeToken(Interned<Type>),
    /// Gets the length of a platform array
    LdLen(Interned<CILNode>),
    /// Allocates a local buffer sizeof type, and aligned to algin.
    LocAllocAlgined {
        tpe: Interned<Type>,
        align: u64,
    },
    /// Loads a reference to array element at index.
    LdElelemRef {
        array: Interned<CILNode>,
        index: Interned<CILNode>,
    },
    /// Turns a managed reference to object into type
    UnboxAny {
        object: Interned<CILNode>,
        tpe: Interned<Type>,
    },
}
#[derive(Hash, PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum PtrCastRes {
    Ptr(Interned<Type>),
    Ref(Interned<Type>),
    FnPtr(Interned<FnSig>),
    USize,
    ISize,
}
impl PtrCastRes {
    pub fn as_type(&self) -> Type {
        match self {
            PtrCastRes::Ptr(type_idx) => Type::Ptr(*type_idx),
            PtrCastRes::Ref(type_idx) => Type::Ref(*type_idx),
            PtrCastRes::FnPtr(sig_idx) => Type::FnPtr(*sig_idx),
            PtrCastRes::USize => Type::Int(Int::USize),
            PtrCastRes::ISize => Type::Int(Int::ISize),
        }
    }
}
#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]

pub enum ExtendKind {
    ZeroExtend,
    SignExtend,
}
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MethodKind {
    Static,
    Instance,
    Virtual,
    Constructor,
}
#[derive(Hash, PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum UnOp {
    Not,
    Neg,
}
#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]

pub enum BinOp {
    Add,
    Eq,
    Sub,
    Mul,
    LtUn,
    Lt,
    GtUn,
    Gt,
    Or,
    XOr,
    And,
    Rem,
    RemUn,
    Shl,
    Shr,
    ShrUn,
    DivUn,
    Div,
}
impl BinOp {
    /// All binary operations, including signed and unsinged varaiants. For only one varaint, use  [`Self::ALL_DISTINCT_OPS`]
    pub const ALL_OPS: [Self; 18] = [
        BinOp::Add,
        BinOp::Eq,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::LtUn,
        BinOp::Lt,
        BinOp::GtUn,
        BinOp::Gt,
        BinOp::Or,
        BinOp::XOr,
        BinOp::And,
        BinOp::Rem,
        BinOp::RemUn,
        BinOp::Shl,
        BinOp::Shr,
        BinOp::ShrUn,
        BinOp::DivUn,
        BinOp::Div,
    ];
    /// [`Self::ALL_OPS`], not including the unsinged variants.
    pub const ALL_DISTINCT_OPS: [Self; 13] = [
        BinOp::Add,
        BinOp::Eq,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Lt,
        BinOp::Gt,
        BinOp::Or,
        BinOp::XOr,
        BinOp::And,
        BinOp::Rem,
        BinOp::Shl,
        BinOp::Shr,
        BinOp::Div,
    ];
    /// Returns a short name descirbing this operation.
    /// WARNING: this function will return the same name for signed and unsinged variants!
    /// ```
    /// # use cilly::BinOp;
    /// assert_eq!(BinOp::Eq.name(),"eq");
    /// assert_eq!(BinOp::Add.name(),"add");
    /// // Signed and unsinged variant have the same name!
    /// assert_eq!(BinOp::Lt.name(),BinOp::LtUn.name());
    /// assert_eq!(BinOp::Lt.name(),"lt");
    /// assert_eq!(BinOp::LtUn.name(),"lt");
    /// ```
    pub fn name(&self) -> &'static str {
        match self {
            BinOp::Add => "add",
            BinOp::Eq => "eq",
            BinOp::Sub => "sub",
            BinOp::Mul => "mul",
            BinOp::LtUn | BinOp::Lt => "lt",
            BinOp::GtUn | BinOp::Gt => "gt",
            BinOp::Or => "or",
            BinOp::XOr => "xor",
            BinOp::And => "and",
            BinOp::Rem | BinOp::RemUn => "mod",
            BinOp::Shl => "shl",
            BinOp::Shr | BinOp::ShrUn => "shr",
            BinOp::DivUn | BinOp::Div => "div",
        }
    }
    /// Returns the name of the .NET operation this binop corresponds to.
    /// ```
    /// # use cilly::BinOp;
    /// assert_eq!(BinOp::Eq.dotnet_name(),"op_Equality");
    /// assert_eq!(BinOp::Add.dotnet_name(),"op_Addition");
    /// // Signed and unsinged variant have the same name!
    /// assert_eq!(BinOp::Lt.dotnet_name(),BinOp::LtUn.dotnet_name());
    /// assert_eq!(BinOp::Lt.dotnet_name(),"op_LessThan");
    /// assert_eq!(BinOp::LtUn.dotnet_name(),"op_LessThan");
    /// ```
    pub fn dotnet_name(&self) -> &str {
        match self {
            BinOp::Add => "op_Addition",
            BinOp::Eq => "op_Equality",
            BinOp::Sub => "op_Subtraction",
            BinOp::Mul => "op_Multiply",
            BinOp::LtUn | BinOp::Lt => "op_LessThan",
            BinOp::GtUn | BinOp::Gt => "op_GreaterThan",
            BinOp::Or => "op_BitwiseOr",
            BinOp::XOr => "op_ExclusiveOr",
            BinOp::And => "op_BitwiseAnd",
            BinOp::Rem | BinOp::RemUn => "op_Modulus",
            BinOp::Shl => "op_LeftShift",
            BinOp::Shr | BinOp::ShrUn => "op_RightShift",
            BinOp::DivUn | BinOp::Div => "op_Division",
        }
    }
}
impl CILNode {
    pub fn call(mref: Interned<MethodRef>, args: impl Into<Box<[Interned<CILNode>]>>) -> Self {
        Self::Call(Box::new((mref, args.into(), IsPure::NOT)))
    }
    /// Returns all the nodes this node references.
    /// ```
    /// # use cilly::*;
    /// # let mut asm = Assembly::default();
    /// let ldarg_0 = asm.alloc_node(CILNode::LdArg(0));
    /// let ldloc_1 = asm.alloc_node(CILNode::LdLoc(1));
    /// let binop = CILNode::BinOp(ldarg_0,ldloc_1,BinOp::Add);
    /// // Two child nodes - ldarg_0 and ldloc_1
    /// assert_eq!(binop.child_nodes(),vec![ldarg_0,ldloc_1]);
    /// ```
    pub fn child_nodes(&self) -> Vec<Interned<CILNode>> {
        match self {
            CILNode::Const(_)
            | CILNode::LdLoc(_)
            | CILNode::LdLocA(_)
            | CILNode::LdArg(_)
            | CILNode::LdArgA(_)
            | CILNode::SizeOf(_)
            | CILNode::LocAllocAlgined { .. }
            | CILNode::LdFtn(_)
            | CILNode::LdTypeToken(_)
            | CILNode::LdStaticField(_)
            | CILNode::LdStaticFieldAddress(_)
            | CILNode::GetException => vec![],
            CILNode::UnOp(node_idx, _)
            | CILNode::RefToPtr(node_idx)
            | CILNode::PtrCast(node_idx, _)
            | CILNode::LdLen(node_idx)
            | CILNode::LdFieldAddress { addr: node_idx, .. }
            | CILNode::LdField { addr: node_idx, .. }
            | CILNode::LdInd { addr: node_idx, .. }
            | CILNode::LocAlloc { size: node_idx }
            | CILNode::IsInst(node_idx, _)
            | CILNode::CheckedCast(node_idx, _)
            | CILNode::IntCast {
                input: node_idx, ..
            }
            | CILNode::FloatCast {
                input: node_idx, ..
            }
            | CILNode::LdElelemRef {
                array: node_idx, ..
            }
            | CILNode::UnboxAny {
                object: node_idx, ..
            } => vec![*node_idx],
            CILNode::BinOp(lhs, rhs, _) => vec![*lhs, *rhs],
            CILNode::Call(info) => {
                let (_, args, _is_pure) = info.as_ref();
                args.to_vec()
            }
            CILNode::CallI(info) => {
                let (fnptr, _, args) = info.as_ref();
                let mut res = vec![*fnptr];
                res.extend(args);
                res
            }
        }
    }
    /// Turns a native object handle into a special handle of type [`Int::ISize`]
    #[must_use]
    pub fn ref_to_handle(&self, asm: &mut Assembly) -> Self {
        let gc_handle = ClassRef::gc_handle(asm);
        let alloc = asm.alloc_string("Alloc");
        let alloc = asm.class_ref(gc_handle).clone().static_mref(
            &[Type::PlatformObject],
            Type::ClassRef(gc_handle),
            alloc,
            asm,
        );
        let op_explict = asm.alloc_string("op_Explicit");
        let op_explict = asm.class_ref(gc_handle).clone().static_mref(
            &[Type::ClassRef(gc_handle)],
            Type::Int(Int::ISize),
            op_explict,
            asm,
        );
        let arg = asm.alloc_node(self.clone());
        let alloc = asm.alloc_node(CILNode::call(alloc, [arg]));
        CILNode::call(op_explict, [alloc])
    }

    // This function has to be complex
    #[allow(clippy::too_many_lines)]
    pub fn from_v1(v1: &V1Node, asm: &mut Assembly) -> Self {
        match v1 {
            // Varaible access
            V1Node::LDLoc(arg) => CILNode::LdLoc(*arg),

            // Ptr deref
            V1Node::LDIndBool { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Bool),
                    volatile: false,
                }
            }
            V1Node::LDIndU8 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::U8)),
                    volatile: false,
                }
            }
            V1Node::LDIndU16 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::U16)),
                    volatile: false,
                }
            }
            V1Node::LDIndU32 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::U32)),
                    volatile: false,
                }
            }
            V1Node::LDIndU64 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::U64)),
                    volatile: false,
                }
            }
            V1Node::LDIndUSize { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::USize)),
                    volatile: false,
                }
            }
            V1Node::LDIndI8 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::I8)),
                    volatile: false,
                }
            }
            V1Node::LDIndI16 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::I16)),
                    volatile: false,
                }
            }
            V1Node::LDIndI32 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::I32)),
                    volatile: false,
                }
            }
            V1Node::LDIndI64 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::I64)),
                    volatile: false,
                }
            }
            V1Node::LDIndISize { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Int(Int::ISize)),
                    volatile: false,
                }
            }
            V1Node::LDIndF32 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Float(Float::F32)),
                    volatile: false,
                }
            }
            V1Node::LDIndF64 { ptr } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(Type::Float(Float::F64)),
                    volatile: false,
                }
            }
            V1Node::LdObj { ptr, obj } => {
                let ptr = Self::from_v1(ptr, asm);
                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(*obj.as_ref()),
                    volatile: false,
                }
            }
            V1Node::LDIndPtr { ptr, loaded_ptr } => {
                let ptr = Self::from_v1(ptr, asm);

                Self::LdInd {
                    addr: asm.alloc_node(ptr),
                    tpe: asm.alloc_type(*loaded_ptr.as_ref()),
                    volatile: false,
                }
            }
            // Casts
            V1Node::ZeroExtendToU64(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::U64,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::SignExtendToU64(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::U64,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ZeroExtendToUSize(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::USize,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::SignExtendToUSize(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::USize,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ConvU8(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::U8,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::ConvU16(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::U16,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::ConvU32(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::U32,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::SignExtendToI64(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::I64,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ZeroExtendToISize(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::ISize,
                    extend: ExtendKind::ZeroExtend,
                }
            }
            V1Node::SignExtendToISize(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::ISize,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ConvI8(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::I8,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ConvI16(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::I16,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ConvI32(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::IntCast {
                    input: asm.alloc_node(node),
                    target: Int::I32,
                    extend: ExtendKind::SignExtend,
                }
            }
            V1Node::ConvF32(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::FloatCast {
                    input: asm.alloc_node(node),
                    target: Float::F32,
                    is_signed: true,
                }
            }
            V1Node::ConvF64(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::FloatCast {
                    input: asm.alloc_node(node),
                    target: Float::F64,
                    is_signed: true,
                }
            }
            V1Node::ConvF64Un(inner) => {
                let node = Self::from_v1(inner, asm);
                CILNode::FloatCast {
                    input: asm.alloc_node(node),
                    target: Float::F64,
                    is_signed: false,
                }
            }
            V1Node::MRefToRawPtr(inner) => {
                let raw = Self::from_v1(inner, asm);
                CILNode::RefToPtr(asm.alloc_node(raw))
            }
            V1Node::CastPtr { val, new_ptr } => {
                let val = Self::from_v1(val, asm);

                let ptr = match &**new_ptr {
                    Type::Int(Int::USize) => PtrCastRes::USize,
                    Type::Int(Int::ISize) => PtrCastRes::ISize,
                    Type::Ptr(inner) => PtrCastRes::Ptr(*inner),
                    Type::Ref(inner) => PtrCastRes::Ref(*inner),
                    Type::FnPtr(sig) => PtrCastRes::FnPtr(*sig),
                    _ => panic!("Type {new_ptr:?} is not a pointer."),
                };
                CILNode::PtrCast(asm.alloc_node(val), Box::new(ptr))
            }
            // Binops
            V1Node::Add(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Add)
            }
            V1Node::Sub(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Sub)
            }
            V1Node::Mul(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Mul)
            }
            V1Node::Eq(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Eq)
            }
            V1Node::Or(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Or)
            }
            V1Node::XOr(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::XOr)
            }
            V1Node::And(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::And)
            }
            V1Node::LtUn(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::LtUn)
            }
            V1Node::Lt(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Lt)
            }
            V1Node::GtUn(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::GtUn)
            }
            V1Node::Gt(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Gt)
            }
            V1Node::Rem(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Rem)
            }
            V1Node::RemUn(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::RemUn)
            }
            V1Node::Shl(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Shl)
            }
            V1Node::Shr(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Shr)
            }
            V1Node::ShrUn(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::ShrUn)
            }
            V1Node::Div(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::Div)
            }
            V1Node::DivUn(lhs, rhs) => {
                let lhs = Self::from_v1(lhs, asm);
                let rhs = Self::from_v1(rhs, asm);
                Self::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), BinOp::DivUn)
            }
            // Unops
            V1Node::Not(val) => {
                let val = Self::from_v1(val, asm);
                Self::UnOp(asm.alloc_node(val), UnOp::Not)
            }
            V1Node::Neg(val) => {
                let val = Self::from_v1(val, asm);
                Self::UnOp(asm.alloc_node(val), UnOp::Neg)
            }
            // Field access
            V1Node::LDField { addr, field } => {
                let addr = Self::from_v1(addr, asm);
                Self::LdField {
                    addr: asm.alloc_node(addr),
                    field: *field,
                }
            }
            V1Node::LDFieldAdress { addr, field } => {
                let addr = Self::from_v1(addr, asm);
                Self::LdFieldAddress {
                    addr: asm.alloc_node(addr),
                    field: *field,
                }
            }
            // Calls
            V1Node::Call(callargs) => {
                let args: Box<[_]> = callargs
                    .args
                    .iter()
                    .map(|arg| {
                        let node = Self::from_v1(arg, asm);
                        asm.alloc_node(node)
                    })
                    .collect();
                Self::Call(Box::new((callargs.site, args, callargs.is_pure)))
            }
            V1Node::CallVirt(callargs) => {
                let args: Box<[_]> = callargs
                    .args
                    .iter()
                    .map(|arg| {
                        let node = Self::from_v1(arg, asm);
                        asm.alloc_node(node)
                    })
                    .collect();

                Self::Call(Box::new((callargs.site, args, IsPure::NOT)))
            }
            V1Node::NewObj(callargs) => {
                let args: Box<[_]> = callargs
                    .args
                    .iter()
                    .map(|arg| {
                        let node = Self::from_v1(arg, asm);
                        asm.alloc_node(node)
                    })
                    .collect();
                Self::Call(Box::new((callargs.site, args, IsPure::NOT)))
            }
            // Special
            V1Node::GetException => Self::GetException,
            // Consts
            V1Node::LdStr(string) => {
                let string = asm.alloc_string(string.clone());
                Const::PlatformString(string).into()
            }

            V1Node::LDTypeToken(tpe) => Self::LdTypeToken(asm.alloc_type(*tpe.as_ref())),

            // Special
            V1Node::IsInst(combined) => {
                let (val, tpe) = combined.as_ref();

                let tpe = asm.alloc_type(Type::ClassRef(*tpe));
                let val = Self::from_v1(val, asm);

                Self::IsInst(asm.alloc_node(val), tpe)
            }
            V1Node::CheckedCast(combined) => {
                let (val, tpe) = combined.as_ref();

                let val = Self::from_v1(val, asm);
                let tpe = asm.alloc_type(Type::ClassRef(*tpe));
                Self::CheckedCast(asm.alloc_node(val), tpe)
            }
            V1Node::CallI(sig_ptr_args) => {
                let sig = asm.alloc_sig(sig_ptr_args.0.clone());
                let ptr = Self::from_v1(&sig_ptr_args.1, asm);
                let ptr = asm.alloc_node(ptr);
                let args: Box<[_]> = sig_ptr_args
                    .2
                    .iter()
                    .map(|arg| {
                        let arg = Self::from_v1(arg, asm);
                        asm.alloc_node(arg)
                    })
                    .collect();
                Self::CallI(Box::new((ptr, sig, args)))
            }
            V1Node::LocAlloc { size } => {
                let size = Self::from_v1(size, asm);
                let size = asm.alloc_node(size);
                CILNode::LocAlloc { size }
            }
            V1Node::LocAllocAligned { tpe, align } => {
                let tpe = asm.alloc_type(*tpe.as_ref());
                CILNode::LocAllocAlgined { tpe, align: *align }
            }
            V1Node::LDFtn(method_ref) => Self::LdFtn(*method_ref),
            V1Node::Volatile(inner) => {
                let mut tmp = Self::from_v1(inner, asm);
                if let Self::LdInd {
                    volatile: volitale, ..
                } = &mut tmp
                {
                    *volitale = true;
                } else {
                    panic!()
                }
                tmp
            }
            V1Node::LDLen { arr } => {
                let arr = Self::from_v1(arr, asm);
                let arr = asm.alloc_node(arr);
                Self::LdLen(arr)
            }
            V1Node::LDElelemRef { arr, idx } => {
                let arr = Self::from_v1(arr, asm);
                let array = asm.alloc_node(arr);
                let idx = Self::from_v1(idx, asm);
                let index = asm.alloc_node(idx);
                Self::LdElelemRef { array, index }
            }
            V1Node::UnboxAny(object, tpe) => {
                let object = Self::from_v1(object, asm);
                let object = asm.alloc_node(object);
                let tpe = asm.alloc_type(*tpe.as_ref());
                Self::UnboxAny { object, tpe }
            }
            V1Node::V2(v2) => asm[*v2].clone(),
            _ => todo!("v1:{v1:?}"),
        }
    }
}
impl CILNode {
    /// Changes the node by applying the `map` closure to each node. This process is
    // The complexity of this function is unavoidable.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn map(self, asm: &mut Assembly, map: &mut dyn FnMut(Self, &mut Assembly) -> Self) -> Self {
        match self {
            CILNode::Const(_)
            | CILNode::LdLoc(_)
            | CILNode::LdLocA(_)
            | CILNode::LdArg(_)
            | CILNode::LdArgA(_)
            | CILNode::SizeOf(_)
            | CILNode::GetException
            | CILNode::LocAllocAlgined { .. }
            | CILNode::LdStaticField(_)
            | CILNode::LdStaticFieldAddress(_)
            | CILNode::LdFtn(_)
            | CILNode::LdTypeToken(_) => map(self, asm),
            CILNode::BinOp(lhs, rhs, op) => {
                let lhs = asm.get_node(lhs).clone().map(asm, map);
                let rhs = asm.get_node(rhs).clone().map(asm, map);
                let node = CILNode::BinOp(asm.alloc_node(lhs), asm.alloc_node(rhs), op);
                map(node, asm)
            }
            CILNode::UnOp(lhs, op) => {
                let lhs = asm.get_node(lhs).clone().map(asm, map);
                let node = CILNode::UnOp(asm.alloc_node(lhs), op);
                map(node, asm)
            }
            CILNode::Call(call_info) => {
                let (method_id, args, _is_pure) = *call_info;
                let args = args
                    .iter()
                    .map(|arg| {
                        let node = asm.get_node(*arg).clone().map(asm, map);
                        asm.alloc_node(node)
                    })
                    .collect::<Box<_>>();

                let node = CILNode::call(method_id, args);
                map(node, asm)
            }
            CILNode::IntCast {
                input,
                target,
                extend,
            } => {
                let input = asm.get_node(input).clone().map(asm, map);
                let node = CILNode::IntCast {
                    input: asm.alloc_node(input),
                    target,
                    extend,
                };
                map(node, asm)
            }
            CILNode::FloatCast {
                input,
                target,
                is_signed,
            } => {
                let input = asm.get_node(input).clone().map(asm, map);
                let node = CILNode::FloatCast {
                    input: asm.alloc_node(input),
                    target,
                    is_signed,
                };
                map(node, asm)
            }
            CILNode::RefToPtr(input) => {
                let input = asm.get_node(input).clone().map(asm, map);
                let node = CILNode::RefToPtr(asm.alloc_node(input));
                map(node, asm)
            }
            CILNode::PtrCast(input, tpe) => {
                let input = asm.get_node(input).clone().map(asm, map);
                let node = CILNode::PtrCast(asm.alloc_node(input), tpe);
                map(node, asm)
            }
            CILNode::LdFieldAddress { addr, field } => {
                let addr = asm.get_node(addr).clone().map(asm, map);
                let node = CILNode::LdFieldAddress {
                    addr: asm.alloc_node(addr),
                    field,
                };
                map(node, asm)
            }
            CILNode::LdField { addr, field } => {
                let addr = asm.get_node(addr).clone().map(asm, map);
                let node = CILNode::LdField {
                    addr: asm.alloc_node(addr),
                    field,
                };
                map(node, asm)
            }
            CILNode::LdInd {
                addr,
                tpe,
                volatile: volitale,
            } => {
                let addr = asm.get_node(addr).clone().map(asm, map);
                let node = CILNode::LdInd {
                    addr: asm.alloc_node(addr),
                    tpe,
                    volatile: volitale,
                };
                map(node, asm)
            }
            CILNode::IsInst(object, tpe) => {
                let object = asm.get_node(object).clone().map(asm, map);
                let node = CILNode::IsInst(asm.alloc_node(object), tpe);
                map(node, asm)
            }
            CILNode::CheckedCast(object, tpe) => {
                let object = asm.get_node(object).clone().map(asm, map);
                let node = CILNode::CheckedCast(asm.alloc_node(object), tpe);
                map(node, asm)
            }
            CILNode::CallI(call_info) => {
                let (ptr, sig, args) = *call_info;
                let args = args
                    .iter()
                    .map(|arg| {
                        let node = asm.get_node(*arg).clone().map(asm, map);
                        asm.alloc_node(node)
                    })
                    .collect();
                let ptr = asm.get_node(ptr).clone().map(asm, map);
                let node = CILNode::CallI(Box::new((asm.alloc_node(ptr), sig, args)));
                map(node, asm)
            }
            CILNode::LocAlloc { size } => {
                let size = asm.get_node(size).clone().map(asm, map);
                let node = CILNode::LocAlloc {
                    size: asm.alloc_node(size),
                };
                map(node, asm)
            }

            CILNode::LdLen(input) => {
                let input = asm.get_node(input).clone().map(asm, map);
                let node = CILNode::LdLen(asm.alloc_node(input));
                map(node, asm)
            }

            CILNode::LdElelemRef { array, index } => {
                let array = asm.get_node(array).clone().map(asm, map);
                let index = asm.get_node(index).clone().map(asm, map);
                let node = CILNode::LdElelemRef {
                    array: asm.alloc_node(array),
                    index: asm.alloc_node(index),
                };
                map(node, asm)
            }
            CILNode::UnboxAny { object, tpe } => {
                let object = asm.get_node(object).clone().map(asm, map);
                let node = CILNode::UnboxAny {
                    object: asm.alloc_node(object),
                    tpe,
                };
                map(node, asm)
            }
        }
    }
}
