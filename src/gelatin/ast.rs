use std::{borrow::Cow, collections::HashMap, fmt::Write, sync::Arc};

use sqlparser::ast::Statement;

use crate::gelatin::Error;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ident(Arc<str>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Name {
    Ident(Ident),
    Dotted { parent: Box<Name>, attrs: Vec<Name> },
}

impl<S> From<S> for Name
where
    S: Into<Arc<str>>,
{
    fn from(value: S) -> Self {
        let value: Arc<str> = value.into();
        let mut names = value.split('.');
        let parent = Self::Ident(Ident::from(names.next().expect("expected a name")));

        let mut attrs = Vec::new();
        for name in names {
            attrs.push(name.into());
        }

        if attrs.is_empty() {
            parent
        } else {
            Self::Dotted {
                parent: Box::new(parent),
                attrs,
            }
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<Arc<str>> for Expr {
    fn from(value: Arc<str>) -> Self {
        Self::Value(Value::Str(value))
    }
}

impl<V> From<V> for Expr
where
    V: Into<Value>,
{
    fn from(value: V) -> Self {
        Self::Value(value.into())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    // null
    Nothing,
    // doesn't exist, it's a placeholder for ()
    Unit,
    Bool(bool),
    Int(i64),
    Str(Arc<str>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Datasource {
    Niku,
    Dwh,
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum HttpVerb {
    POST,
    GET,
    PATCH,
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum QueryType {
    SELECT,
    UPDATE,
    INSERT,
    DELETE,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Call {
    pub name: Name,
    pub args: Vec<Expr>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfixOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Infix {
        lhs: Box<Expr>,
        op: InfixOp,
        rhs: Box<Expr>,
    },
    Value(Value),
    Call(Call),
    Func {
        params: Vec<Ident>,
        body: Vec<Stmt>,
    },
    StaticField(Name),
    Static(Call),
    Range {
        start: i64,
        end: i64,
        step: i64,
    },
    Ident(Name),
    Alias(Ident),
    Dict(HashMap<Arc<str>, Expr>),
    Query {
        datasource: Datasource,
        r#type: QueryType,
        query: Statement,
        params: Vec<Expr>,
    },
    Http {
        verb: HttpVerb,
        url: Box<Expr>,
        body: Vec<Stmt>,
    },
    Json {
        expr: Ident,
    },
    Instance {
        class: Name,
        args: Vec<Expr>,
    },
    Soap {
        endpoint: Arc<str>,
        header: Option<Vec<xml::reader::XmlEvent>>,
        body: Option<Vec<xml::reader::XmlEvent>>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Stmt {
    Block(Vec<Stmt>),
    Catch {
        name: Ident,
        body: Vec<Stmt>,
    },
    Let(Ident, Expr),
    Alias {
        alias: Ident,
        cls: Expr,
    },
    Expr {
        expr: Expr,
    },
    ForEach {
        var: Ident,
        items: Expr,
        body: Vec<Stmt>,
    },
    While {
        test: Expr,
        body: Vec<Stmt>,
    },
    If {
        test: Expr,
        body: Vec<Stmt>,
        alt: Option<Vec<Stmt>>,
    },
    Log {
        level: LogLevel,
        message: Arc<str>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Node {
    Expr(Expr),
    Stmt(Stmt),
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ident(ident) => write!(f, "{ident}"),
            Self::Dotted {
                parent: box parent,
                attrs,
            } => {
                write!(f, "{parent}")?;

                for attr in attrs {
                    write!(f, ".{attr}")?;
                }

                Ok(())
            }
        }
    }
}

impl Ident {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> From<S> for Ident
where
    S: Into<Arc<str>>,
{
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

impl TryFrom<&str> for Datasource {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "niku" => Ok(Self::Niku),
            "datawarehouse" => Ok(Self::Dwh),
            val => Err(Error::Value {
                message: format!("expected \"niku\" or \"datawarehouse\", got: {val}"),
            }),
        }
    }
}

impl TryFrom<&str> for HttpVerb {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "GET" => Ok(Self::GET),
            "POST" => Ok(Self::POST),
            "PATCH" => Ok(Self::PATCH),
            val => Err(Error::Value {
                message: format!("not an http verb: {val}"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Context {
    // Normal context.
    Text,
    // We're inside a ${} literal.
    Expr,
}

impl Name {
    /// Returns a string representation of the `Value` as a java value.
    pub fn as_value(&self, ctx: Context) -> Cow<'_, str> {
        if matches!(ctx, Context::Text) {
            // use an expr to get the value out of the ident
            return Cow::Owned(format!("${{{self}}}"));
        }
        Cow::Owned(self.to_string())
    }
}

impl std::fmt::Display for InfixOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Eq => write!(f, "=="),
            Self::Neq => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Gt => write!(f, ">"),
            Self::Lte => write!(f, "<="),
            Self::Gte => write!(f, ">="),
        }
    }
}

impl Expr {
    pub fn infix(lhs: Self, op: InfixOp, rhs: Self) -> Self {
        Self::Infix {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        }
    }

    pub fn get_static<N: Into<Name>>(name: N) -> Self {
        Self::StaticField(name.into())
    }

    pub fn static_invoke<N: Into<Name>>(name: N, args: Vec<Self>) -> Self {
        Self::Static(Call {
            name: name.into(),
            args,
        })
    }

    pub fn call<N: Into<Name>>(name: N, args: Vec<Self>) -> Self {
        Self::Call(Call {
            name: name.into(),
            args,
        })
    }

    /// Returns a string representation of the `Value` as a java value.
    pub fn as_value(&self, ctx: Context) -> Cow<'_, str> {
        match self {
            Self::Value(v) => v.as_value(ctx),
            Self::Ident(v) => v.as_value(ctx),
            Self::Call(Call { name: func, args }) => {
                let mut buff = String::new();
                let should_expr = matches!(ctx, Context::Text);

                let ctx = if should_expr { Context::Expr } else { ctx };

                let _ = write!(
                    buff,
                    "{}{}(",
                    if should_expr { "${" } else { "" },
                    func.as_value(ctx)
                );

                buff.extend(
                    args.iter()
                        .map(|arg| arg.as_value(ctx))
                        .intersperse(", ".into()),
                );
                let _ = write!(buff, "){}", if should_expr { "}" } else { "" },);

                buff.into()
            }
            Self::Dict(map) => {
                let mut buff = String::new();
                let _ = write!(buff, "{{");

                buff.extend(
                    map.iter()
                        .map(|(k, v)| format!("{k}: {}", v.as_value(ctx)))
                        .intersperse(", ".to_string()),
                );

                let _ = write!(buff, "}}");

                buff.into()
            }
            Self::Infix { lhs, op, rhs } => {
                if matches!(ctx, Context::Text) {
                    return Cow::Owned(format!(
                        "${{({} {op} {})}}",
                        lhs.as_value(Context::Expr),
                        rhs.as_value(Context::Expr)
                    ));
                }
                Cow::Owned(format!(
                    "({} {op} {})",
                    lhs.as_value(ctx),
                    rhs.as_value(ctx)
                ))
            }
            Self::StaticField(name) => name.as_value(ctx),
            Self::Query { .. }
            | Self::Http { .. }
            | Self::Json { .. }
            | Self::Instance { .. }
            | Self::Range { .. }
            | Self::Alias(_)
            | Self::Static(_)
            | Self::Func { .. }
            | Self::Soap { .. } => unreachable!("{self:?}"),
        }
    }
}

impl Value {
    /// Returns a string representation of the `Value` as a java value.
    pub fn as_value(&self, ctx: Context) -> Cow<'_, str> {
        match self {
            Self::Nothing => Cow::Borrowed("null"),
            Self::Bool(b) => Cow::Owned(format!("{b}")),
            Self::Int(n) => {
                if matches!(ctx, Context::Text) {
                    Cow::Owned(format!("${{n}}"))
                } else {
                    Cow::Owned(format!("{n}"))
                }
            },
            Self::Str(str) => {
                if matches!(ctx, Context::Expr) {
                    Cow::Owned(format!("\"{str}\""))
                } else {
                    Cow::Borrowed(str)
                }
            }
            Self::Unit => unreachable!("unit value should not be used"),
        }
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for Datasource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Niku => write!(f, "niku"),
            Self::Dwh => write!(f, "datawarehouse"),
        }
    }
}

impl HttpVerb {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::POST => "POST",
            Self::GET => "GET",
            Self::PATCH => "PATCH",
        }
    }
}

impl std::fmt::Display for HttpVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}
