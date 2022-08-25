#![feature(register_tool)]
#![register_tool(flux)]

#[flux::sig(
fn(x: &strg i32[@n]) -> i32
ensures x: i32[n+1]
)]
pub fn inc(x: &mut i32) -> i32 {
    *x += 1;
    0
}

#[flux::sig(fn() -> i32[2])]
pub fn test_inc() -> i32 {
    let mut x = 1;
    inc(&mut x);
    x
}
