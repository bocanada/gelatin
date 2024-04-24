pub mod ast;
mod env;

use std::fmt::Write;
use std::sync::Arc;
use std::{collections::HashMap, fmt::Debug};

use crate::errors::Error;
use ast::{Datasource, Expr, HttpVerb, Ident, Name, Node, QueryType, Stmt, Value};
use env::Env;
use lazy_static::lazy_static;
use miette::{NamedSource, SourceOffset, SourceSpan};
use pest::iterators::Pairs;
use pest::pratt_parser::{Op, PrattParser};
use pest::Parser as PestParser;
use pest_derive::Parser as PestParser;
use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::{ast::Query, parser::ParserError};
use xml::common::Position;

use self::ast::{Call, InfixOp, LogLevel};

#[derive(PestParser)]
#[grammar = "gelatin/gel-lang.pest"]
struct Gelatin;

pub struct Parser<'a, D: Dialect> {
    env: Env<Expr>,
    file_name: &'a str,
    source: &'a str,
    #[allow(clippy::struct_field_names)]
    sql_parser: D,
}

lazy_static! {
    static ref PRATT: PrattParser<Rule> = PrattParser::new()
        .op(Op::infix(Rule::eq, pest::pratt_parser::Assoc::Right)
            | Op::infix(Rule::neq, pest::pratt_parser::Assoc::Right)
            | Op::infix(Rule::gt, pest::pratt_parser::Assoc::Right)
            | Op::infix(Rule::gte, pest::pratt_parser::Assoc::Right)
            | Op::infix(Rule::lt, pest::pratt_parser::Assoc::Right)
            | Op::infix(Rule::lte, pest::pratt_parser::Assoc::Right))
        .op(Op::infix(Rule::plus, pest::pratt_parser::Assoc::Left)
            | Op::infix(Rule::sub, pest::pratt_parser::Assoc::Left))
        .op(Op::infix(Rule::mul, pest::pratt_parser::Assoc::Left)
            | Op::infix(Rule::div, pest::pratt_parser::Assoc::Left));
}

impl<'a, D: Dialect> Parser<'a, D> {
    pub fn new_with_dialect(file_name: &'a str, source: &'a str, dialect: D) -> Self {
        Self {
            env: Env::new(),
            file_name,
            source,
            sql_parser: dialect,
        }
    }

    pub fn parse(&mut self) -> miette::Result<Vec<Node>> {
        let pairs: miette::Result<_> = Gelatin::parse(Rule::program, self.source).map_err(|e| {
            let line_col = match e.line_col {
                pest::error::LineColLocation::Pos((line, col)) => {
                    SourceOffset::from_location(self.source, line, col)
                }
                pest::error::LineColLocation::Span((l1, c1), (_l2, _c2)) => {
                    SourceOffset::from_location(self.source, l1, c1)
                }
            };

            match e.variant {
                pest::error::ErrorVariant::ParsingError {
                    positives,
                    negatives: _,
                } => {
                    let expected = positives
                        .iter()
                        .map(|r| format!("{r:?}"))
                        .collect::<Vec<String>>()
                        .join(", ");

                    Error::Syntax {
                        source_code: NamedSource::new(self.file_name, self.source.to_string()),
                        at: SourceSpan::new(line_col, 1),
                        expected: Some(format!("expected {expected}")),
                    }
                    .into()
                }
                pest::error::ErrorVariant::CustomError { message: _ } => todo!(),
            }
        });
        let pairs = pairs?;

        let mut ast = vec![];

        for pair in pairs {
            match pair.as_rule() {
                Rule::stmt => {
                    let stmt = self.stmt_from(pair.into_inner().next().unwrap())?;
                    let stmt = Self::macro_expand_stmt(stmt);
                    ast.push(stmt);
                }
                Rule::expression => {
                    let expr = self.expr_from(pair.into_inner())?;
                    let expr = Self::macro_expand_expr("_", expr);
                    ast.push(expr);
                }
                Rule::EOI => break,
                rule => unreachable!("got rule {rule:?}"),
            }
        }

        Ok(ast)
    }
    #[allow(clippy::too_many_lines)]
    fn expr_from(&mut self, pair: Pairs<Rule>) -> miette::Result<Expr> {
        PRATT
            .map_primary(|pair| {
                match pair.as_rule() {
                    Rule::unit => Ok(Expr::Value(Value::Unit)),
                    Rule::bool => Ok(Expr::Value(Value::Bool(pair.as_str() == "true"))),
                    Rule::null => Ok(Expr::Value(Value::Nothing)),
                    Rule::number => Ok(Expr::Value(Value::Int(
                        pair.as_str().trim().parse().unwrap(),
                    ))),
                    Rule::normal_string => Ok(Expr::Value(pair.into_inner().as_str().into())),
                    Rule::access_ident => {
                        if self.env.resolve(pair.as_str()).is_none() {
                            let (line, col) = pair.line_col();
                            return Err(Error::UnboundName {
                                source_code: pair.get_input().to_string(),
                                at: SourceSpan::new(
                                    SourceOffset::from_location(pair.get_input(), line, col),
                                    pair.as_str().len(),
                                ),
                            }
                            .into());
                        }

                        Ok(Expr::Ident(Name::Ident(Ident::from(pair.as_str()))))
                    }
                    Rule::ident => Ok(Expr::Ident(Name::Ident(Ident::from(pair.as_str())))),
                    Rule::dotted_access => {
                        let mut dpair = pair.clone().into_inner();
                        let parentp = dpair.next().unwrap();
                        let Expr::Ident(parent) = self.expr_from(Pairs::single(parentp.clone()))?
                        else {
                            unreachable!()
                        };

                        // If it's an ident, we can resolve it as it should be defined.
                        if let Name::Ident(ref parent) = parent {
                            if self.env.resolve(parent.as_str()).is_none() {
                                let (line, col) = parentp.line_col();
                                return Err(Error::UnboundName {
                                    source_code: pair.get_input().to_string(),
                                    at: SourceSpan::new(
                                        SourceOffset::from_location(pair.get_input(), line, col),
                                        parentp.as_str().len(),
                                    ),
                                }
                                .into());
                            }
                        }

                        let mut attrs = Vec::with_capacity(dpair.len());

                        for attr in dpair {
                            let Expr::Ident(attr @ Name::Ident(_)) =
                                self.expr_from(Pairs::single(attr))?
                            else {
                                unreachable!()
                            };
                            attrs.push(attr);
                        }

                        Ok(Expr::Ident(Name::Dotted {
                            parent: Box::new(parent),
                            attrs,
                        }))
                    }
                    Rule::soap => {
                        let mut pair = pair.into_inner();
                        let Expr::Value(Value::Str(endpoint)) =
                            self.expr_from(Pairs::single(pair.next().unwrap()))?
                        else {
                            unreachable!()
                        };

                        let body = pair.next().expect("HEADER").into_inner();
                        let mut soap_header = None;
                        let mut soap_body = None;

                        for pair in body {
                            let (line, _) = pair.line_col();

                            match pair.as_rule() {
                                Rule::soap_message_header => {
                                    let mut buff = String::new();
                                    let pair = pair.into_inner().next().unwrap();

                                    for arg in pair.into_inner() {
                                        match arg.as_rule() {
                                            Rule::fmt => {
                                                let fmt = self.expr_from(arg.into_inner())?;
                                                let _ = write!(
                                                    buff,
                                                    "{}",
                                                    fmt.as_value(ast::Context::Text)
                                                );
                                            }
                                            Rule::xml_str => {
                                                let arg = arg.as_str();

                                                let _ = buff.write_str(arg);
                                            }
                                            _ => unreachable!("{:?}", arg.as_rule()),
                                        }
                                    }

                                    if buff.is_empty() {
                                        continue;
                                    }

                                    let _ = soap_header.insert(self.parse_xml(&buff, line)?);
                                }
                                Rule::soap_message_body => {
                                    let mut buff = String::new();
                                    let pair = pair.into_inner().next().unwrap();

                                    for arg in pair.into_inner() {
                                        match arg.as_rule() {
                                            Rule::fmt => {
                                                let fmt = self.expr_from(arg.into_inner())?;
                                                let _ = write!(
                                                    buff,
                                                    "{}",
                                                    fmt.as_value(ast::Context::Text)
                                                );
                                            }
                                            Rule::xml_str => {
                                                let arg = arg.as_str();

                                                let _ = buff.write_str(arg);
                                            }
                                            _ => unreachable!("{:?}", arg.as_rule()),
                                        }
                                    }

                                    if buff.is_empty() {
                                        continue;
                                    }

                                    let _ = soap_body.insert(self.parse_xml(&buff, line)?);
                                }
                                _ => unreachable!(),
                            }
                        }

                        Ok(Expr::Soap {
                            endpoint,
                            header: soap_header,
                            body: soap_body,
                        })
                    }
                    Rule::java_class => {
                        let mut dpair = pair.clone().into_inner();
                        let parent = Ident::from(dpair.next().unwrap().as_str());

                        let mut attrs = Vec::with_capacity(dpair.len());

                        for attr in dpair {
                            let attr = Name::Ident(Ident::from(attr.as_str()));
                            attrs.push(attr);
                        }

                        Ok(Expr::Ident(Name::Dotted {
                            parent: Box::new(Name::Ident(parent)),
                            attrs,
                        }))
                    }
                    Rule::http => {
                        let mut qpair = pair.clone().into_inner();
                        let verb: HttpVerb = qpair.next().unwrap().as_str().try_into()?;

                        let url = self.expr_from(Pairs::single(qpair.next().unwrap()))?;

                        let Stmt::Block(body) = self.stmt_from(qpair.next().unwrap())? else {
                            unreachable!()
                        };

                        Ok(Expr::Http {
                            verb,
                            url: Box::new(url),
                            body,
                        })
                    }
                    Rule::fmt_string => {
                        let pair = pair.into_inner();

                        let mut buff = String::new();

                        for arg in pair {
                            match arg.as_rule() {
                                Rule::fmt => {
                                    let fmt = self.expr_from(arg.into_inner())?;
                                    let _ =
                                        buff.write_str(fmt.as_value(ast::Context::Text).as_ref());
                                }
                                Rule::character => {
                                    let _ = buff.write_str(arg.as_str());
                                }
                                _ => unreachable!(),
                            }
                        }

                        Ok(Expr::Value(buff.as_str().into()))
                    }
                    Rule::range => {
                        let mut pair = pair.into_inner();
                        let start = pair.next().unwrap();
                        let end = pair.next().unwrap();

                        Ok(Expr::Range {
                            start: start.as_str().parse().unwrap(),
                            end: end.as_str().parse().unwrap(),
                            step: 1,
                        })
                    }
                    Rule::alias_ident => {
                        let alias = pair.as_str();
                        if let Some(expr) = self.env.resolve(alias) {
                            return Ok(expr.clone());
                        }

                        Err(Error::UnboundAlias {
                            source_code: pair.get_input().to_string(),
                            at: SourceSpan::new(
                                SourceOffset::from_location(
                                    pair.get_input(),
                                    pair.line_col().0,
                                    pair.line_col().1,
                                ),
                                alias.len(),
                            ),
                        }
                        .into())
                    }
                    Rule::json => {
                        let mut pair = pair.into_inner();
                        if let Expr::Ident(Name::Ident(conn_obj)) =
                            self.expr_from(Pairs::single(pair.next().unwrap()))?
                        {
                            Ok(Expr::Json { expr: conn_obj })
                        } else {
                            unreachable!()
                        }
                    }

                    Rule::dict => {
                        let pair = pair.into_inner();

                        let mut dict = HashMap::new();

                        for kv in pair {
                            let mut kv = kv.into_inner();

                            let Expr::Value(Value::Str(key)) =
                                self.expr_from(Pairs::single(kv.next().unwrap()))?
                            else {
                                unreachable!()
                            };

                            let value = self.expr_from(kv.next().unwrap().into_inner())?;
                            dict.insert(key, value);
                        }

                        Ok(Expr::Dict(dict))
                    }
                    Rule::query => {
                        let mut qpair = pair.clone().into_inner();
                        let datasource: Datasource = qpair.next().unwrap().as_str().try_into()?;

                        let body_pair = qpair.next().unwrap();

                        let body = body_pair.clone().into_inner().as_str();

                        let parser = sqlparser::parser::Parser::new(&self.sql_parser);

                        let ((query, param_len), query_type) =
                            self.try_parse_query(parser, body, &pair, &body_pair)?;

                        let params = match qpair.next() {
                            Some(qpair) => {
                                let (line, col) = qpair.line_col();

                                let params = qpair
                                    .into_inner()
                                    .map(|expr| self.expr_from(Pairs::single(expr)))
                                    .collect::<miette::Result<Vec<Expr>>>()?;

                                if param_len != params.len() {
                                    return Err(Error::SqlParamErr {
                                        source_code: NamedSource::new(
                                            self.file_name,
                                            self.source.to_string(),
                                        ),
                                        at: SourceSpan::new(
                                            SourceOffset::from_location(self.source, line, col),
                                            col,
                                        ),
                                        err: format!(
                                            "expected {param_len} parameters but got {}",
                                            params.len()
                                        ),
                                    }
                                    .into());
                                }

                                params
                            }
                            None => vec![],
                        };

                        Ok(Expr::Query {
                            datasource,
                            query,
                            params,
                            r#type: query_type,
                        })
                    }
                    Rule::new_class => {
                        let pair = pair.into_inner();
                        let (callable, args) = self.parse_callable(pair)?;

                        Ok(Expr::Instance {
                            class: callable,
                            args,
                        })
                    }
                    Rule::r#static => {
                        let mut pair = pair.into_inner();
                        let Expr::Ident(callable) =
                            self.expr_from(Pairs::single(pair.next().unwrap()))?
                        else {
                            unreachable!()
                        };

                        let mut args = Vec::new();

                        if let Some(args_pair) = pair.next() {
                            let args_pair = args_pair.into_inner();

                            for arg in args_pair.map(|arg| self.expr_from(Pairs::single(arg))) {
                                let arg = arg?;
                                if matches!(arg, Expr::Value(Value::Unit)) {
                                    continue;
                                }

                                args.push(arg);
                            }
                            Ok(Expr::static_invoke(callable, args))
                        } else {
                            Ok(Expr::get_static(callable))
                        }
                    }
                    Rule::call => {
                        let pair = pair.into_inner();
                        let (callable, args) = self.parse_callable(pair)?;

                        Ok(Expr::call(callable, args))
                    }
                    Rule::expression | Rule::expr | Rule::value => {
                        self.expr_from(pair.into_inner())
                    }
                    rule => unreachable!("not an expr rule: {:?}", rule),
                }
            })
            .map_prefix(|op, rhs| todo!("{op:?} {rhs:?}"))
            .map_infix(|lhs, op, rhs| {
                let op = match op.as_rule() {
                    Rule::plus => InfixOp::Add,
                    Rule::sub => InfixOp::Sub,
                    Rule::div => InfixOp::Div,
                    Rule::mul => InfixOp::Mul,
                    Rule::eq => InfixOp::Eq,
                    Rule::neq => InfixOp::Neq,
                    Rule::lt => InfixOp::Lt,
                    Rule::gt => InfixOp::Gt,
                    Rule::lte => InfixOp::Lte,
                    Rule::gte => InfixOp::Gte,
                    _ => todo!(),
                };
                let lhs = lhs?;
                let rhs = rhs?;

                Ok(Expr::infix(lhs, op, rhs))
            })
            .parse(pair)
    }

    fn parse_xml(&self, xml: &str, line: usize) -> miette::Result<Vec<xml::reader::XmlEvent>> {
        let reader = xml::reader::ParserConfig::new()
            .trim_whitespace(true)
            .create_reader(xml.as_bytes());

        reader
            .into_iter()
            .map(|event| {
                event.map_err(|err| {
                    let xml::common::TextPosition { row, column } = err.position();

                    Error::XmlSyntax {
                        source_code: NamedSource::new(self.file_name, self.source.to_string()),
                        at: SourceSpan::new(
                            SourceOffset::from_location(
                                self.source,
                                line + usize::try_from(row).expect("a valid usize"),
                                usize::try_from(column).expect("a valid usize"),
                            ),
                            1,
                        ),
                        err: err.msg().to_string(),
                    }
                    .into()
                })
            })
            .skip(1)
            .collect::<miette::Result<Vec<xml::reader::XmlEvent>>>()
    }

    #[allow(clippy::too_many_lines)]
    fn stmt_from(&mut self, pair: pest::iterators::Pair<Rule>) -> miette::Result<Stmt> {
        match pair.as_rule() {
            Rule::expression | Rule::expr => {
                let expr = self.expr_from(pair.into_inner())?;

                Ok(Stmt::Expr { expr })
            }
            Rule::catch => {
                let mut pair = pair.into_inner();
                let Expr::Ident(Name::Ident(name)) =
                    self.expr_from(Pairs::single(pair.next().unwrap()))?
                else {
                    unreachable!()
                };

                let scoped = self.env.scoped();
                let mut old = std::mem::replace(&mut self.env, scoped);
                self.env.bind(name.to_string(), Expr::Value(Value::Unit));

                let body = pair
                    .map(|stmt| self.stmt_from(stmt))
                    .collect::<miette::Result<_>>()?;

                std::mem::swap(&mut old, &mut self.env);

                Ok(Stmt::Catch { name, body })
            }
            Rule::body => {
                let mut stmts = Vec::new();
                for pair in pair.into_inner() {
                    let stmt = match self.stmt_from(pair)? {
                        Stmt::Catch { name, body } => {
                            let ident = Expr::Ident(name.as_str().into());

                            return Ok(Stmt::Block(vec![
                                Stmt::Catch { name, body: stmts },
                                Stmt::If {
                                    test: Expr::infix(ident, InfixOp::Neq, Value::Nothing.into()),
                                    body,
                                    alt: None,
                                },
                            ]));
                        }
                        stmt => stmt,
                    };
                    stmts.push(stmt);
                }

                Ok(Stmt::Block(stmts))
            }
            Rule::r#let | Rule::stmt => self.stmt_from(pair.into_inner().next().unwrap()),
            Rule::r#letfn => {
                let mut pair = pair.into_inner();
                let Expr::Ident(Name::Ident(name)) =
                    self.expr_from(Pairs::single(pair.next().unwrap()))?
                else {
                    unreachable!()
                };

                let params_pair = pair.next().expect("function params").into_inner();
                let mut params = Vec::with_capacity(params_pair.len());

                let old = self.env.clone();
                self.env = old.scoped();

                for arg in params_pair {
                    let Expr::Ident(Name::Ident(ident)) = self.expr_from(Pairs::single(arg))?
                    else {
                        unreachable!()
                    };
                    self.env.bind(ident.to_string(), Expr::Value(Value::Unit));

                    params.push(ident);
                }

                let body = pair
                    .next()
                    .expect("function body")
                    .into_inner()
                    .map(|stmt| self.stmt_from(stmt))
                    .collect::<miette::Result<Vec<Stmt>>>()?;

                self.env = old;

                Ok(Stmt::Let(name, Expr::Func { params, body }))
            }
            Rule::r#lete => {
                let mut pair = pair.into_inner();
                let ident = pair.next().unwrap();
                let ident = Ident::from(ident.as_str());

                let expr = self.expr_from(Pairs::single(pair.next().unwrap()))?;

                self.env.bind(ident.to_string(), expr.clone());

                Ok(Stmt::Let(ident, expr))
            }
            Rule::alias => {
                let mut pair = pair.into_inner();
                let next = pair.next().unwrap();
                let Expr::Ident(Name::Ident(alias)) = self.expr_from(Pairs::single(next))? else {
                    unreachable!()
                };

                let cls = self.expr_from(pair)?;

                let _ = self.env.bind(alias.to_string(), cls.clone());

                Ok(Stmt::Alias { alias, cls })
            }
            Rule::log => {
                let mut pair = pair.into_inner();

                let level = match pair.next().unwrap().as_str() {
                    "INFO" => LogLevel::Info,
                    "DEBUG" => LogLevel::Debug,
                    "WARN" => LogLevel::Warn,
                    "ERROR" => LogLevel::Error,
                    _ => unreachable!(),
                };
                let Expr::Value(Value::Str(message)) =
                    self.expr_from(Pairs::single(pair.next().unwrap()))?
                else {
                    unreachable!()
                };

                Ok(Stmt::Log { level, message })
            }
            Rule::r#if => {
                let mut pair = pair.into_inner();

                let expr = self.expr_from(Pairs::single(pair.next().unwrap()))?;
                let conseq = pair
                    .next()
                    .unwrap()
                    .into_inner()
                    .map(|stmt| self.stmt_from(stmt))
                    .collect::<miette::Result<_>>()?;

                let mut alt: Option<Vec<Stmt>> = None;
                if let Some(pair) = pair.next() {
                    alt = Some(
                        pair.into_inner()
                            .map(|stmt| self.stmt_from(stmt))
                            .collect::<miette::Result<Vec<_>>>()?,
                    );
                }

                Ok(Stmt::If {
                    test: expr,
                    body: conseq,
                    alt,
                })
            }
            Rule::r#for => {
                let mut pair = pair.into_inner();

                let Expr::Ident(Name::Ident(var)) =
                    self.expr_from(Pairs::single(pair.next().unwrap()))?
                else {
                    unreachable!()
                };
                let old = self.env.clone();
                self.env = old.scoped();
                self.env.bind(var.to_string(), Expr::Value(Value::Unit));

                let expr = self.expr_from(Pairs::single(pair.next().unwrap()))?;

                let mut body = Vec::with_capacity(pair.len());

                for stmt in pair {
                    let stmt = self.stmt_from(stmt)?;
                    body.push(stmt);
                }

                self.env = old;

                Ok(Stmt::ForEach {
                    var,
                    items: expr,
                    body,
                })
            }
            rule => unreachable!("not an stmt rule: {:?}", rule),
        }
    }

    fn parse_callable(
        &mut self,
        mut pair: pest::iterators::Pairs<Rule>,
    ) -> miette::Result<(Name, Vec<Expr>)> {
        let Expr::Ident(callable) = self.expr_from(Pairs::single(pair.next().unwrap()))? else {
            unreachable!()
        };

        let mut args = Vec::new();

        if let Some(args_pair) = pair.next() {
            let args_pair = args_pair.into_inner();

            for arg in args_pair.map(|arg| self.expr_from(Pairs::single(arg))) {
                let arg = arg?;
                if matches!(arg, Expr::Value(Value::Unit)) {
                    continue;
                }

                args.push(arg);
            }
        }

        Ok((callable, args))
    }

    #[allow(clippy::too_many_lines)]
    fn macro_expand_expr<I: Into<Ident>>(name: I, expr: Expr) -> Node {
        match expr {
            Expr::Http {
                verb,
                box url,
                body,
            } => {
                let name: Ident = name.into();

                let mut tags = vec![
                    Stmt::Let(
                        "remoteURL".into(),
                        Expr::Instance {
                            class: "java.net.URL".into(),
                            args: vec![url],
                        },
                    ),
                    Stmt::Let(name.clone(), Expr::call("remoteURL.openConnection", vec![])),
                    Stmt::Expr {
                        expr: Expr::call(
                            format!("{name}.setRequestMethod"),
                            vec![verb.as_str().into()],
                        ),
                    },
                    Stmt::Expr {
                        expr: Expr::call(format!("{name}.setDoOutput"), vec![true.into()]),
                    },
                ];

                if !matches!(verb, HttpVerb::GET) {
                    tags.push(Stmt::Expr {
                        expr: Expr::call(format!("{name}.setDoInput"), vec![true.into()]),
                    });
                }

                for expr in body {
                    let Stmt::Expr {
                        expr:
                            Expr::Call(Call {
                                name: Name::Ident(func),
                                mut args,
                            }),
                    } = expr
                    else {
                        unreachable!()
                    };

                    match func.as_str() {
                        "timeout" => {
                            let Some(timeout) = args.pop() else {
                                unreachable!()
                            };

                            tags.push(Stmt::Expr {
                                expr: Expr::call(
                                    format!("{name}.setConnectTimeout"),
                                    vec![timeout.clone()],
                                ),
                            });
                            tags.push(Stmt::Expr {
                                expr: Expr::call(
                                    format!("{name}.setReadTimeout"),
                                    vec![timeout.clone()],
                                ),
                            });
                        }
                        "headers" => {
                            let Some(Expr::Dict(dict)) = args.pop() else {
                                unreachable!("kek")
                            };

                            tags.extend(dict.into_iter().map(|(k, v)| Stmt::Expr {
                                expr: Expr::call(
                                    format!("{name}.setRequestProperty"),
                                    vec![k.into(), v],
                                ),
                            }));
                        }
                        "json" => {
                            let Some(Expr::Dict(dict)) = args.pop() else {
                                unreachable!("kek")
                            };
                            tags.push(Stmt::Expr {
                                expr: Expr::call(
                                    format!("{name}.setRequestProperty"),
                                    vec!["content-type".into(), "application/json".into()],
                                ),
                            });

                            tags.push(Stmt::Let(
                                format!("{name}_w").into(),
                                Expr::Instance {
                                    class: "java.io.OutputStreamWriter".into(),
                                    args: vec![Expr::call(
                                        format!("{name}.getOutputStream"),
                                        vec![],
                                    )],
                                },
                            ));

                            Self::create_json_tags(
                                dict,
                                format!("{name}_payload").as_str(),
                                &mut tags,
                            );

                            tags.push(Stmt::Expr {
                                expr: Expr::call(
                                    format!("{name}_payload.write"),
                                    vec![Expr::Ident(format!("{name}_w").into())],
                                ),
                            });
                            tags.push(Stmt::Expr {
                                expr: Expr::call(format!("{name}_w.flush"), vec![]),
                            });
                        }
                        _ => unreachable!(),
                    }
                }

                tags.push(Stmt::Expr {
                    expr: Expr::call(format!("{name}.connect"), vec![]),
                });

                Node::Stmt(Stmt::Block(tags))
            }
            Expr::Json { expr } => {
                let tags = vec![
                    Stmt::Let(
                        "input_stream".into(),
                        Expr::call(format!("{expr}.getInputStream"), vec![]),
                    ),
                    Stmt::Let(
                        "input_reader".into(),
                        Expr::Instance {
                            class: "java.io.InputStreamReader".into(),
                            args: vec![Expr::Ident("input_stream".into())],
                        },
                    ),
                    Stmt::Let(
                        "buf_reader".into(),
                        Expr::Instance {
                            class: "java.io.BufferedReader".into(),
                            args: vec![Expr::Ident("input_reader".into())],
                        },
                    ),
                    Stmt::Let(
                        "sb".into(),
                        Expr::Instance {
                            class: "java.lang.StringBuilder".into(),
                            args: vec![],
                        },
                    ),
                    Stmt::Let("line".into(), Expr::call("buf_reader.readLine", vec![])),
                    Stmt::While {
                        test: Expr::Infix {
                            lhs: Box::new(Expr::Ident("line".into())),
                            op: InfixOp::Neq,
                            rhs: Box::new(Expr::Value(Value::Nothing)),
                        },
                        body: vec![
                            Stmt::Expr {
                                expr: Expr::call("sb.append", vec![Expr::Ident("line".into())]),
                            },
                            Stmt::Let("line".into(), Expr::call("buf_reader.readLine", vec![])),
                        ],
                    },
                    Stmt::Let(
                        name.into(),
                        Expr::Instance {
                            class: "org.json.JSONObject".into(),
                            args: vec![Expr::call("sb.toString", vec![])],
                        },
                    ),
                ];

                Node::Stmt(Stmt::Block(tags))
            }
            Expr::Dict(dict) => {
                let name: Ident = name.into();
                let mut tags = vec![];
                Self::create_json_tags(dict, name.as_str(), &mut tags);

                Node::Stmt(Stmt::Block(tags))
            }
            expr => Node::Expr(expr),
        }
    }

    fn macro_expand_stmt(stmt: Stmt) -> Node {
        match stmt {
            Stmt::Let(name, expr @ (Expr::Dict(_) | Expr::Json { .. } | Expr::Http { .. })) => {
                Self::macro_expand_expr(name, expr)
            }
            stmt => Node::Stmt(stmt),
        }
    }

    fn create_json_tags(map: HashMap<Arc<str>, Expr>, bind_to: &str, tags: &mut Vec<Stmt>) {
        tags.push(Stmt::Let(
            bind_to.into(),
            Expr::Instance {
                class: "org.json.JSONObject".into(),
                args: vec![],
            },
        ));

        for (k, mut v) in map {
            if let Expr::Dict(inner) = v {
                Self::create_json_tags(inner, &format!("{bind_to}_{k}"), tags);
                v = Expr::Ident(k.clone().into());
            }

            tags.push(Stmt::Expr {
                expr: Expr::call(format!("{bind_to}_payload.put"), vec![k.into(), v]),
            });
        }
    }

    fn try_parse_query(
        &self,
        parser: sqlparser::parser::Parser,
        sql: &str,
        pair: &pest::iterators::Pair<Rule>,
        body_pair: &pest::iterators::Pair<Rule>,
    ) -> miette::Result<((Statement, usize), QueryType)> {
        let mut parser = match parser.try_with_sql(sql) {
            Ok(p) => p,
            Err(e) => match e {
                ParserError::TokenizerError(_) => todo!(),
                ParserError::RecursionLimitExceeded => todo!(),
                ParserError::ParserError(_) => unreachable!(),
            },
        };

        let stmts: miette::Result<_> = parser.parse_statements().map_err(|e| {
            let tok = parser.next_token();

            let (line, _) = pair.line_col();

            Error::SqlSyntax {
                source_code: NamedSource::new(self.file_name, self.source.to_string()),
                at: SourceSpan::new(
                    SourceOffset::from_location(
                        self.source,
                        usize::try_from(tok.location.line + (line as u64) - 1)
                            .expect("a valid usize"),
                        usize::try_from(tok.location.column).expect("a valid usize"),
                    ),
                    1,
                ),
                err: e.to_string().split_once(": ").unwrap().1.to_string(),
            }
            .into()
        });
        let mut stmts = stmts?;

        assert!(stmts.len() == 1, "expected only one statement in sql query");

        let stmt = stmts.pop().unwrap();

        let (ty, len) = match stmt {
            sqlparser::ast::Statement::Query(ref query) => {
                (query_type(query), query_params(query, 0))
            }
            sqlparser::ast::Statement::Insert { .. } => (QueryType::INSERT, 0),
            sqlparser::ast::Statement::Update { .. } => (QueryType::UPDATE, 0),
            sqlparser::ast::Statement::Delete { .. } => (QueryType::DELETE, 0),
            _ => {
                let input = pair.get_input();
                return Err(Error::Syntax {
                    source_code: NamedSource::new(self.file_name, input.to_string()),
                    at: SourceSpan::new(
                        SourceOffset::from_location(
                            body_pair.get_input(),
                            pair.line_col().0,
                            pair.line_col().1,
                        ),
                        sql.len() + 13, // move it to the }
                    ),
                    expected: Some("expected a select/insert/update/delete statement".to_string()),
                }
                .into());
            }
        };

        Ok(((stmt, len), ty))
    }
}

fn add_if_placeholder(expr: &sqlparser::ast::Expr, count: &mut usize) {
    match expr {
        sqlparser::ast::Expr::Value(sqlparser::ast::Value::Placeholder(_)) => {
            *count += 1;
        }
        sqlparser::ast::Expr::JsonAccess { left, right, .. }
        | sqlparser::ast::Expr::BinaryOp { left, right, .. }
        | sqlparser::ast::Expr::AnyOp { left, right, .. }
        | sqlparser::ast::Expr::AllOp { left, right, .. }
        | sqlparser::ast::Expr::IsDistinctFrom(left, right)
        | sqlparser::ast::Expr::IsNotDistinctFrom(left, right) => {
            add_if_placeholder(left, count);
            add_if_placeholder(right, count);
        }
        sqlparser::ast::Expr::IsFalse(expr)
        | sqlparser::ast::Expr::IsNotFalse(expr)
        | sqlparser::ast::Expr::IsTrue(expr)
        | sqlparser::ast::Expr::IsNotTrue(expr)
        | sqlparser::ast::Expr::IsNull(expr)
        | sqlparser::ast::Expr::IsNotNull(expr)
        | sqlparser::ast::Expr::IsUnknown(expr)
        | sqlparser::ast::Expr::IsNotUnknown(expr)
        | sqlparser::ast::Expr::UnaryOp { expr, .. } => add_if_placeholder(expr, count),
        sqlparser::ast::Expr::InList { expr, list, .. } => {
            add_if_placeholder(expr, count);
            list.iter()
                .map(|e| add_if_placeholder(e, count))
                .for_each(drop);
        }
        sqlparser::ast::Expr::InSubquery { expr, subquery, .. } => {
            add_if_placeholder(expr, count);

            *count = query_params(subquery, *count);
        }
        sqlparser::ast::Expr::InUnnest {
            expr, array_expr, ..
        } => {
            add_if_placeholder(expr, count);
            add_if_placeholder(array_expr, count);
        }
        sqlparser::ast::Expr::Between {
            expr, low, high, ..
        } => {
            add_if_placeholder(expr, count);
            add_if_placeholder(low, count);
            add_if_placeholder(high, count);
        }
        sqlparser::ast::Expr::Like { expr, pattern, .. }
        | sqlparser::ast::Expr::ILike { expr, pattern, .. }
        | sqlparser::ast::Expr::SimilarTo { expr, pattern, .. }
        | sqlparser::ast::Expr::RLike { expr, pattern, .. } => {
            add_if_placeholder(expr, count);
            add_if_placeholder(pattern, count);
        }
        sqlparser::ast::Expr::AggregateExpressionWithFilter { expr, filter } => {
            add_if_placeholder(expr, count);
            add_if_placeholder(filter, count);
        }
        sqlparser::ast::Expr::Case { .. } => todo!(),
        sqlparser::ast::Expr::Exists { subquery, .. } => {
            *count = query_params(subquery, *count);
        }
        sqlparser::ast::Expr::Subquery(qry) => {
            *count = query_params(qry, *count);
        }
        sqlparser::ast::Expr::OuterJoin(join) => add_if_placeholder(join, count),
        sqlparser::ast::Expr::Array(sqlparser::ast::Array { elem, .. }) => {
            for el in elem {
                add_if_placeholder(el, count);
            }
        }
        sqlparser::ast::Expr::Tuple(_) => todo!(),
        sqlparser::ast::Expr::ArrayIndex { .. } => todo!(),
        sqlparser::ast::Expr::Interval(_) => todo!(),
        sqlparser::ast::Expr::Convert { .. } => todo!(),
        sqlparser::ast::Expr::Cast { .. } => todo!(),
        sqlparser::ast::Expr::TryCast { .. } => todo!(),
        sqlparser::ast::Expr::SafeCast { .. } => todo!(),
        sqlparser::ast::Expr::AtTimeZone { .. } => todo!(),
        sqlparser::ast::Expr::Extract { .. } => todo!(),
        sqlparser::ast::Expr::Ceil { .. } => todo!(),
        sqlparser::ast::Expr::Floor { .. } => todo!(),
        sqlparser::ast::Expr::Position { .. } => todo!(),
        sqlparser::ast::Expr::Substring { .. } => todo!(),
        sqlparser::ast::Expr::Trim { .. } => todo!(),
        sqlparser::ast::Expr::Collate { .. } => todo!(),
        sqlparser::ast::Expr::Nested(_) => todo!(),
        sqlparser::ast::Expr::Function(_) => todo!(),

        _ => (),
    }
}

fn query_params(query: &Query, mut other: usize) -> usize {
    match query.body.as_ref() {
        sqlparser::ast::SetExpr::Select(ref select) => {
            if let Some(selection) = &select.selection {
                add_if_placeholder(selection, &mut other);
            }
            other
        }
        sqlparser::ast::SetExpr::Query(qry) => query_params(qry, other),
        sqlparser::ast::SetExpr::SetOperation { .. } => todo!(),
        sqlparser::ast::SetExpr::Values(_) => todo!(),
        sqlparser::ast::SetExpr::Insert(_) => todo!(),
        sqlparser::ast::SetExpr::Update(_) => todo!(),
        sqlparser::ast::SetExpr::Table(_) => todo!(),
    }
}

fn query_type(query: &Query) -> QueryType {
    match *query.body {
        sqlparser::ast::SetExpr::Select(_) => QueryType::SELECT,
        sqlparser::ast::SetExpr::Insert(_) => QueryType::INSERT,
        sqlparser::ast::SetExpr::Update(_) => QueryType::UPDATE,
        sqlparser::ast::SetExpr::Query(ref query) => query_type(query),
        _ => todo!("make this return a result"),
    }
}
