use crate::utils::get_store_with_middlewares;
use anyhow::Result;
use wasmer_middlewares::Metering;

use std::sync::Arc;
use wasmer::wasmparser::Operator;
use wasmer::*;

fn cost_always_one(_: &Operator) -> u64 {
    1
}

fn run_add_with_limit(limit: u64) -> Result<()> {
    let store = get_store_with_middlewares(std::iter::once(Arc::new(Metering::new(
        limit,
        cost_always_one,
    )) as Arc<dyn ModuleMiddleware>));
    let wat = r#"(module
        (func (export "add") (param i32 i32) (result i32)
           (i32.add (local.get 0)
                    (local.get 1)))
)"#;
    let module = Module::new(&store, wat).unwrap();

    let import_object = imports! {};

    let instance = Instance::new(&module, &import_object)?;

    let f: NativeFunc<(i32, i32), i32> = instance.exports.get_native_function("add")?;
    f.call(4, 6)?;
    Ok(())
}

fn run_loop(limit: u64, iter_count: i32) -> Result<()> {
    let store = get_store_with_middlewares(std::iter::once(Arc::new(Metering::new(
        limit,
        cost_always_one,
    )) as Arc<dyn ModuleMiddleware>));
    let wat = r#"(module
        (func (export "test") (param i32)
           (local i32)
           (local.set 1 (i32.const 0))
           (loop
            (local.get 1)
            (i32.const 1)
            (i32.add)
            (local.tee 1)
            (local.get 0)
            (i32.ne)
            (br_if 0)
           )
        )
)"#;
    let module = Module::new(&store, wat).unwrap();

    let import_object = imports! {};

    let instance = Instance::new(&module, &import_object)?;

    let f: NativeFunc<i32, ()> = instance.exports.get_native_function("test")?;
    f.call(iter_count)?;
    Ok(())
}

#[test]
fn metering_ok() -> Result<()> {
    assert!(run_add_with_limit(4).is_ok());
    Ok(())
}

#[test]
fn metering_fail() -> Result<()> {
    assert!(run_add_with_limit(3).is_err());
    Ok(())
}

#[test]
fn loop_once() -> Result<()> {
    assert!(run_loop(12, 1).is_ok());
    assert!(run_loop(11, 1).is_err());
    Ok(())
}

#[test]
fn loop_twice() -> Result<()> {
    assert!(run_loop(19, 2).is_ok());
    assert!(run_loop(18, 2).is_err());
    Ok(())
}

/// Ported from https://github.com/wasmerio/wasmer/blob/master/tests/middleware_common.rs
#[test]
fn complex_loop() -> Result<()> {
    // Assemblyscript
    // export function add_to(x: i32, y: i32): i32 {
    //    for(var i = 0; i < x; i++){
    //      if(i % 1 == 0){
    //        y += i;
    //      } else {
    //        y *= i
    //      }
    //    }
    //    return y;
    // }
    static WAT: &'static str = r#"
    (module
        (type $t0 (func (param i32 i32) (result i32)))
        (type $t1 (func))
        (func $add_to (export "add_to") (type $t0) (param $p0 i32) (param $p1 i32) (result i32)
        (local $l0 i32)
        block $B0
            i32.const 0
            set_local $l0
            loop $L1
            get_local $l0
            get_local $p0
            i32.lt_s
            i32.eqz
            br_if $B0
            get_local $l0
            i32.const 1
            i32.rem_s
            i32.const 0
            i32.eq
            if $I2
                get_local $p1
                get_local $l0
                i32.add
                set_local $p1
            else
                get_local $p1
                get_local $l0
                i32.mul
                set_local $p1
            end
            get_local $l0
            i32.const 1
            i32.add
            set_local $l0
            br $L1
            unreachable
            end
            unreachable
        end
        get_local $p1)
        (func $f1 (type $t1))
        (table $table (export "table") 1 anyfunc)
        (memory $memory (export "memory") 0)
        (global $g0 i32 (i32.const 8))
        (elem (i32.const 0) $f1))
    "#;
    let store = get_store_with_middlewares(std::iter::once(Arc::new(Metering::new(
        100,
        cost_always_one,
    )) as Arc<dyn ModuleMiddleware>));
    let module = Module::new(&store, WAT).unwrap();

    let import_object = imports! {};

    let instance = Instance::new(&module, &import_object)?;

    let f: NativeFunc<(i32, i32), i32> = instance.exports.get_native_function("add_to")?;

    // FIXME: Since now a metering error is signaled with an `unreachable`, it is impossible to verify
    // the error type. Fix this later.
    f.call(10_000_000, 4).unwrap_err();
    Ok(())
}