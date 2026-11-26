//! OS calls that need `unsafe`: their one home in the core.

#[cfg(target_os = "macos")]
pub(crate) mod mac;

pub(crate) fn clear_sensitive(text: &mut String) {
    // SAFETY: every byte of this exclusively borrowed allocation is writable; zero is UTF-8.
    unsafe {
        let bytes = text.as_mut_vec();
        for offset in 0..bytes.capacity() {
            bytes.as_mut_ptr().add(offset).write_volatile(0);
        }
        bytes.clear();
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}
