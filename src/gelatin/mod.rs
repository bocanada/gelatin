pub mod ast;
mod env;

use std::fmt::Write;
use std::{collections::HashMap, fmt::Debug};

use crate::errors::Error;
use ast::{Datasource, Expr, HttpVerb, Ident, Name, Node, QueryType, Stmt, Value};
use env::Env;
use lazy_static::lazy_static;
use miette::{NamedSource, SourceOffset, SourceSpan};
use pest::iterators::Pairs;
use pest::pratt_parser::{Op, PrattParser};
use pest::Parser;
use pest_derive::Parser;
use sqlparser::ast::Statement;
use sqlparser::{ast::Query, dialect::GenericDialect, parser::ParserError};

use self::ast::{InfixOp, LogLevel};

#[derive(Parser)]
#[grammar = "gelatin/gel-lang.pest"]
struct Gelatin;

pub struct GelatinParser<'a> {
    env: Env<Expr>,
    file_name: String,
    source: &'a str,
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

impl<'a> GelatinParser<'a> {
    pub fn new(file_name: &str, source: &'a str) -> Self {
        Self {
            env: Env::new(),
            file_name: file_name.to_string(),
            source,
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
                        .map(|r| format!("{:?}", r))
                        .collect::<Vec<String>>()
                        .join(", ");

                    Error::Syntax {
                        source_code: NamedSource::new(&self.file_name, self.source.to_string()),
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
                Rule::stmt => ast.push(Node::Stmt(
                    self.stmt_from(pair.into_inner().next().unwrap())?,
                )),
                Rule::expression => ast.push(Node::Expr(self.expr_from(pair.into_inner())?)),
                Rule::EOI => break,
                rule => unreachable!("got rule {rule:?}"),
            }
        }

        Ok(ast)
    }

    // fn expr_from(&mut self, pair: pest::iterators::Pair<Rule>) -> miette::Result<Expr> {
    //     match pair.as_rule() {
    //         Rule::unit => Ok(Expr::Value(Value::Unit)),
    //         Rule::bool => Ok(Expr::Value(Value::Bool(pair.as_str() == "true"))),
    //         Rule::nothing => Ok(Expr::Value(Value::Nothing)),
    //         Rule::number => {
    //             println!("'{}'", pair.as_str());
    //             Ok(Expr::Value(Value::Int(
    //                 pair.as_str().trim().parse().unwrap(),
    //             )))
    //         }
    //         Rule::normal_string => Ok(Expr::Value(Value::Str(
    //             pair.into_inner().as_str().to_string(),
    //         ))),
    //         Rule::access_ident => {
    //             if self.env.resolve(pair.as_str()).is_none() {
    //                 let (line, col) = pair.line_col();
    //                 return Err(Error::UnboundName {
    //                     source_code: pair.get_input().to_string(),
    //                     at: SourceSpan::new(
    //                         SourceOffset::from_location(pair.get_input(), line, col),
    //                         pair.as_str().len(),
    //                     ),
    //                 }
    //                 .into());
    //             }
    //
    //             Ok(Expr::Ident(Name::Ident(Ident::from(pair.as_str()))))
    //         }
    //         Rule::ident => Ok(Expr::Ident(Name::Ident(Ident::from(pair.as_str())))),
    //         Rule::dotted_access => {
    //             let mut dpair = pair.clone().into_inner();
    //             let parentp = dpair.next().unwrap();
    //             let Expr::Ident(parent) = self.expr_from(parentp.clone())? else {
    //                 unreachable!()
    //             };
    //
    //             // If it's an ident, we can resolve it as it should be defined.
    //             if let Name::Ident(ref parent) = parent {
    //                 if self.env.resolve(parent.as_str()).is_none() {
    //                     let (line, col) = parentp.line_col();
    //                     return Err(Error::UnboundName {
    //                         source_code: pair.get_input().to_string(),
    //                         at: SourceSpan::new(
    //                             SourceOffset::from_location(pair.get_input(), line, col),
    //                             parentp.as_str().len(),
    //                         ),
    //                     }
    //                     .into());
    //                 }
    //             }
    //
    //             let mut attrs = Vec::with_capacity(dpair.len());
    //
    //             for attr in dpair {
    //                 let Expr::Ident(attr @ Name::Ident(_)) = self.expr_from(attr)? else {
    //                     unreachable!()
    //                 };
    //                 attrs.push(attr);
    //             }
    //
    //             Ok(Expr::Ident(Name::Dotted {
    //                 parent: Box::new(parent),
    //                 attrs,
    //             }))
    //         }
    //         Rule::java_class => {
    //             let mut dpair = pair.clone().into_inner();
    //             let parent = Ident::from(dpair.next().unwrap().as_str());
    //
    //             let mut attrs = Vec::with_capacity(dpair.len());
    //
    //             for attr in dpair {
    //                 let attr = Name::Ident(Ident::from(attr.as_str()));
    //                 attrs.push(attr);
    //             }
    //
    //             Ok(Expr::Ident(Name::Dotted {
    //                 parent: Box::new(Name::Ident(parent)),
    //                 attrs,
    //             }))
    //         }
    //         Rule::http => {
    //             let mut qpair = pair.clone().into_inner();
    //             let verb: HttpVerb = qpair.next().unwrap().as_str().try_into()?;
    //
    //             let url = self.expr_from(qpair.next().unwrap())?;
    //
    //             let body = qpair
    //                 .map(|expr| self.stmt_from(expr))
    //                 .collect::<miette::Result<Vec<Stmt>>>()?;
    //
    //             Ok(Expr::Http {
    //                 verb,
    //                 url: Box::new(url),
    //                 body,
    //             })
    //         }
    //         Rule::fmt_string => {
    //             let pair = pair.into_inner();
    //
    //             let mut buff = String::new();
    //
    //             for arg in pair {
    //                 match arg.as_rule() {
    //                     Rule::fmt => {
    //                         let fmt = self.expr_from(arg.into_inner().next().unwrap())?;
    //                         println!("FMT: {}", fmt);
    //                         let _ = write!(buff, "{}", fmt.as_value(ast::Context::Text));
    //                     }
    //                     Rule::character => {
    //                         println!("ARG: {}", arg.as_str());
    //                         let _ = buff.write_str(arg.as_str());
    //                     }
    //                     _ => unreachable!(),
    //                 }
    //             }
    //
    //             Ok(Expr::Value(Value::Str(buff)))
    //         }
    //         Rule::range => {
    //             let mut pair = pair.into_inner();
    //             let start = pair.next().unwrap();
    //             let end = pair.next().unwrap();
    //
    //             Ok(Expr::Range {
    //                 start: start.as_str().parse().unwrap(),
    //                 end: end.as_str().parse().unwrap(),
    //                 step: 1,
    //             })
    //         }
    //         Rule::alias_ident => {
    //             let alias = pair.as_str();
    //             println!("GOT ALIAS IDENT {alias}");
    //             if let Some(expr) = self.env.resolve(alias) {
    //                 return Ok(expr.clone());
    //             }
    //
    //             Err(Error::UnboundAlias {
    //                 source_code: pair.get_input().to_string(),
    //                 at: SourceSpan::new(
    //                     SourceOffset::from_location(
    //                         pair.get_input(),
    //                         pair.line_col().0,
    //                         pair.line_col().1,
    //                     ),
    //                     alias.len(),
    //                 ),
    //             }
    //             .into())
    //         }
    //         Rule::json => {
    //             let mut pair = pair.into_inner();
    //             if let Expr::Ident(Name::Ident(conn_obj)) = self.expr_from(pair.next().unwrap())? {
    //                 Ok(Expr::Json { expr: conn_obj })
    //             } else {
    //                 unreachable!()
    //             }
    //         }
    //
    //         Rule::dict => {
    //             let pair = pair.into_inner();
    //
    //             let mut dict = HashMap::new();
    //
    //             for kv in pair {
    //                 let mut kv = kv.into_inner();
    //
    //                 let key = kv.next().unwrap().into_inner();
    //
    //                 let value = self.expr_from(kv.next().unwrap())?;
    //                 dict.insert(key.as_str().to_string(), value);
    //             }
    //
    //             Ok(Expr::Dict(dict))
    //         }
    //         Rule::query => {
    //             let mut qpair = pair.clone().into_inner();
    //             let ds: Datasource = qpair.next().unwrap().as_str().try_into()?;
    //
    //             let body_pair = qpair.next().unwrap();
    //
    //             let body = body_pair.clone().into_inner().as_str();
    //             let dialect = GenericDialect {}; // or AnsiDialect, or your own dialect ...
    //
    //             let parser = sqlparser::parser::Parser::new(&dialect);
    //
    //             let (stmt, query_type) = try_parse_query(parser, body, pair, body_pair)?;
    //             Ok(Expr::Query {
    //                 datasource: ds,
    //                 r#type: query_type,
    //                 query: stmt,
    //             })
    //         }
    //         Rule::new_class => {
    //             let pair = pair.into_inner();
    //             let (callable, args) = self.parse_callable(pair)?;
    //
    //             Ok(Expr::Instance {
    //                 class: callable,
    //                 args,
    //             })
    //         }
    //         Rule::r#static => {
    //             let pair = pair.into_inner();
    //             let (callable, args) = self.parse_callable(pair)?;
    //
    //             Ok(Expr::static_invoke(callable, args))
    //         }
    //         Rule::call => {
    //             let pair = pair.into_inner();
    //             let (callable, args) = self.parse_callable(pair)?;
    //
    //             Ok(Expr::call(callable, args))
    //         }
    //         Rule::expression | Rule::expr | Rule::value => {
    //             self.expr_from(pair.into_inner().next().unwrap())
    //         }
    //         rule => unreachable!("not an expr rule: {:?}", rule),
    //     }
    // }

    fn expr_from(&mut self, pair: Pairs<Rule>) -> miette::Result<Expr> {
        PRATT
            .map_primary(|pair| {
                match pair.as_rule() {
                    Rule::unit => Ok(Expr::Value(Value::Unit)),
                    Rule::bool => Ok(Expr::Value(Value::Bool(pair.as_str() == "true"))),
                    Rule::nothing => Ok(Expr::Value(Value::Nothing)),
                    Rule::number => Ok(Expr::Value(Value::Int(
                        pair.as_str().trim().parse().unwrap(),
                    ))),
                    Rule::normal_string => Ok(Expr::Value(Value::Str(
                        pair.into_inner().as_str().to_string(),
                    ))),
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
                                    let _ = write!(buff, "{}", fmt.as_value(ast::Context::Text));
                                }
                                Rule::character => {
                                    let _ = buff.write_str(arg.as_str());
                                }
                                _ => unreachable!(),
                            }
                        }

                        Ok(Expr::Value(Value::Str(buff)))
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

                            let key = kv.next().unwrap().into_inner();

                            let value = self.expr_from(kv.next().unwrap().into_inner())?;
                            dict.insert(key.as_str().to_string(), value);
                        }

                        Ok(Expr::Dict(dict))
                    }
                    Rule::query => {
                        let mut qpair = pair.clone().into_inner();
                        let ds: Datasource = qpair.next().unwrap().as_str().try_into()?;

                        let body_pair = qpair.next().unwrap();

                        let body = body_pair.clone().into_inner().as_str();
                        let dialect = GenericDialect {}; // or AnsiDialect, or your own dialect ...

                        let parser = sqlparser::parser::Parser::new(&dialect);

                        let (stmt, query_type) =
                            self.try_parse_query(parser, body, pair, body_pair)?;
                        Ok(Expr::Query {
                            datasource: ds,
                            r#type: query_type,
                            query: stmt,
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

                                args.push(arg)
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
                    _ => todo!(),
                };
                let lhs = lhs?;
                let rhs = rhs?;

                Ok(Expr::Infix {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                })
            })
            .parse(pair)
    }

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

                Ok(Stmt::Catch {
                    name,
                    body,
                    no_err: vec![],
                })
            }
            Rule::body => {
                let mut stmts = Vec::new();
                for pair in pair.into_inner() {
                    let stmt = match self.stmt_from(pair)? {
                        Stmt::Catch {
                            name,
                            no_err: _,
                            body,
                        } => {
                            return Ok(Stmt::Catch {
                                name,
                                body,
                                no_err: stmts,
                            })
                        }
                        stmt => stmt,
                    };
                    stmts.push(stmt);
                }

                Ok(Stmt::Block(stmts))
            }
            Rule::r#let => self.stmt_from(pair.into_inner().next().unwrap()),
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

                self.env.bind(alias.to_string().clone(), cls.clone());

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
            Rule::stmt => self.stmt_from(pair.into_inner().next().unwrap()),
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

                args.push(arg)
            }
        }

        Ok((callable, args))
    }

    fn try_parse_query(
        &mut self,
        parser: sqlparser::parser::Parser,
        sql: &str,
        pair: pest::iterators::Pair<Rule>,
        body_pair: pest::iterators::Pair<Rule>,
    ) -> miette::Result<(Statement, QueryType)> {
        let mut parser = match parser.try_with_sql(sql) {
            Ok(p) => p,
            Err(e) => match e {
                ParserError::TokenizerError(_) => todo!(),
                ParserError::RecursionLimitExceeded => todo!(),
                _ => unreachable!(),
            },
        };

        let stmts: miette::Result<_> = parser.parse_statements().map_err(|e| {
            let tok = parser.next_token();

            let (line, _) = pair.line_col();

            Error::SqlSyntax {
                source_code: NamedSource::new(&self.file_name, self.source.to_string()),
                at: SourceSpan::new(
                    SourceOffset::from_location(
                        self.source,
                        tok.location.line as usize + line - 1,
                        tok.location.column as usize,
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

        let ty = match stmt {
            sqlparser::ast::Statement::Query(ref query) => query_type(query),
            sqlparser::ast::Statement::Insert { .. } => QueryType::INSERT,
            sqlparser::ast::Statement::Update { .. } => QueryType::UPDATE,
            sqlparser::ast::Statement::Delete { .. } => QueryType::DELETE,
            _ => {
                let input = pair.get_input();
                return Err(Error::Syntax {
                    source_code: NamedSource::new(&self.file_name, input.to_string()),
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

        Ok((stmt, ty))
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
