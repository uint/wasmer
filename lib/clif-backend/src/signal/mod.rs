use crate::{
    relocation::{TrapData, TrapSink},
    resolver::FuncResolver,
    trampoline::Trampolines,
};
use libc::c_void;
use std::{cell::Cell, ptr::NonNull, sync::Arc};
use wasmer_runtime_core::{
    backend::RunnableModule,
    error::RuntimeError,
    module::ModuleInfo,
    typed_func::{Trampoline, Wasm},
    types::{LocalFuncIndex, SigIndex},
    vm,
};

#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use self::unix::*;

#[cfg(windows)]
pub use self::windows::*;

thread_local! {
    pub static TRAP_EARLY_DATA: Cell<Option<RuntimeError>> = Cell::new(None);
}

pub struct Caller {
    handler_data: HandlerData,
    trampolines: Arc<Trampolines>,
    resolver: FuncResolver,
}

impl Caller {
    pub fn new(
        handler_data: HandlerData,
        trampolines: Arc<Trampolines>,
        resolver: FuncResolver,
    ) -> Self {
        Self {
            handler_data,
            trampolines,
            resolver,
        }
    }
}

impl RunnableModule for Caller {
    fn get_func(&self, _: &ModuleInfo, func_index: LocalFuncIndex) -> Option<NonNull<vm::Func>> {
        self.resolver.lookup(func_index)
    }

    fn get_trampoline(&self, _: &ModuleInfo, sig_index: SigIndex) -> Option<Wasm> {
        unsafe extern "C" fn invoke(
            trampoline: Trampoline,
            ctx: *mut vm::Ctx,
            func: NonNull<vm::Func>,
            args: *const u64,
            rets: *mut u64,
            error_out: *mut Option<RuntimeError>,
            invoke_env: Option<NonNull<c_void>>,
        ) -> bool {
            let handler_data = &*invoke_env.unwrap().cast().as_ptr();

            #[cfg(not(target_os = "windows"))]
            let res = call_protected(handler_data, || {
                // Leap of faith.
                trampoline(ctx, func, args, rets);
            });

            // the trampoline is called from C on windows
            #[cfg(target_os = "windows")]
            let res = call_protected(handler_data, trampoline, ctx, func, args, rets);

            match res {
                Err(err) => {
                    *error_out = Some(err.into());
                    false
                }
                Ok(()) => true,
            }
        }

        let trampoline = self
            .trampolines
            .lookup(sig_index)
            .expect("that trampoline doesn't exist");

        Some(unsafe {
            Wasm::from_raw_parts(
                trampoline,
                invoke,
                Some(NonNull::from(&self.handler_data).cast()),
            )
        })
    }

    unsafe fn do_early_trap(&self, data: RuntimeError) -> ! {
        TRAP_EARLY_DATA.with(|cell| cell.set(Some(data)));
        trigger_trap()
    }
}

unsafe impl Send for HandlerData {}
unsafe impl Sync for HandlerData {}

#[derive(Clone)]
pub struct HandlerData {
    pub trap_data: Arc<TrapSink>,
    exec_buffer_ptr: *const c_void,
    exec_buffer_size: usize,
}

impl HandlerData {
    pub fn new(
        trap_data: Arc<TrapSink>,
        exec_buffer_ptr: *const c_void,
        exec_buffer_size: usize,
    ) -> Self {
        Self {
            trap_data,
            exec_buffer_ptr,
            exec_buffer_size,
        }
    }

    pub fn lookup(&self, ip: *const c_void) -> Option<TrapData> {
        let ip = ip as usize;
        let buffer_ptr = self.exec_buffer_ptr as usize;

        if buffer_ptr <= ip && ip < buffer_ptr + self.exec_buffer_size {
            let offset = ip - buffer_ptr;
            self.trap_data.lookup(offset)
        } else {
            None
        }
    }
}
