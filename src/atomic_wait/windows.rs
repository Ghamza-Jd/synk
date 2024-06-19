use core::sync::atomic::AtomicU32;
use windows_sys::Win32::System::Threading::WaitOnAddress;
use windows_sys::Win32::System::Threading::WakeByAddressAll;
use windows_sys::Win32::System::Threading::WakeByAddressSingle;
use windows_sys::Win32::System::WindowsProgramming::INFINITE;

#[inline]
pub fn wait(a: &AtomicU32, expected: u32) {
    let ptr: *const AtomicU32 = a;
    let expected_ptr: *const u32 = &expected;
    unsafe { WaitOnAddress(ptr.cast(), expected_ptr.cast(), 4, INFINITE) };
}

#[inline]
pub fn wake_one(ptr: *const AtomicU32) {
    unsafe { WakeByAddressSingle(ptr.cast()) };
}

#[inline]
pub fn wake_all(ptr: *const AtomicU32) {
    unsafe { WakeByAddressAll(ptr.cast()) };
}
