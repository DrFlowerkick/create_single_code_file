// some utilities to use in project

// macro for adding file and line info to messages
#[macro_export]
macro_rules! add_context {
    ($message:expr) => {
        format!("{} ({}:{})", $message, file!(), line!())
    };
}
