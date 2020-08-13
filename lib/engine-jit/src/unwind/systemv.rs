// This file contains code from external sources.
// Attributions: https://github.com/wasmerio/wasmer/blob/master/ATTRIBUTIONS.md

//! Module for System V ABI unwind registry.
use wasmer_compiler::CompiledFunctionUnwindInfo;

/// Represents a registry of function unwind information for System V ABI.
pub struct UnwindRegistry {
    registrations: Vec<usize>,
    published: bool,
}

extern "C" {
    // libunwind import
    fn __register_frame(fde: *const u8);
    fn __deregister_frame(fde: *const u8);
}

impl UnwindRegistry {
    /// Creates a new unwind registry with the given base address.
    pub fn new() -> Self {
        Self {
            registrations: Vec::new(),
            published: false,
        }
    }

    /// Registers a function given the start offset, length, and unwind information.
    pub fn register(
        &mut self,
        _base_address: usize,
        _func_start: u32,
        _func_len: u32,
        info: &CompiledFunctionUnwindInfo,
    ) -> Result<(), String> {
        match info {
            CompiledFunctionUnwindInfo::Dwarf => {}
            _ => return Err("unsupported unwind information".to_string()),
        };
        Ok(())
    }

    /// Publishes all registered functions.
    pub fn publish(&mut self, eh_frame: Option<&[u8]>) -> Result<(), String> {
        if self.published {
            return Err("unwind registry has already been published".to_string());
        }

        if let Some(eh_frame) = eh_frame {
            unsafe {
                self.register_frames(eh_frame);
            }
        }

        self.published = true;

        Ok(())
    }

    #[allow(clippy::cast_ptr_alignment)]
    unsafe fn register_frames(&mut self, eh_frame: &[u8]) {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "macos")] {
                // On macOS, `__register_frame` takes a pointer to a single FDE
                let start = eh_frame.as_ptr();
                let end = start.add(eh_frame.len());
                let mut current = start;

                // Walk all of the entries in the frame table and register them
                while current < end {
                    let len = std::ptr::read::<u32>(current as *const u32) as usize;

                    // Skip over the CIE
                    if current != start {
                        __register_frame(current);
                        self.registrations.push(current as usize);
                    }

                    // Move to the next table entry (+4 because the length itself is not inclusive)
                    current = current.add(len + 4);
                }
            } else {
                // On other platforms, `__register_frame` will walk the FDEs until an entry of length 0
                let ptr = eh_frame.as_ptr();
                __register_frame(ptr);
                self.registrations.push(ptr as usize);
            }
        }
    }
}

impl Drop for UnwindRegistry {
    fn drop(&mut self) {
        if self.published {
            unsafe {
                // libgcc stores the frame entries as a linked list in decreasing sort order
                // based on the PC value of the registered entry.
                //
                // As we store the registrations in increasing order, it would be O(N^2) to
                // deregister in that order.
                //
                // To ensure that we just pop off the first element in the list upon every
                // deregistration, walk our list of registrations backwards.
                for fde in self.registrations.iter().rev() {
                    __deregister_frame(*fde as *const _);
                }
            }
        }
    }
}
