#![feature(register_tool)]
#![register_tool(flux)]
#![feature(custom_inner_attributes)]
#![flux::expr(nat(x: int) -> bool { 0 <= x })]

#[flux::alias(type Nat() = i32{v:nat(v)})]
type _Nat = i32;

#[flux::sig(fn(x:Nat) -> Nat)]
pub fn test2(x: i32) -> i32 {
    x + 1
}