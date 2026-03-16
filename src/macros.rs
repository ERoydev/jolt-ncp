//! Macro to measure execution time of an expression.
//! Returns a tuple of (result, duration).
#[macro_export]
macro_rules! time_it {
    ($expr:expr) => {{
        let start = std::time::Instant::now();
        let result = $expr;
        (result, start.elapsed())
    }};
}
