#![feature(
    lang_items,
    adt_const_params,
    associated_type_defaults,
    core_intrinsics,
    unsized_const_params
)]
#![allow(internal_features, incomplete_features, unused_variables, dead_code)]
include!("../common.rs");
extern crate core;

use core::intrinsics::ceilf32;
use core::intrinsics::ceilf64;
use core::intrinsics::fabsf32;
use core::intrinsics::fabsf64;
use core::intrinsics::floorf32;
use core::intrinsics::floorf64;
use core::intrinsics::round_ties_even_f32;
use core::intrinsics::round_ties_even_f64;
use core::intrinsics::roundf32;
use core::intrinsics::roundf64;
use core::intrinsics::truncf32;
use core::intrinsics::truncf64;

fn main() {
    let x = 3.5_f32;
    let y = -3.5_f32;
    test_eq!(unsafe { fabsf32(x) }, black_box(x));
    test_eq!(unsafe { fabsf32(y) }, black_box(-y));
    test!(unsafe { fabsf32(f32::NAN) }.is_nan());
    let x = 3.5_f64;
    let y = -3.5_f64;
    test_eq!(unsafe { fabsf64(x) }, black_box(x));
    test_eq!(unsafe { fabsf64(y) }, black_box(-y));
    test!(unsafe { fabsf64(f64::NAN) }.is_nan());
    let f = 3.3_f32;
    let g = -3.3_f32;
    let h = 3.5_f32;
    let i = 4.5_f32;
    test_eq!({ round_ties_even_f32(f) }, black_box(3.0));
    test_eq!({ round_ties_even_f32(g) }, black_box(-3.0));
    test_eq!({ round_ties_even_f32(h) }, black_box(4.0));
    test_eq!({ round_ties_even_f32(i) }, black_box(4.0));
    let f = 3.3_f64;
    let g = -3.3_f64;
    let h = 3.5_f64;
    let i = 4.5_f64;
    test_eq!({ round_ties_even_f64(f) }, black_box(3.0));
    test_eq!({ round_ties_even_f64(g) }, black_box(-3.0));
    test_eq!({ round_ties_even_f64(h) }, black_box(4.0));
    test_eq!({ round_ties_even_f64(i) }, black_box(4.0));
    let f = 3.3_f32;
    let g = -3.3_f32;
    let h = -3.7_f32;
    let i = 3.5_f32;
    let j = 4.5_f32;
    test_eq!(unsafe { roundf32(f) }, black_box(3.0));
    test_eq!(unsafe { roundf32(g) }, black_box(-3.0));
    test_eq!(unsafe { roundf32(h) }, black_box(-4.0));
    test_eq!(unsafe { roundf32(i) }, black_box(4.0));
    test_eq!(unsafe { roundf32(j) }, black_box(5.0));
    let f = 3.3_f64;
    let g = -3.3_f64;
    let h = -3.7_f64;
    let i = 3.5_f64;
    let j = 4.5_f64;
    test_eq!(unsafe { roundf64(f) }, black_box(3.0));
    test_eq!(unsafe { roundf64(g) }, black_box(-3.0));
    test_eq!(unsafe { roundf64(h) }, black_box(-4.0));
    test_eq!(unsafe { roundf64(i) }, black_box(4.0));
    test_eq!(unsafe { roundf64(j) }, black_box(5.0));
    let f = 3.01_f32;
    let g = 4.0_f32;
    test_eq!(unsafe { ceilf32(f) }, black_box(4.0));
    test_eq!(unsafe { ceilf32(g) }, black_box(4.0));
    let f = 3.01_f64;
    let g = 4.0_f64;
    test_eq!(unsafe { ceilf64(f) }, black_box(4.0));
    test_eq!(unsafe { ceilf64(g) }, black_box(4.0));
    let f = 3.7_f32;
    let g = 3.0_f32;
    let h = -3.7_f32;
    test_eq!(unsafe { floorf32(f) }, black_box(3.0));
    test_eq!(unsafe { floorf32(g) }, black_box(3.0));
    test_eq!(unsafe { floorf32(h) }, black_box(-4.0));
    let f = 3.7_f64;
    let g = 3.0_f64;
    let h = -3.7_f64;
    test_eq!(unsafe { floorf64(f) }, black_box(3.0));
    test_eq!(unsafe { floorf64(g) }, black_box(3.0));
    test_eq!(unsafe { floorf64(h) }, black_box(-4.0));
    let f = 3.7_f32;
    let g = 3.0_f32;
    let h = -3.7_f32;
    assert_eq!(unsafe { truncf32(f) }, black_box(3.0));
    assert_eq!(unsafe { truncf32(g) }, black_box(3.0));
    assert_eq!(unsafe { truncf32(h) }, black_box(-3.0));
    let f = 3.7_f64;
    let g = 3.0_f64;
    let h = -3.7_f64;
    assert_eq!(unsafe { truncf64(f) }, black_box(3.0));
    assert_eq!(unsafe { truncf64(g) }, black_box(3.0));
    assert_eq!(unsafe { truncf64(h) }, black_box(-3.0));
}
