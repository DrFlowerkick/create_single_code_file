// some utilities to use in project

// macro for adding file and line info to messages
#[macro_export]
macro_rules! add_context {
    ($message:expr) => {
        format!("{} ({}:{})", $message, file!(), line!())
    };
}

// codingame supports the following crates from crates.io
// see https://www.codingame.com/playgrounds/40701/help-center/languages-versions
// chrono 0.4.26, itertools 0.11.0, libc 0.2.147, rand 0.8.5, regex 1.8.4, time 0.3.22
// we ignore for now version numbers
pub const CODINGAME_SUPPORTED_CRATES: [&str; 6] =
    ["chrono", "itertools", "libc", "rand", "regex", "time"];
