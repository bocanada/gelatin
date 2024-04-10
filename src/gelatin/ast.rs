use std::{collections::HashMap, fmt::Write};

use sqlparser::ast::Statement;

use crate::gelatin::Error;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ident(String);

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Name {
    Ident(Ident),
    Dotted { parent: Box<Name>, attrs: Vec<Name> },
}

impl<S> From<S> for Name
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        let value: String = value.into();
        let mut names = value.split('.');
        let parent = Self::Ident(Ident::from(names.next().unwrap()));

        let mut attrs = Vec::new();
        for name in names {
            attrs.push(name.into())
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

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
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
    Str(String),
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

#[derive(Debug, PartialEq, Eq, Clone)]
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
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
    Dict(HashMap<String, Expr>),
    Query {
        datasource: Datasource,
        r#type: QueryType,
        query: Statement,
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
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Stmt {
    Block(Vec<Stmt>),
    Catch {
        name: Ident,
        no_err: Vec<Stmt>,
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
    If {
        test: Expr,
        body: Vec<Stmt>,
        alt: Option<Vec<Stmt>>,
    },
    Log {
        level: LogLevel,
        message: String,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Node {
    Expr(Expr),
    Stmt(Stmt),
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Name::Ident(ident) => write!(f, "{ident}"),
            Name::Dotted {
                parent: box parent,
                attrs,
            } => {
                write!(f, "{}", parent)?;

                for attr in attrs {
                    write!(f, ".{}", attr)?;
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
    S: Into<String>,
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
            val => Err(Error::ValueError {
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
            val => Err(Error::ValueError {
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
    pub fn as_value(&self, ctx: Context) -> String {
        if matches!(ctx, Context::Text) {
            // use an expr to get the value out of the ident
            return format!("${{{self}}}");
        }
        self.to_string()
    }
}

impl std::fmt::Display for InfixOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfixOp::Add => write!(f, "+"),
            InfixOp::Sub => write!(f, "-"),
            InfixOp::Mul => write!(f, "*"),
            InfixOp::Div => write!(f, "/"),
            InfixOp::Eq => write!(f, "=="),
            InfixOp::Neq => write!(f, "!="),
            InfixOp::Lt => write!(f, "<"),
            InfixOp::Gt => write!(f, ">"),
        }
    }
}
impl Expr {
    pub fn get_static<N: Into<Name>>(name: N) -> Self {
        Self::StaticField(name.into())
    }

    pub fn static_invoke<N: Into<Name>>(name: N, args: Vec<Expr>) -> Self {
        Self::Static(Call {
            name: name.into(),
            args,
        })
    }

    pub fn call<N: Into<Name>>(name: N, args: Vec<Expr>) -> Self {
        Self::Call(Call {
            name: name.into(),
            args,
        })
    }

    /// Returns a string representation of the `Value` as a java value.
    pub fn as_value(&self, ctx: Context) -> String {
        match self {
            Expr::Value(v) => v.as_value(ctx),
            Expr::Ident(v) => {
                if matches!(ctx, Context::Text) {
                    // use an expr to get the value out of the ident
                    return format!("${{{v}}}");
                }
                v.to_string()
            }
            Expr::Call(Call { name: func, args }) => {
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
                        .intersperse(", ".to_string()),
                );
                let _ = write!(buff, "){}", if should_expr { "}" } else { "" },);

                buff
            }
            Expr::Static(_) => todo!(),
            Expr::Dict(map) => {
                let mut buff = String::new();
                let _ = write!(buff, "{{");

                buff.extend(
                    map.iter()
                        .map(|(k, v)| format!("{k}: {}", v.as_value(ctx)))
                        .intersperse(", ".to_string()),
                );

                let _ = write!(buff, "}}");

                buff
            }
            Expr::Query { .. }
            | Expr::Http { .. }
            | Expr::Json { .. }
            | Expr::Instance { .. }
            | Expr::Range { .. }
            | Expr::Alias(_)
            | Expr::Func { .. } => unreachable!("{self:?}"),
            Expr::Infix { lhs, op, rhs } => {
                if matches!(ctx, Context::Text) {
                    return format!(
                        "${{({} {op} {})}}",
                        lhs.as_value(Context::Expr),
                        rhs.as_value(Context::Expr)
                    );
                }
                format!("({} {op} {})", lhs.as_value(ctx), rhs.as_value(ctx))
            }
            Expr::StaticField(name) => {
                if matches!(ctx, Context::Text) {
                    return format!("${{({})}}", name.as_value(Context::Expr));
                }
                name.as_value(ctx)
            }
        }
    }
}

impl Value {
    /// Returns a string representation of the `Value` as a java value.
    pub fn as_value(&self, ctx: Context) -> String {
        match self {
            Self::Nothing => "null".to_string(),
            Self::Bool(b) => format!("{b}"),
            Self::Int(n) => format!("{n}"),
            Self::Str(str) => {
                if matches!(ctx, Context::Expr) {
                    format!("\"{str}\"")
                } else {
                    str.to_owned()
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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Str(str) => write!(f, "{str}"),
            val => write!(f, "{}", val.as_value(Context::Text)),
        }
    }
}

impl std::fmt::Display for Datasource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Datasource::Niku => write!(f, "niku"),
            Datasource::Dwh => write!(f, "datawarehouse"),
        }
    }
}

impl std::fmt::Display for HttpVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpVerb::POST => write!(f, "POST"),
            HttpVerb::GET => write!(f, "GET"),
            HttpVerb::PATCH => write!(f, "PATCH"),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Value(v) => write!(f, "{v}"),
            Expr::Ident(v) => write!(f, "{v}"),
            Expr::Alias(v) => write!(f, "{v}"),
            Expr::Call(..) => {
                write!(f, "${{{}}}", self.as_value(Context::Expr))
            }
            _ => todo!(),
        }
    }
}
