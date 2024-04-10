use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum Error {
    #[error("syntax error")]
    #[diagnostic(code(gelatin::syntax_error))]
    Syntax {
        #[source_code]
        source_code: NamedSource<String>,
        #[label("here")]
        at: SourceSpan,

        #[help]
        expected: Option<String>,
    },

    #[error("sql syntax error")]
    #[diagnostic(code(gelatin::sql_syntax_error))]
    SqlSyntax {
        #[source_code]
        source_code: NamedSource<String>,
        #[label("here")]
        at: SourceSpan,

        #[help]
        err: String,
    },

    #[error("alias is unbound error")]
    #[diagnostic(code(gelatin::unbound_alias))]
    UnboundAlias {
        #[source_code]
        source_code: String,
        #[label("alias here")]
        at: SourceSpan,
    },

    #[error("name is unbound error")]
    #[diagnostic(code(gelatin::unbound_name))]
    UnboundName {
        #[source_code]
        source_code: String,
        #[label("name here")]
        at: SourceSpan,
    },

    #[error("value error: {message}")]
    #[diagnostic(code(gelatin::value_error))]
    Value { message: String },
}
