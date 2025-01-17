#![feature(register_tool)]
#![register_tool(flux)]

#[flux::sig(fn(x: i32{v : v > 0 && x < 10}) -> i32{v : v > x && v < 11})]
fn exists(x: i32) -> i32 {
    x + 1
}

#[flux::sig(fn(x: &i32[@n]) -> i32[n + 1])]
fn shr_ref(x: &i32) -> i32 {
    *x + 1
}

#[flux::sig(fn(x: i32) -> i32[x + 1])]
fn path(x: i32) -> i32 {
    x + 1
}

#[flux::sig(fn(x: [i32{v : v > 0}; _]) -> [i32{v : v >= 0}; _])]
fn arr(x: [i32; 1]) -> [i32; 1] {
    x
}

#[flux::sig(fn(x: &[i32{v : v > 0}]) -> &[i32{v : v >= 0}])]
fn slice(x: &[i32]) -> &[i32] {
    x
}
