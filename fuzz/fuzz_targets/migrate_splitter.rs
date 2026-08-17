#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        // split_statements should be resilient to arbitrary UTF-8 SQL text.
        let _ = ruprizzle_migrate::runner::split_statements(source);
    }
});
