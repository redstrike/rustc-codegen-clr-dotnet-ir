#![feature(rustc_private)]
extern crate rustc_abi;
extern crate rustc_driver;
extern crate rustc_middle;
extern crate rustc_target;
use cilly::FnSig;
use rustc_abi::{CanonAbi, ExternAbi as TargetAbi};
use rustc_codegen_clr_ctx::MethodCompileCtx;
use rustc_codegen_clr_type::r#type::get_type;
use rustc_middle::ty::{Instance, List, PseudoCanonicalInput, TyKind};
pub struct CallInfo {
    sig: FnSig,
    split_last_tuple: bool,
}
impl CallInfo {
    /// Returns the signature of function behind `function`.
    pub fn sig_from_instance_<'tcx>(
        function: Instance<'tcx>,
        ctx: &mut MethodCompileCtx<'tcx, '_>,
    ) -> Self {
        let fn_abi = ctx.tcx().fn_abi_of_instance(PseudoCanonicalInput {
            typing_env: rustc_middle::ty::TypingEnv::fully_monomorphized(),
            value: (function, List::empty()),
        });
        let fn_abi = match fn_abi {
            Ok(abi) => abi,
            Err(_error) => todo!(),
        };
        let conv = fn_abi.conv;
        #[allow(clippy::match_same_arms)]
        match conv {
            CanonAbi::C | CanonAbi::Rust => (),
            // TODO: check this is 100% correct!
            CanonAbi::X86(_) => (),
            _ => panic!("ERROR:calling using convention {conv:?} is not supported!"),
        }
        //assert!(!fn_abi.c_variadic);
        let ret = get_type(fn_abi.ret.layout.ty, ctx);
        let mut args = Vec::with_capacity(fn_abi.args.len());

        for arg in &fn_abi.args {
            args.push(get_type(arg.layout.ty, ctx));
        }
        // There are 2 ABI enums for some reasons(they differ in what memebers they have)
        let fn_ty = function.ty(
            ctx.tcx(),
            rustc_middle::ty::TypingEnv::fully_monomorphized(),
        );
        let internal_abi = match fn_ty.kind() {
            TyKind::FnDef(_, _) => fn_ty.fn_sig(ctx.tcx()).abi(),
            TyKind::Closure(_, args) => args.as_closure().sig().abi(),
            TyKind::Coroutine(_, _) => TargetAbi::Rust, // TODO: this assumes all coroutines have the ABI Rust. This *should* be correct.
            _ => todo!("Can't get signature of {fn_ty}"),
        };
        // Only those ABIs are supported
        let split_last_tuple = match internal_abi {
            TargetAbi::C { unwind: _ }
            | TargetAbi::Cdecl { unwind: _ }
            | TargetAbi::Rust
            | TargetAbi::RustCold
            | TargetAbi::Unadjusted
            | TargetAbi::SysV64 { unwind: _ } => false,

            TargetAbi::RustCall => true, /*Err(CodegenError::FunctionABIUnsuported(
            "\"rust_call\" ABI, used for things like clsoures, is not supported yet!",
            ))?,*/
            _ => todo!("Unsuported ABI:{internal_abi:?}"),
        };
        let mut sig = FnSig::new(args, ret);
        if fn_abi.c_variadic {
            let remaining = fn_abi.args[(fn_abi.fixed_count as usize)..]
                .iter()
                .map(|ty| get_type(ctx.monomorphize(ty.layout.ty), ctx));
            let mut inputs = sig.inputs().to_vec();
            inputs.extend(remaining);
            sig.set_inputs(inputs);
        }
        Self {
            sig,
            split_last_tuple,
        }
    }

    pub fn sig(&self) -> &FnSig {
        &self.sig
    }

    pub fn split_last_tuple(&self) -> bool {
        self.split_last_tuple
    }
}
