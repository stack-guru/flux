#![feature(register_tool)]
#![register_tool(flux)]

#[path = "../../lib/my_option.rs"]
mod my_option;
use my_option::MyOption;

pub fn unwrap_or_else<T>(opt: MyOption<T>, val: T) -> T {
    opt.unwrap() //~ ERROR precondition might not hold
}

#[flux::sig(fn() -> i32[1])]
pub fn test() -> i32 {
    let opt = MyOption::none();
    opt.unwrap() //~ ERROR precondition might not hold
}
