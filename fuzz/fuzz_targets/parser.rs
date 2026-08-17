#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        // The parser should never panic or hang on arbitrary UTF-8.
        let _ = ruprizzle_parser::parse("fuzz.ruprizzle", source);
    }
});
