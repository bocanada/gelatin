pub mod tags;

use std::{borrow::Borrow, collections::HashMap, io};

use xml::{writer::XmlEvent, EventWriter};

use crate::{
    gelatin::ast::{Call, Context, Expr, Ident, Name, Node, QueryType, Stmt},
    transpiler::tags::{Soap, SoapEnv},
};

use self::tags::{Core, Gel, Sql};

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

pub struct Transpiler<W> {
    env: HashMap<String, Expr>,
    writer: EventWriter<W>,
}

macro_rules! close {
    ($writer:expr) => {
        $writer.write::<XmlEvent<'static>>(XmlEvent::end_element().into())?;
    };
}
macro_rules! auto_close {
    ($event:expr, $writer:expr) => {
        $writer.write($event)?;
        $writer.write::<XmlEvent<'static>>(XmlEvent::end_element().into())?;
    };
}

impl<W: io::Write> Transpiler<W> {
    pub fn new(sink: W, prettify: bool) -> Self {
        Self {
            env: HashMap::new(),
            writer: EventWriter::new_with_config(
                sink,
                xml::EmitterConfig::default().perform_indent(prettify),
            ),
        }
    }

    pub fn transpile<I>(&mut self, it: I) -> xml::writer::Result<()>
    where
        I: IntoIterator<Item = Node>,
    {
        //       <gel:script xmlns:core="jelly:core"
        // xmlns:gel="">
        self.writer.write(
            XmlEvent::start_element(Gel::Script)
                .ns("gel", "jelly:com.niku.union.gel.GELTagLibrary")
                .ns("core", "jelly:core"),
        )?;

        for node in it {
            self.as_tags(node)?;
        }

        close!(self.writer);
        Ok(())
    }

    pub fn as_tags(&mut self, node: Node) -> xml::writer::Result<()> {
        match node {
            Node::Stmt(stmt) => self.transpile_node(stmt),
            Node::Expr(expr) => self.transpile_node(Stmt::Expr { expr }),
        }
    }

    pub fn transpile_node(&mut self, stmt: Stmt) -> xml::writer::Result<()> {
        match stmt {
            Stmt::Expr {
                expr: expr @ Expr::Query { .. },
            } => self.transpile_node(Stmt::Let("_".into(), expr)),
            Stmt::While { test, body } => {
                let val = test.as_value(Context::Text);

                self.writer
                    .write(XmlEvent::start_element(Core::While).attr("test", val.borrow()))?;

                self.transpile_vec(body)?;

                close!(self.writer);
                Ok(())
            }
            Stmt::Expr { expr } => {
                let value = expr.as_value(Context::Text);
                auto_close!(
                    XmlEvent::start_element(Core::Expr).attr("value", &value),
                    self.writer
                );

                Ok(())
            }
            Stmt::Let(name, soap @ Expr::Soap { .. }) => self.transpile_soap(&name, soap),
            Stmt::Let(_, Expr::Http { .. } | Expr::Json { .. }) => {
                unreachable!("macro expanded")
            }
            Stmt::Let(_, _) => self.let_stmt(stmt),
            Stmt::Alias { alias: ident, cls } => {
                self.env.insert(ident.to_string(), cls);
                Ok(())
            }
            Stmt::ForEach { .. } => self.for_each(stmt),
            Stmt::Log { level, message } => {
                auto_close!(
                    XmlEvent::start_element(Gel::Log)
                        .attr("level", level.as_str())
                        .attr("message", &message),
                    self.writer
                );

                Ok(())
            }
            Stmt::If { .. } => self.if_stmt(stmt),
            Stmt::Block(block) => self.transpile_vec(block),
            Stmt::Catch { name, body } => {
                self.writer
                    .write(XmlEvent::start_element(Core::Catch).attr("var", name.as_str()))?;

                self.transpile_vec(body)?;

                close!(self.writer);
                Ok(())
            }
        }
    }

    fn let_stmt(&mut self, stmt: Stmt) -> xml::writer::Result<()> {
        match stmt {
            Stmt::Let(name, query @ Expr::Query { .. }) => self.query(&name, query),
            Stmt::Let(name, value @ Expr::Value(_)) => {
                let str = value.as_value(Context::Text);
                auto_close!(
                    XmlEvent::start_element(Core::Set)
                        .attr("value", &str)
                        .attr("var", name.as_str()),
                    self.writer
                );

                Ok(())
            }
            Stmt::Let(name, Expr::Instance { class, args }) => {
                self.writer.write(
                    XmlEvent::start_element(Core::New)
                        .attr("className", class.to_string().as_str())
                        .attr("var", name.as_str()),
                )?;

                self.transpile_args(args)?;

                close!(self.writer);

                Ok(())
            }
            Stmt::Let(name, Expr::StaticField(Name::Dotted { parent, mut attrs })) => {
                assert!(attrs.len() == 1);
                let Some(Name::Ident(attr)) = attrs.pop() else {
                    unreachable!()
                };

                auto_close!(
                    XmlEvent::start_element(Core::GetStatic)
                        .attr("var", name.as_str())
                        .attr("className", parent.to_string().as_str())
                        .attr("field", attr.as_str()),
                    self.writer
                );

                Ok(())
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

                self.writer.write(
                    XmlEvent::start_element(Core::InvokeStatic)
                        .attr("className", parent.to_string().as_str())
                        .attr("method", method.as_str())
                        .attr("var", name.as_str()),
                )?;

                self.transpile_args(args)?;

                close!(self.writer);
                Ok(())
            }
            Stmt::Let(name, expr) => {
                let expr = expr.as_value(Context::Text);
                auto_close!(
                    XmlEvent::start_element(Core::Set)
                        .attr("var", name.as_str())
                        .attr("value", expr.borrow()),
                    self.writer
                );

                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn for_each(&mut self, stmt: Stmt) -> xml::writer::Result<()> {
        match stmt {
            Stmt::ForEach {
                var,
                items: Expr::Range { start, end, step },
                body,
            } => {
                // <core:forEach var='i' items='1, 2, 3'>

                self.writer.write(
                    XmlEvent::start_element(Core::ForEach)
                        .attr("var", var.as_str())
                        .attr("start", format!("{start}").as_str())
                        .attr("step", format!("{step}").as_str())
                        .attr("end", format!("{end}").as_str()),
                )?;

                self.transpile_vec(body)?;

                close!(self.writer);
                Ok(())
            }

            Stmt::ForEach { var, items, body } => {
                // <core:forEach var='i' items='1, 2, 3'>

                self.writer.write(
                    XmlEvent::start_element(Core::ForEach)
                        .attr("var", var.as_str())
                        .attr("items", items.as_value(Context::Text).borrow()),
                )?;

                self.transpile_vec(body)?;

                close!(self.writer);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn if_stmt(&mut self, stmt: Stmt) -> xml::writer::Result<()> {
        match stmt {
            // if/else case,
            // make it into a core:choose.
            Stmt::If {
                test,
                body,
                alt: Some(alt),
            } => {
                self.writer.write(XmlEvent::start_element(Core::Choose))?;
                self.writer.write(
                    XmlEvent::start_element(Core::When)
                        .attr("test", test.as_value(Context::Text).borrow()),
                )?;

                self.transpile_vec(body)?;

                close!(self.writer);
                self.writer
                    .write(XmlEvent::start_element(Core::Otherwise))?;

                self.transpile_vec(alt)?;

                close!(self.writer);
                close!(self.writer);
                Ok(())
            }

            Stmt::If {
                test,
                body,
                alt: None,
            } => {
                self.writer.write(
                    XmlEvent::start_element(Core::If)
                        .attr("test", test.as_value(Context::Text).borrow()),
                )?;

                self.transpile_vec(body)?;

                close!(self.writer);
                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn query(&mut self, name: &Ident, expr: Expr) -> xml::writer::Result<()> {
        match expr {
            Expr::Query {
                datasource,
                r#type,
                query,
                params,
            } => {
                let tag = match r#type {
                    QueryType::SELECT => Sql::Query,
                    QueryType::UPDATE | QueryType::INSERT | QueryType::DELETE => Sql::Update,
                };

                auto_close!(
                    XmlEvent::start_element(Gel::SetDatasource)
                        .attr("dbId", datasource.to_string().as_str()),
                    self.writer
                );

                self.writer
                    .write(XmlEvent::start_element(tag).attr("var", name.as_str()))?;

                self.writer
                    .write(XmlEvent::cdata(query.to_string().as_str()))?;

                for param in params {
                    auto_close!(
                        XmlEvent::start_element(Sql::Param)
                            .attr("value", &param.as_value(Context::Text)),
                        self.writer
                    );
                }

                close!(self.writer);

                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn transpile_vec(&mut self, body: Vec<Stmt>) -> xml::writer::Result<()> {
        for stmt in body {
            self.transpile_node(stmt)?;
        }

        Ok(())
    }

    fn transpile_args(&mut self, args: Vec<Expr>) -> xml::writer::Result<()> {
        for arg in args {
            let arg = arg.as_value(Context::Text);
            auto_close!(
                XmlEvent::start_element(Core::Arg).attr("value", arg.borrow()),
                self.writer
            );
        }

        Ok(())
    }

    fn transpile_soap(&mut self, name: &Ident, soap: Expr) -> Result<(), xml::writer::Error> {
        //       <soapenv:Body>
        //                     <NikuDataBus xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../xsd/nikuxog_user.xsd">
        //                             <Header action="write" externalSource="NIKU" objectType="user" version="16.0.2.861" />
        //                             <Users>
        //                                     <User externalId=" " userName="${row.email}" oldUserName="${row.unique_name}" userLanguage="Spanish" userLocale="es" userUid="${row.unique_name}">
        //                                             <PersonalInformation emailAddress="${row.email}" firstName="${row.first_name}" lastName="${row.last_name}" />
        //                                     </User>
        //                             </Users>
        //                     </NikuDataBus>
        //       </soapenv:Body>
        let Expr::Soap {
            endpoint,
            header,
            body,
        } = soap
        else {
            unreachable!()
        };

        //   <soap:invoke endpoint="internal" var="result">
        self.writer.write(
            XmlEvent::start_element(Soap::Invoke.as_str())
                .attr("endpoint", &endpoint)
                .attr("var", name.as_str()),
        )?;

        //   <soap:message>
        self.writer
            .write(XmlEvent::start_element(Soap::Message.as_str()))?;

        //     <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:xog="http://www.niku.com/xog">
        self.writer.write(
            XmlEvent::start_element(SoapEnv::Envelope.as_str())
                .ns("soapenv", "http://schemas.xmlsoap.org/soap/envelope/")
                .ns("xog", "http://www.niku.com/xog"),
        )?;

        if let Some(header) = header {
            //       <soapenv:Header>
            self.writer
                .write(XmlEvent::start_element(SoapEnv::Header.as_str()))?;

            // skip the start document tag
            for el in header {
                match el.as_writer_event() {
                    Some(el) => self.writer.write(el)?,
                    None => break,
                }
            }

            //       </soapenv:Header>
            close!(self.writer);
        }

        if let Some(body) = body {
            //       <soapenv:Body>
            self.writer
                .write(XmlEvent::start_element(SoapEnv::Body.as_str()))?;

            // skip the start document tag
            for el in body {
                match el.as_writer_event() {
                    Some(el) => self.writer.write(el)?,
                    None => break,
                }
            }

            //       </soapenv:Body>
            close!(self.writer);
        }

        //     </soapenv:Envelope>
        close!(self.writer);
        //   </soap:message>
        close!(self.writer);
        // </soap:invoke>
        close!(self.writer);

        Ok(())
    }
}
