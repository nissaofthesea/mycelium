use crate::{report, Test};
use core::{
    ffi,
    fmt::Write,
    mem, ptr, slice,
    sync::atomic::{AtomicPtr, Ordering},
};
use mycelium_trace::writer::MakeWriter;

// These symbols are auto-generated by lld (and similar linkers) for data
// `link_section` sections, and are located at the beginning and end of the
// section.
//
// The memory region between the two symbols will contain an array of `Test`
// instances.
extern "C" {
    static __start_MyceliumTests: ffi::c_void;
    static __stop_MyceliumTests: ffi::c_void;
}

static CURRENT_TEST: AtomicPtr<Test> = AtomicPtr::new(ptr::null_mut());

/// Run all tests linked into the current binary, outputting test reports to the
/// provided `mk_writer`.
///
/// # Returns
///
/// - `Err(())` if any test failed
/// - `Ok(())` if all tests passed
pub fn run_tests(mk_writer: impl for<'writer> MakeWriter<'writer>) -> Result<(), ()> {
    let _span = tracing::info_span!("run tests").entered();

    let mut passed = 0;
    let mut failed = 0;
    let tests = all_tests();
    writeln!(
        mk_writer.make_writer(),
        "{}{}",
        report::TEST_COUNT,
        tests.len()
    )
    .expect("write failed");
    for test in tests {
        CURRENT_TEST.store(test as *const _ as *mut _, Ordering::Release);

        writeln!(
            mk_writer.make_writer(),
            "{}{} {}",
            report::START_TEST,
            test.descr.module,
            test.descr.name
        )
        .expect("write failed");

        let _span =
            tracing::info_span!("test", name = %test.descr.name, module = %test.descr.module)
                .entered();

        let outcome = (test.run)();
        tracing::info!(?outcome);
        CURRENT_TEST.store(ptr::null_mut(), Ordering::Release);
        test.write_outcome(outcome, mk_writer.make_writer())
            .expect("write failed");
        if outcome.is_ok() {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    tracing::warn!("{} passed | {} failed", passed, failed);

    if failed > 0 {
        Err(())
    } else {
        Ok(())
    }
}

/// Returns the current test, if a test is currently running.
pub fn current_test() -> Option<&'static Test> {
    let ptr = CURRENT_TEST.load(Ordering::Acquire);
    ptr::NonNull::new(ptr).map(|ptr| unsafe {
        // Safety: the current test is always set from a `&'static`ally declared `Test`.
        &*(ptr.as_ptr() as *const _)
    })
}

/// Get a list of `Test` objects.
pub fn all_tests() -> &'static [Test] {
    unsafe {
        // FIXME: These should probably be `&raw const __start_*`.
        let start: *const ffi::c_void = &__start_MyceliumTests;
        let stop: *const ffi::c_void = &__stop_MyceliumTests;

        let len_bytes = (stop as usize) - (start as usize);
        let len = len_bytes / mem::size_of::<Test>();
        assert!(
            len_bytes % mem::size_of::<Test>() == 0,
            "Section should contain a whole number of `Test`s"
        );

        if len > 0 {
            slice::from_raw_parts(start as *const Test, len)
        } else {
            &[]
        }
    }
}