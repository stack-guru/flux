#![feature(register_tool)]
#![feature(stmt_expr_attributes)]
#![register_tool(lr)]

//extern crate prusti_contracts;
//use prusti_contracts::*;

//#[lr::ty(fn(i32{x: 0 < x}) -> i32{x: 0 < x})]
//#[requires(n > 0)]
//#[ensures(result == ((n * (n + 1)) / 2))]

#[lr::ty(fn(i32{x: 0 < x}) -> i32{x: 0 < x})]
fn _sum_up_to(n: i32) -> i32 {
    let mut sum = 0;
    let mut i = 0;

    // #[lr::loop_invariant(sum == ((i * (i + 1)) / 2))]
    #[lr::loop_invariant(sum > 0)]
    #[lr::loop_invariant(i >= 0)]
    while i < n {
        // body_invariant!(sum == ((i * (i + 1)) / 2))      
        i += 1;
        sum += i;
    }

    sum
}