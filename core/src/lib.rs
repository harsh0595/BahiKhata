pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_set() {
        assert!(!super::VERSION.is_empty());
    }
}
