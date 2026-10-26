use telarel_common::CompileError;

/// Arguments for the `end` hook: the error, when the compile failed.
///
/// The hook is notify-only, and its return value is ignored. `end` runs on
/// every settled plugin on every path, including a compile failure, but not
/// when the `options` stage fails. A failing `end` returns its own error
/// instead of the stage error. On the success path, the `end` error is the
/// returned error.
#[derive(Debug)]
pub struct EndArgs {
    /// The error, when the compile failed.
    pub error: Option<CompileError>,
}

#[cfg(test)]
mod tests {
    use telarel_common::CompileError;

    use super::*;

    #[test]
    fn test_end_args_on_success() {
        let args: EndArgs = EndArgs { error: None };

        assert!(args.error.is_none());
    }

    #[test]
    fn test_end_args_on_error() {
        let args: EndArgs = EndArgs { error: Some(CompileError::new("boom")) };

        assert_eq!(
            args.error.as_ref().map(ToString::to_string).as_deref(),
            Some("boom")
        );
    }
}
