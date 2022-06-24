//! Testing the imports with different provided functions.
//! This tests checks that the provided functions (both native and
//! dynamic ones) work properly.

use anyhow::Result;
use std::convert::Infallible;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};
use wasmer::Context as WasmerContext;
use wasmer::Type as ValueType;
use wasmer::*;

fn get_module(store: &Store) -> Result<Module> {
    let wat = r#"
        (import "host" "0" (func))
        (import "host" "1" (func (param i32) (result i32)))
        (import "host" "2" (func (param i32) (param i64)))
        (import "host" "3" (func (param i32 i64 i32 f32 f64)))
        (memory $mem 1)
        (export "memory" (memory $mem))

        (func $foo
            call 0
            i32.const 0
            call 1
            i32.const 1
            i32.add
            i64.const 3
            call 2

            i32.const 100
            i64.const 200
            i32.const 300
            f32.const 400
            f64.const 500
            call 3
        )
        (start $foo)
    "#;

    let module = Module::new(store, &wat)?;
    Ok(module)
}

#[compiler_test(imports)]
#[serial_test::serial(dynamic_function)]
fn dynamic_function(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module(&store)?;
    let mut ctx = WasmerContext::new(&store, ());
    static HITS: AtomicUsize = AtomicUsize::new(0);
    let imports = imports! {
        "host" => {
            "0" => Function::new(&mut ctx, FunctionType::new(vec![], vec![]), |_ctx, _values| {
                    assert_eq!(HITS.fetch_add(1, SeqCst), 0);
                    Ok(vec![])
                }),
            "1" => Function::new(&mut ctx, FunctionType::new(vec![ValueType::I32], vec![ValueType::I32]), |_ctx, values| {
                    assert_eq!(values[0], Value::I32(0));
                    assert_eq!(HITS.fetch_add(1, SeqCst), 1);
                    Ok(vec![Value::I32(1)])
                }),
            "2" => Function::new(&mut ctx, FunctionType::new(vec![ValueType::I32, ValueType::I64], vec![]), |_ctx, values| {
                    assert_eq!(values[0], Value::I32(2));
                    assert_eq!(values[1], Value::I64(3));
                    assert_eq!(HITS.fetch_add(1, SeqCst), 2);
                    Ok(vec![])
                }),
            "3" => Function::new(&mut ctx, FunctionType::new(vec![ValueType::I32, ValueType::I64, ValueType::I32, ValueType::F32, ValueType::F64], vec![]), |_ctx, values| {
                    assert_eq!(values[0], Value::I32(100));
                    assert_eq!(values[1], Value::I64(200));
                    assert_eq!(values[2], Value::I32(300));
                    assert_eq!(values[3], Value::F32(400.0));
                    assert_eq!(values[4], Value::F64(500.0));
                    assert_eq!(HITS.fetch_add(1, SeqCst), 3);
                    Ok(vec![])
                }),
        }
    };
    Instance::new(&mut ctx, &module, &imports)?;
    assert_eq!(HITS.swap(0, SeqCst), 4);
    Ok(())
}

#[compiler_test(imports)]
fn dynamic_function_with_env(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module(&store)?;

    #[derive(Clone)]
    struct Env {
        counter: Arc<AtomicUsize>,
    }

    impl std::ops::Deref for Env {
        type Target = Arc<AtomicUsize>;
        fn deref(&self) -> &Self::Target {
            &self.counter
        }
    }

    let env: Env = Env {
        counter: Arc::new(AtomicUsize::new(0)),
    };
    let mut ctx = WasmerContext::new(&store, env);
    let f0 = Function::new(
        &mut ctx,
        FunctionType::new(vec![], vec![]),
        |ctx, _values| {
            assert_eq!(ctx.data().fetch_add(1, SeqCst), 0);
            Ok(vec![])
        },
    );
    let f1 = Function::new(
        &mut ctx,
        FunctionType::new(vec![ValueType::I32], vec![ValueType::I32]),
        |ctx, values| {
            assert_eq!(values[0], Value::I32(0));
            assert_eq!(ctx.data().fetch_add(1, SeqCst), 1);
            Ok(vec![Value::I32(1)])
        },
    );
    let f2 = Function::new(
        &mut ctx,
        FunctionType::new(vec![ValueType::I32, ValueType::I64], vec![]),
        |ctx, values| {
            assert_eq!(values[0], Value::I32(2));
            assert_eq!(values[1], Value::I64(3));
            assert_eq!(ctx.data().fetch_add(1, SeqCst), 2);
            Ok(vec![])
        },
    );
    let f3 = Function::new(
        &mut ctx,
        FunctionType::new(
            vec![
                ValueType::I32,
                ValueType::I64,
                ValueType::I32,
                ValueType::F32,
                ValueType::F64,
            ],
            vec![],
        ),
        |ctx, values| {
            assert_eq!(values[0], Value::I32(100));
            assert_eq!(values[1], Value::I64(200));
            assert_eq!(values[2], Value::I32(300));
            assert_eq!(values[3], Value::F32(400.0));
            assert_eq!(values[4], Value::F64(500.0));
            assert_eq!(ctx.data().fetch_add(1, SeqCst), 3);
            Ok(vec![])
        },
    );
    Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "0" => f0,
                "1" => f1,
                "2" => f2,
                "3" => f3,
            },
        },
    )?;
    assert_eq!(ctx.data_mut().load(SeqCst), 4);
    Ok(())
}

#[compiler_test(imports)]
#[serial_test::serial(static_function)]
fn static_function(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module(&store)?;

    static HITS: AtomicUsize = AtomicUsize::new(0);
    let mut ctx = WasmerContext::new(&store, ());
    let f0 = Function::new_native(&mut ctx, |_ctx: ContextMut<_>| {
        assert_eq!(HITS.fetch_add(1, SeqCst), 0);
    });
    let f1 = Function::new_native(&mut ctx, |_ctx: ContextMut<_>, x: i32| -> i32 {
        assert_eq!(x, 0);
        assert_eq!(HITS.fetch_add(1, SeqCst), 1);
        1
    });
    let f2 = Function::new_native(&mut ctx, |_ctx: ContextMut<_>, x: i32, y: i64| {
        assert_eq!(x, 2);
        assert_eq!(y, 3);
        assert_eq!(HITS.fetch_add(1, SeqCst), 2);
    });
    let f3 = Function::new_native(
        &mut ctx,
        |_ctx: ContextMut<_>, a: i32, b: i64, c: i32, d: f32, e: f64| {
            assert_eq!(a, 100);
            assert_eq!(b, 200);
            assert_eq!(c, 300);
            assert_eq!(d, 400.0);
            assert_eq!(e, 500.0);
            assert_eq!(HITS.fetch_add(1, SeqCst), 3);
        },
    );
    Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "0" => f0,
                "1" => f1,
                "2" => f2,
                "3" => f3,
            },
        },
    )?;
    assert_eq!(HITS.swap(0, SeqCst), 4);
    Ok(())
}

#[compiler_test(imports)]
#[serial_test::serial(static_function_with_results)]
fn static_function_with_results(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module(&store)?;

    static HITS: AtomicUsize = AtomicUsize::new(0);
    let mut ctx = WasmerContext::new(&store, ());
    let f0 = Function::new_native(&mut ctx, |_ctx: ContextMut<_>| {
        assert_eq!(HITS.fetch_add(1, SeqCst), 0);
    });
    let f1 = Function::new_native(
        &mut ctx,
        |_ctx: ContextMut<_>, x: i32| -> Result<i32, Infallible> {
            assert_eq!(x, 0);
            assert_eq!(HITS.fetch_add(1, SeqCst), 1);
            Ok(1)
        },
    );
    let f2 = Function::new_native(&mut ctx, |_ctx: ContextMut<_>, x: i32, y: i64| {
        assert_eq!(x, 2);
        assert_eq!(y, 3);
        assert_eq!(HITS.fetch_add(1, SeqCst), 2);
    });
    let f3 = Function::new_native(
        &mut ctx,
        |_ctx: ContextMut<_>, a: i32, b: i64, c: i32, d: f32, e: f64| {
            assert_eq!(a, 100);
            assert_eq!(b, 200);
            assert_eq!(c, 300);
            assert_eq!(d, 400.0);
            assert_eq!(e, 500.0);
            assert_eq!(HITS.fetch_add(1, SeqCst), 3);
        },
    );
    Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "0" => f0,
                "1" => f1,
                "2" => f2,
                "3" => f3,
            },
        },
    )?;
    assert_eq!(HITS.swap(0, SeqCst), 4);
    Ok(())
}

#[compiler_test(imports)]
fn static_function_with_env(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module(&store)?;

    #[derive(Clone)]
    struct Env(Arc<AtomicUsize>);

    impl std::ops::Deref for Env {
        type Target = Arc<AtomicUsize>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    let env: Env = Env(Arc::new(AtomicUsize::new(0)));
    let mut ctx = WasmerContext::new(&store, env);
    let f0 = Function::new_native(&mut ctx, |ctx: ContextMut<Env>| {
        assert_eq!(ctx.data().fetch_add(1, SeqCst), 0);
    });
    let f1 = Function::new_native(&mut ctx, |ctx: ContextMut<Env>, x: i32| -> i32 {
        assert_eq!(x, 0);
        assert_eq!(ctx.data().fetch_add(1, SeqCst), 1);
        1
    });
    let f2 = Function::new_native(&mut ctx, |ctx: ContextMut<Env>, x: i32, y: i64| {
        assert_eq!(x, 2);
        assert_eq!(y, 3);
        assert_eq!(ctx.data().fetch_add(1, SeqCst), 2);
    });
    let f3 = Function::new_native(
        &mut ctx,
        |ctx: ContextMut<Env>, a: i32, b: i64, c: i32, d: f32, e: f64| {
            assert_eq!(a, 100);
            assert_eq!(b, 200);
            assert_eq!(c, 300);
            assert_eq!(d, 400.0);
            assert_eq!(e, 500.0);
            assert_eq!(ctx.data().fetch_add(1, SeqCst), 3);
        },
    );
    Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "0" => f0,
                "1" => f1,
                "2" => f2,
                "3" => f3,
            },
        },
    )?;
    assert_eq!(ctx.data_mut().load(SeqCst), 4);
    Ok(())
}

#[compiler_test(imports)]
fn static_function_that_fails(config: crate::Config) -> Result<()> {
    let store = config.store();
    let wat = r#"
        (import "host" "0" (func))

        (func $foo
            call 0
        )
        (start $foo)
    "#;

    let module = Module::new(&store, &wat)?;
    let mut ctx = WasmerContext::new(&store, ());
    let f0 = Function::new_native(
        &mut ctx,
        |_ctx: ContextMut<_>| -> Result<Infallible, RuntimeError> {
            Err(RuntimeError::new("oops"))
        },
    );
    let result = Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "0" => f0,
            },
        },
    );

    assert!(result.is_err());

    match result {
        Err(InstantiationError::Start(runtime_error)) => {
            assert_eq!(runtime_error.message(), "oops")
        }
        _ => assert!(false),
    }

    Ok(())
}

fn get_module2(store: &Store) -> Result<Module> {
    let wat = r#"
        (import "host" "fn" (func))
        (memory $mem 1)
        (export "memory" (memory $mem))
        (export "main" (func $main))
        (func $main (param) (result)
          (call 0))
    "#;

    let module = Module::new(store, &wat)?;
    Ok(module)
}

#[compiler_test(imports)]
fn dynamic_function_with_env_wasmer_env_init_works(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module2(&store)?;

    #[allow(dead_code)]
    #[derive(Clone)]
    struct Env {
        memory: Option<Memory>,
    }

    let env: Env = Env { memory: None };
    let mut ctx = WasmerContext::new(&store, env);
    let f0 = Function::new(
        &mut ctx,
        FunctionType::new(vec![], vec![]),
        |ctx, _values| {
            assert!(ctx.data().memory.as_ref().is_some());
            Ok(vec![])
        },
    );
    let instance = Instance::new(
        &mut ctx,
        &module,
        &imports! {
            "host" => {
                "fn" => f0,
            },
        },
    )?;
    let memory = instance.exports.get_memory("memory")?;
    ctx.data_mut().memory = Some(memory.clone());
    let f: TypedFunction<(), ()> = instance.exports.get_typed_function(&ctx, "main")?;
    f.call(&mut ctx)?;
    Ok(())
}

#[compiler_test(imports)]
fn multi_use_host_fn_manages_memory_correctly(config: crate::Config) -> Result<()> {
    let store = config.store();
    let module = get_module2(&store)?;

    #[allow(dead_code)]
    #[derive(Clone)]
    struct Env {
        memory: Option<Memory>,
    }

    /*    impl WasmerEnv for Env {
        fn init_with_instance(&mut self, instance: &Instance) -> Result<(), HostEnvInitError> {
            let memory = instance.exports.get_memory("memory")?.clone();
            self.memory.initialize(memory);
            Ok(())
        }
    }*/

    let env: Env = Env { memory: None };
    let mut ctx = WasmerContext::new(&store, env);
    fn host_fn(ctx: ContextMut<Env>) {
        assert!(ctx.data().memory.is_some());
        println!("Hello, world!");
    }
    let imports = imports! {
        "host" => {
            "fn" => Function::new_native(&mut ctx, host_fn),
        },
    };
    let instance1 = Instance::new(&mut ctx, &module, &imports)?;
    let instance2 = Instance::new(&mut ctx, &module, &imports)?;
    {
        let f1: TypedFunction<(), ()> = instance1.exports.get_typed_function(&mut ctx, "main")?;
        let memory = instance1.exports.get_memory("memory")?;
        ctx.data_mut().memory = Some(memory.clone());
        f1.call(&mut ctx)?;
    }
    drop(instance1);
    {
        let f2: TypedFunction<(), ()> = instance2.exports.get_typed_function(&mut ctx, "main")?;
        let memory = instance2.exports.get_memory("memory")?;
        ctx.data_mut().memory = Some(memory.clone());
        f2.call(&mut ctx)?;
    }
    drop(instance2);
    Ok(())
}

#[compiler_test(imports)]
fn instance_local_memory_lifetime(config: crate::Config) -> Result<()> {
    let store = config.store();
    let mut ctx = WasmerContext::new(&store, ());
    let memory: Memory = {
        let wat = r#"(module
    (memory $mem 1)
    (export "memory" (memory $mem))
)"#;
        let module = Module::new(&store, wat)?;
        let instance = Instance::new(&mut ctx, &module, &imports! {})?;
        instance.exports.get_memory("memory")?.clone()
    };

    let wat = r#"(module
    (import "env" "memory" (memory $mem 1) )
    (func $get_at (type $get_at_t) (param $idx i32) (result i32)
      (i32.load (local.get $idx)))
    (type $get_at_t (func (param i32) (result i32)))
    (type $set_at_t (func (param i32) (param i32)))
    (func $set_at (type $set_at_t) (param $idx i32) (param $val i32)
      (i32.store (local.get $idx) (local.get $val)))
    (export "get_at" (func $get_at))
    (export "set_at" (func $set_at))
)"#;
    let module = Module::new(&store, wat)?;
    let imports = imports! {
        "env" => {
            "memory" => memory,
        },
    };
    let instance = Instance::new(&mut ctx, &module, &imports)?;
    let set_at: TypedFunction<(i32, i32), ()> =
        instance.exports.get_typed_function(&mut ctx, "set_at")?;
    let get_at: TypedFunction<i32, i32> =
        instance.exports.get_typed_function(&mut ctx, "get_at")?;
    set_at.call(&mut ctx, 200, 123)?;
    assert_eq!(get_at.call(&mut ctx, 200)?, 123);

    Ok(())
}
