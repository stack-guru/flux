#![feature(register_tool)]
#![register_tool(flux)]

type MyResult<T> = Result<T, ()>;

#[flux::sig(fn() -> MyResult<i32>)]
pub fn test() -> MyResult<i32> {
    Ok(10)
}
