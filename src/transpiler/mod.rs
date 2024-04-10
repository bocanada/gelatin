pub mod tags;

use std::collections::HashMap;

use crate::gelatin::ast::{Call, Expr, HttpVerb, Ident, Name, Node, QueryType, Stmt, Value};

use self::tags::{Core, Gel, Sql, Tag};

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[allow(dead_code)]
enum Libraries {
    Core,
    Gel,
    Sql,
    Email,
    File,
    Ftp,
    Soap,
    SoapEnv,
    Xog,
}

pub struct Transpiler {
    env: HashMap<String, Expr>,
}

impl Transpiler {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
        }
    }

    pub fn as_tags(&mut self, node: Node) -> Tag {
        match node {
            Node::Stmt(stmt) => self.transpile_node(stmt),
            Node::Expr(expr) => self.transpile_node(Stmt::Expr { expr }),
        }
    }

    pub fn transpile_node(&mut self, stmt: Stmt) -> Tag {
        match stmt {
            Stmt::Expr {
                expr: qry @ Expr::Query { .. },
            } => self.query("_".into(), qry),
            Stmt::Expr { expr } => Tag::Core(Core::Expr { expr }),
            Stmt::Let(name, value @ Expr::Value(_)) => Tag::Core(Core::Set { var: name, value }),
            Stmt::Let(name, Expr::Instance { class, args }) => Tag::Core(Core::New {
                class_name: class,
                var: name,
                args: args
                    .into_iter()
                    .map(|arg| Core::Arg {
                        r#type: None,
                        value: arg,
                    })
                    .collect(),
            }),
            Stmt::Let(name, Expr::Http { verb, url, body }) => {
                let mut tags = vec![
                    Tag::Core(Core::New {
                        class_name: "java.net.URL".into(),
                        var: Ident::from("remoteURL"),
                        args: vec![Core::Arg {
                            r#type: Some("java.lang.String".into()),
                            value: *url,
                        }],
                    }),
                    Tag::Core(Core::Set {
                        var: name.clone(),
                        value: Expr::call("remoteURL.openConnection", vec![]),
                    }),
                    Tag::Core(Core::Expr {
                        expr: Expr::call(
                            format!("{name}.setRequestMethod"),
                            vec![verb.to_string().into()],
                        ),
                    }),
                    Tag::Core(Core::Expr {
                        expr: Expr::call(format!("{name}.setDoOutput"), vec![true.into()]),
                    }),
                ];

                if !matches!(verb, HttpVerb::GET) {
                    tags.push(Tag::Core(Core::Expr {
                        expr: Expr::call(format!("{name}.setDoInput"), vec![true.into()]),
                    }))
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

                            tags.push(Tag::Core(Core::Expr {
                                expr: Expr::call(
                                    format!("{name}.setConnectTimeout"),
                                    vec![timeout.clone()],
                                ),
                            }));

                            tags.push(Tag::Core(Core::Expr {
                                expr: Expr::call(format!("{name}.setReadTimeout"), vec![timeout]),
                            }));
                        }
                        "headers" => {
                            let Some(Expr::Dict(dict)) = args.pop() else {
                                unreachable!("kek")
                            };

                            tags.extend(dict.into_iter().map(|(k, v)| {
                                Tag::Core(Core::Expr {
                                    expr: Expr::call(
                                        format!("{name}.setRequestProperty"),
                                        vec![k.into(), v],
                                    ),
                                })
                            }));
                        }
                        "json" => {
                            let Some(Expr::Dict(dict)) = args.pop() else {
                                unreachable!("kek")
                            };
                            tags.push(Tag::Core(Core::Expr {
                                expr: Expr::call(
                                    format!("{name}.setRequestProperty"),
                                    vec!["content-type".into(), "application/json".into()],
                                ),
                            }));

                            tags.push(Tag::Core(Core::New {
                                class_name: "java.io.OutputStreamWriter".into(),
                                var: format!("{name}_w").into(),
                                args: vec![Core::Arg {
                                    r#type: Some("java.io.OutputStream".into()),
                                    value: Expr::call(format!("{name}.getOutputStream"), vec![]),
                                }],
                            }));

                            Self::create_json_tags(dict, name.as_str(), &mut tags);

                            tags.push(Tag::Core(Core::Expr {
                                expr: Expr::call(
                                    format!("{name}_payload.write"),
                                    vec![Expr::Ident(format!("{name}_w").into())],
                                ),
                            }));
                            tags.push(Tag::Core(Core::Expr {
                                expr: Expr::call(format!("{name}_w.flush"), vec![]),
                            }))
                        }
                        _ => unreachable!(),
                    }
                }

                tags.push(Tag::Core(Core::Expr {
                    expr: Expr::call(format!("{name}.connect"), vec![]),
                }));

                Tag::Macro(tags)
            }
            Stmt::Let(name, Expr::Json { expr }) => {
                let tags = vec![
                    Tag::Core(Core::Set {
                        var: "input_stream".into(),
                        value: Expr::call(format!("{expr}.getInputStream"), vec![]),
                    }),
                    Tag::Core(Core::New {
                        class_name: "java.io.InputStreamReader".into(),
                        var: "input_reader".into(),
                        args: vec![Core::Arg {
                            r#type: Some("java.io.InputStream".into()),
                            value: Expr::Ident("input_stream".into()),
                        }],
                    }),
                    Tag::Core(Core::New {
                        class_name: "java.io.BufferedReader".into(),
                        var: "buf_reader".into(),
                        args: vec![Core::Arg {
                            r#type: Some("java.io.InputStreamReader".into()),
                            value: Expr::Ident("input_reader".into()),
                        }],
                    }),
                    Tag::Core(Core::New {
                        class_name: "java.lang.StringBuilder".into(),
                        var: "sb".into(),
                        args: vec![],
                    }),
                    // <core:set value='${{buf_reader.readLine()}}' var='line'/>
                    Tag::Core(Core::Set {
                        var: "line".into(),
                        value: Expr::call("buf_reader.readLine", vec![]),
                    }),
                    // <!-- READ STREAM LINE BY LINE AND BUILD THE STRING -->\n\
                    // <core:while test='${{line != null}}'>\n\
                    Tag::Core(Core::While {
                        test: Expr::Value(Value::Str("${line != null}".to_string())),
                        body: vec![
                            //     <core:expr value='${{sb.append(line)}}'/>\n\
                            Tag::Core(Core::Expr {
                                expr: Expr::call("sb.append", vec![Expr::Ident("line".into())]),
                            }),
                            //     <core:set var='line' value='${{buf_reader.readLine()}}'/>\n\
                            Tag::Core(Core::Set {
                                var: "line".into(),
                                value: Expr::call("buf_reader.readLine", vec![]),
                            }),
                        ],
                    }),
                    // </core:while>
                    // <core:new className='org.json.JSONObject' var='{name}'>
                    //     <core:arg type='java.lang.String' value='${{sb.toString()}}'/>
                    // </core:new>
                    //
                    Tag::Core(Core::New {
                        class_name: "org.json.JSONObject".into(),
                        var: name,
                        args: vec![Core::Arg {
                            r#type: Some("java.lang.String".into()),
                            value: Expr::call("sb.toString", vec![]),
                        }],
                    }),
                ];

                Tag::Macro(tags)
            }
            Stmt::Let(name, Expr::StaticField(Name::Dotted { parent, mut attrs })) => {
                assert!(attrs.len() == 1);
                let Some(Name::Ident(attr)) = attrs.pop() else {
                    unreachable!()
                };

                Tag::Core(Core::GetStatic {
                    var: name,
                    class_name: *parent,
                    field: attr,
                })
            }
            Stmt::Let(
                name,
                Expr::Static(Call {
                    name: Name::Dotted { parent, attrs },
                    args,
                }),
            ) => {
                // <core:invokeStatic className="com.niku.union.security.UserSessionControllerFactory" method="getInstance" var="userSessionCtrl"/>
                let Some(Name::Ident(method)) = attrs.last().cloned() else {
                    unreachable!()
                };
                Tag::Core(Core::InvokeStatic {
                    var: name,
                    method,
                    class_name: *parent,
                    args: args
                        .into_iter()
                        .map(|arg| Core::Arg {
                            r#type: None,
                            value: arg,
                        })
                        .collect(),
                })
            }
            Stmt::Let(name, query @ Expr::Query { .. }) => self.query(name, query),
            Stmt::Let(name, expr) => Tag::Core(Core::Set {
                var: name,
                value: expr,
            }),
            Stmt::Alias { alias: ident, cls } => {
                self.env.insert(ident.to_string(), cls.clone());
                Tag::Noop
            }
            Stmt::ForEach { var, items, body } => Tag::Core(Core::ForEach {
                var,
                items,
                body: body
                    .into_iter()
                    .map(|stmt| self.transpile_node(stmt))
                    .collect(),
            }),

            Stmt::Log { level, message } => Tag::Gel(Gel::Log {
                level,
                message,
                category: None,
            }),

            Stmt::If {
                test,
                body,
                alt: Some(alt),
            } => Tag::Core(Core::Choose {
                branches: vec![
                    Core::When {
                        test,
                        body: body
                            .into_iter()
                            .map(|stmt| self.transpile_node(stmt))
                            .collect(),
                    },
                    Core::Otherwise {
                        body: alt
                            .into_iter()
                            .map(|stmt| self.transpile_node(stmt))
                            .collect(),
                    },
                ],
            }),
            Stmt::If {
                test,
                body,
                alt: None,
            } => Tag::Core(Core::If {
                test,
                body: body
                    .into_iter()
                    .map(|stmt| self.transpile_node(stmt))
                    .collect(),
            }),
            Stmt::Block(block) => Tag::Macro(
                block
                    .into_iter()
                    .map(|stmt| self.transpile_node(stmt))
                    .collect(),
            ),
            Stmt::Catch { name, body, no_err } => Tag::Macro(vec![
                Tag::Core(Core::Catch {
                    var: name.clone(),
                    body: no_err
                        .into_iter()
                        .map(|stmt| self.transpile_node(stmt))
                        .collect(),
                }),
                Tag::Core(Core::If {
                    test: Expr::Infix {
                        lhs: Box::new(Expr::Ident(Name::Ident(name))),
                        op: crate::gelatin::ast::InfixOp::Neq,
                        rhs: Box::new(Expr::Value(Value::Nothing)),
                    },
                    body: body
                        .into_iter()
                        .map(|stmt| self.transpile_node(stmt))
                        .collect(),
                }),
            ]),
        }
    }

    fn create_json_tags(map: HashMap<String, Expr>, bind_to: &str, tags: &mut Vec<Tag>) {
        tags.push(Tag::Core(Core::New {
            class_name: "org.json.JSONObject".into(),
            var: format!("{bind_to}_payload").into(),
            args: vec![],
        }));

        for (k, mut v) in map {
            if let Expr::Dict(inner) = v {
                Self::create_json_tags(inner, &format!("{bind_to}_{}", k.as_str()), tags);
                v = Expr::Ident(k.as_str().into());
            }

            tags.push(Tag::Core(Core::Expr {
                expr: Expr::call(format!("{bind_to}_payload.put"), vec![k.into(), v]),
            }));
        }
    }

    fn query(&self, name: Ident, expr: Expr) -> Tag {
        match expr {
            Expr::Query {
                datasource,
                r#type,
                query,
            } => {
                let sql = match r#type {
                    QueryType::SELECT => Sql::Query {
                        var: name,
                        sql: query.to_string(),
                    },
                    QueryType::UPDATE | QueryType::INSERT | QueryType::DELETE => Sql::Update {
                        var: name,
                        sql: query.to_string(),
                    },
                };

                Tag::Macro(vec![
                    Tag::Gel(Gel::SetDatasource { db_id: datasource }),
                    Tag::Sql(sql),
                ])
            }
            _ => unreachable!(),
        }
    }
}
