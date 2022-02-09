#![feature(register_tool)]
#![feature(stmt_expr_attributes)]
#![register_tool(lr)]

//extern crate prusti_contracts;
//use prusti_contracts::*;

#[lr::ty(fn(i32{x: 0 < x}) -> i32{x: 0 < x})]
//#[requires(n > 0)]
//#[ensures(result > 0)]
fn fib_loop(n: i32) -> i32 {
    let mut k = n;
    let mut i = 1;
    let mut j = 1;
    // need i > 0, and therefore j >= 0 as well
    // i > 0 and j >= 0 on entry
    #[lr::loop_invariant(i > 0 && j >= 0)]
    while k > 2 {
        //body_invariant!(i > 0 && j >= 0);
        // Loop invariant: i > 0 && j >= 0
        let tmp = i + j;
        // tmp > 0
        j = i;
        // i > 0, so j > 0 (j >= 0)
        i = tmp;
        // tmp > 0, so i > 0
        k -= 1;
        // have i > 0 and j >= 0
    }
    i
}

// For reference, below is recursive variant with explicit invariant using liquid-type: i: i32{x: 0 < x}, j: i32{y: 0 <= y}
#[lr::ty(fn(i32{x: 0 < x}) -> i32{x: 0 < x})]
fn fib_recursive(n: i32) -> i32 {
    let mut k = n;
    fib_recursive_inv(&mut 1, &mut 1, &mut k)
}

#[lr::ty(fn(i: i32{x: 0 < x}, j: i32{y: 0 <= y}, k: i32; ref<i>, ref<j>, ref<k>) -> i32{ret: 0 < ret}; i: i32{x: 0 < x}, j: i32{y: 0 <= y}, k: i32)]
fn fib_recursive_inv(i: &mut i32, j: &mut i32, k: &mut i32) -> i32 {
    if *k > 2 {
        let tmp = *i + *j;
        *j = *i;
        *i = tmp;
        *k -= 1;
        fib_recursive_inv(i, j, k)
    } else {
        *i
    }
}