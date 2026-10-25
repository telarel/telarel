pub mod canonical;
pub mod start_end;

use oxc::ast::ast::CommentContent;

use crate::comment::adapted::AdaptedComment;
use crate::comment::normalize::CommentContext;
use crate::comment::raw::RawComment;

/// A dialect adapter: reads the comment fields one producer shape carries.
///
/// Adapters are pure and return partial data. The normalizer folds them in
/// registry order with the first `Some` value winning per field.
pub trait CommentAdapter {
    /// Adapt one raw comment; unknown labels are errors.
    fn adapt(
        &self,
        raw: &RawComment,
        ctx: &CommentContext<'_>,
    ) -> anyhow::Result<AdaptedComment>;
}

/// Parse a JS `content` label into its oxc content kind.
pub fn content_from_label(label: &str) -> anyhow::Result<CommentContent> {
    let content: CommentContent = match label {
        | "none" => CommentContent::None,
        | "legal" => CommentContent::Legal,
        | "jsdoc" => CommentContent::Jsdoc,
        | "jsdoc-legal" => CommentContent::JsdocLegal,
        | "pure" => CommentContent::Pure,
        | "pure-not-applied" => CommentContent::PureNotApplied,
        | "no-side-effects" => CommentContent::NoSideEffects,
        | "no-side-effects-not-applied" => {
            CommentContent::NoSideEffectsNotApplied
        },
        | "webpack" => CommentContent::Webpack,
        | "vite" => CommentContent::Vite,
        | "turbopack" => CommentContent::Turbopack,
        | "coverage-ignore" => CommentContent::CoverageIgnore,
        | "coverage-ignore-file" => CommentContent::CoverageIgnoreFile,
        | "property-key" => CommentContent::PropertyKey,
        | other => {
            anyhow::bail!("unsupported content \"{other}\"")
        },
    };

    Ok(content)
}

/// The JS label for an oxc content kind.
pub fn content_label(content: CommentContent) -> &'static str {
    match content {
        | CommentContent::None => "none",
        | CommentContent::Legal => "legal",
        | CommentContent::Jsdoc => "jsdoc",
        | CommentContent::JsdocLegal => "jsdoc-legal",
        | CommentContent::Pure => "pure",
        | CommentContent::PureNotApplied => "pure-not-applied",
        | CommentContent::NoSideEffects => "no-side-effects",
        | CommentContent::NoSideEffectsNotApplied => {
            "no-side-effects-not-applied"
        },
        | CommentContent::Webpack => "webpack",
        | CommentContent::Vite => "vite",
        | CommentContent::Turbopack => "turbopack",
        | CommentContent::CoverageIgnore => "coverage-ignore",
        | CommentContent::CoverageIgnoreFile => "coverage-ignore-file",
        | CommentContent::PropertyKey => "property-key",
    }
}

/// Telarel's extended placement metadata.
pub static CANONICAL: canonical::CanonicalAdapter = canonical::CanonicalAdapter;

/// Standard `start`/`end` spans (Acorn, oxc, Esprima).
pub static START_END: start_end::StartEndAdapter = start_end::StartEndAdapter;

/// The default adapter registry, in precedence order.
pub const ADAPTERS: &[&dyn CommentAdapter] = &[&CANONICAL, &START_END];

#[cfg(test)]
mod tests {
    use crate::comment::adapters::{content_from_label, content_label};

    #[test]
    fn test_content_labels_round_trip() {
        let labels: [&str; 14] = [
            "none",
            "legal",
            "jsdoc",
            "jsdoc-legal",
            "pure",
            "pure-not-applied",
            "no-side-effects",
            "no-side-effects-not-applied",
            "webpack",
            "vite",
            "turbopack",
            "coverage-ignore",
            "coverage-ignore-file",
            "property-key",
        ];

        for label in labels {
            let content: oxc::ast::ast::CommentContent =
                content_from_label(label).expect("the label parses");

            assert_eq!(content_label(content), label, "{label}");
        }
    }

    #[test]
    fn test_content_from_label_rejects_unknown() {
        assert!(content_from_label("mystery").is_err());
    }
}
