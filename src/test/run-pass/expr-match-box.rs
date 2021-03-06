// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(managed_boxes)]

use std::gc::GC;

// Tests for match as expressions resulting in boxed types
fn test_box() {
    let res = match true { true => { box(GC) 100 } _ => fail!("wat") };
    assert_eq!(*res, 100);
}

fn test_str() {
    let res = match true { true => { "happy".to_string() },
                         _ => fail!("not happy at all") };
    assert_eq!(res, "happy".to_string());
}

pub fn main() { test_box(); test_str(); }
