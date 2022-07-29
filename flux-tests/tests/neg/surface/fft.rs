#![allow(unused_attributes)]
#![feature(register_tool)]
#![register_tool(flux)]

#[path = "../../lib/rvec.rs"]
pub mod rvec;
use rvec::RVec;

#[flux::sig(fn() -> f32)]
fn pi() -> f32 {
    3.14159265358979323846
}

#[flux::sig(fn() -> f32)]
fn two_pi() -> f32 {
    2.0 * pi()
}

#[flux::assume]
#[flux::sig(fn(n:usize) -> f32)]
fn float_of_int(n: usize) -> f32 {
    n as f32
}

#[flux::assume]
#[flux::sig(fn(f32) -> f32)]
pub fn fabs(x: f32) -> f32 {
    f32::abs(x)
}

#[flux::assume]
#[flux::sig(fn(f32) -> f32)]
fn cos(x: f32) -> f32 {
    f32::cos(x)
}

#[flux::assume]
#[flux::sig(fn(f32) -> f32)]
fn sin(x: f32) -> f32 {
    f32::sin(x)
}

#[flux::sig(
fn(&mut RVec<f32>[@n], &mut RVec<f32>[n]) -> i32
requires 2 <= n
)]
pub fn fft(px: &mut RVec<f32>, py: &mut RVec<f32>) -> i32 {
    loop_a(px, py);
    loop_b(px, py);
    loop_c(px, py);
    0
}

#[flux::sig(fn( &mut RVec<f32>[@n], &mut RVec<f32>[n]) -> i32)]
fn loop_a(px: &mut RVec<f32>, py: &mut RVec<f32>) -> i32 {
    let n = px.len() - 1;
    let mut n2 = n;
    let mut n4 = n / 4;

    while 2 < n2 {
        let e = two_pi() / float_of_int(n2);
        let e3 = 3.0 * e;
        let mut a: f32 = 0.0;
        let mut a3: f32 = 0.0;
        let mut j = 1;
        while j <= n4 {
            let cc1 = cos(a);
            let ss1 = sin(a);
            let cc3 = cos(a3);
            let ss3 = sin(a3);
            a = a + e;
            a3 = a3 + e3;

            let mut is = j;
            let mut id = 2 * n2;
            while is < n {
                // INV 0 <= is, 0 <= n2 <= id
                let mut i0 = is;
                let mut i1 = i0 + n4;
                let mut i2 = i1 + n4;
                let mut i3 = i2 + n4;

                while i3 <= n {
                    // INV 0 <= i0 <= i1 <= i2 <= i3, 0 <= id

                    let r1 = px[i0] - px[i2];
                    px[i0] = px[i0] + px[i2 + 2]; //~ ERROR precondition might not hold
                    let r2 = px[i1] - px[i3];
                    px[i1] = px[i1] + px[i3];
                    let s1 = py[i0] - py[i2];
                    py[i0] = py[i0] + py[i2];
                    let s2 = py[i1] - py[i3];
                    py[i1] = py[i1] + py[i3];

                    let s3 = r1 - s2;
                    let r1 = r1 + s2;
                    let s2 = r2 - s1;
                    let r2 = r2 + s1;
                    px[i2] = r1 * cc1 - s2 * ss1;
                    py[i2] = (0. - s2) * cc1 - r1 * ss1;
                    px[i3] = s3 * cc3 + r2 * ss3;
                    py[i3] = r2 * cc3 - s3 * ss3;

                    i0 = i0 + id;
                    i1 = i1 + id;
                    i2 = i2 + id;
                    i3 = i3 + id;
                }
                // end loop1

                is = 2 * id - n2 + j;
                id = 4 * id;
            }
            // end loop2
            j += 1
        }
        n2 = n2 / 2;
        n4 = n4 / 2;
    }
    0
}

#[flux::sig(fn(&mut RVec<f32>[@n], &mut RVec<f32>[n]) -> i32)]
fn loop_b(px: &mut RVec<f32>, py: &mut RVec<f32>) -> i32 {
    let n = px.len() - 1;
    let mut is = 1;
    let mut id = 4;
    while is < n {
        // INV: 0 <= is, 4 <= id
        let mut i0 = is;
        let mut i1 = is + 1;
        while i1 <= n {
            // INV: 0 <= i0 <= i1, 0 <= id
            let r1 = px[i0];
            px[i0] = r1 + px[i1];
            px[i1] = r1 - px[i1];

            let r1 = py[i0];
            py[i0] = r1 + py[i1];
            py[i1] = r1 - py[i1];

            i0 = i0 + id;
            i1 = i1 + id;
        }
        is = 2 * id - 1;
        id = 4 * id;
    }
    0
}

#[flux::sig(
fn(&mut RVec<f32>[@n], &mut RVec<f32>[n]) -> i32
requires 2 <= n
)]
fn loop_c(px: &mut RVec<f32>, py: &mut RVec<f32>) -> i32 {
    let n = px.len() - 1;
    let mut i = 1;
    let mut j = 1;
    while i < n {
        // INV: 0 <= i, 0 <= j <= n
        if i < j {
            let xt = px[j]; //~ ERROR precondition might not hold
            px[j] = px[i]; //~ ERROR precondition might not hold
            px[i] = xt;

            let xt = py[j]; //~ ERROR precondition might not hold
            py[j] = py[i]; //~ ERROR precondition might not hold
            py[i] = xt;
        }
        i += 1;
        j = loop_c1(j, n / 2);
    }
    0
}

#[flux::sig(fn(j: usize{0 <= j}, k: usize{0 <= k}) -> usize{v: 0 <= v})]
pub fn loop_c1(j: usize, k: usize) -> usize {
    if j <= k {
        j + k
    } else {
        loop_c1(j - k, k / 2)
    }
}

#[flux::sig(fn(np: usize{2 <= np}) -> f32)]
pub fn fft_test(np: usize) -> f32 {
    let enp = float_of_int(np);
    let n2 = np / 2;
    let npm = n2 - 1;
    let mut pxr = RVec::from_elem_n(0.0, np + 1);
    let mut pxi = RVec::from_elem_n(0.0, np + 1);
    let t = pi() / enp;
    pxr[1] = (enp - 1.0) * 0.5;
    pxi[1] = 0.0;
    pxr[n2 + 1] = 0.0 - 0.5;
    pxi[n2 + 1] = 0.0;
    let mut i = 1;
    while i <= npm {
        let j = np - i;
        pxr[i + 1] = 0.0 - 0.5;
        pxr[j + 1] = 0.0 - 0.5;
        let z = t * float_of_int(i);
        let y = 0.5 * cos(z) / sin(z);
        pxi[i + 1] = 0.0 - y;
        pxi[j + 1] = y;
        i += 1;
    }

    fft(&mut pxr, &mut pxi);

    let mut zr = 0.0;
    let mut zi = 0.0;
    let mut _kr = 0;
    let mut _ki = 0;
    let mut i = 0;
    while i < np {
        let a = fabs(pxr[i + 1] - float_of_int(i));
        if zr < a {
            zr = a;
            _kr = i;
        }
        let a = fabs(pxi[i + 1]);
        if zi < a {
            zi = a;
            _ki = i;
        }
        i += 1;
    }
    if fabs(zr) < fabs(zi) {
        zi
    } else {
        zr
    }
}

#[flux::sig(fn() -> i32)]
pub fn doit() -> i32 {
    let mut i = 4;
    let mut np = 16;
    while i <= 16 {
        fft_test(np);
        i = i + 1;
        np = np * 2;
    }
    0
}
