use telarel_common::CompileError;

/// Arguments for the `end` hook: the error, when the compile failed.
///
/// The hook is notify-only; its return value is ignored. It is invoked on
/// every settled plugin, on success and on every error path.
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
        let args: EndArgs =
            EndArgs { error: Some(CompileError::from_message("boom")) };

        assert_eq!(
            args.error.as_ref().map(ToString::to_string).as_deref(),
            Some("boom")
        );
    }
}
