//! Core functionality.

/// Returns a greeting. Replace it with real logic.
pub fn hello(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_greets_by_name() {
        assert_eq!(hello("world"), "Hello, world!");
    }
}
