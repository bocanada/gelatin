use crate::gelatin::ast::{Context, Datasource, Expr, Ident, LogLevel, Name};

pub enum Gel {
    /// <gel:log level='{level}' category='' message='{message}'/>
    Log {
        level: LogLevel,
        category: Option<String>,
        message: String,
    },
    /// <gel:setDatasource dbId="niku"/>
    SetDatasource { db_id: Datasource },
    Script {
        body: Vec<Tag>,
    }
}

#[derive(Debug)]
pub enum Sql {
    Query { var: Ident, sql: String },
    Update { var: Ident, sql: String },
    Param { value: Expr },
}

pub enum Core {
    /// A tag which evaluates an expression.
    ///
    /// # Example:
    /// ```xml
    /// <core:expr value='${1 + 1}'/>
    /// ```
    Expr {
        expr: Expr,
    },
    /// A tag which sets a variable from the result of an expression.
    ///
    /// # Example:
    /// ```xml
    /// <core:set var='name' value='${1 + 1}'/>
    /// ```
    Set {
        var: Ident,
        value: Expr,
    },
    /// Iterates over a collection, iterator or an array of objects.
    /// Uses the same syntax as the JSTL `forEach` tag does.
    ///
    /// # Example:
    /// ```xml
    /// <core:forEach var='i' items='1, 2, 3'>
    ///
    /// </core:forEach>
    /// ```
    ForEach {
        var: Ident,
        items: Expr,
        body: Vec<Tag>,
    },
    /// An argument to a `org.apache.commons.jelly.tags.core.NewTag` or `org.apache.commons.jelly.tags.core.InvokeTag`.
    /// This tag MUST be enclosed within an `org.apache.commons.jelly.tags.core.ArgTagParentimplementation`.
    /// A tag which terminates the execution of the current <forEach> or <while> loop.
    ///
    /// # Example:
    /// ```xml
    /// <core:arg type='java.io.InputStream' value='${input_stream}'/>
    /// ```
    Arg {
        r#type: Option<Name>,
        value: Expr,
    },
    /// This tag can take an optional boolean test attribute which if its true then the break occurs otherwise, the loop continues processing.
    ///
    /// # Example:
    /// ```xml
    /// <core:break test='${1 == 2}'/>
    /// ```
    /// or
    /// ```xml
    /// <core:break/>
    /// ```
    Break {
        test: Option<Expr>,
    },
    /// A tag which creates a new child variable scope for its body.So any variables defined within its body will no longer be in scopeafter this tag.
    ///
    /// # Example:
    /// ```xml
    /// <core:scope>
    ///     <core:set var='scoped_var' value='123'/>
    /// </core:scope>
    /// ```
    Scope {
        body: Vec<Tag>,
    },
    /// A tag which creates a new object of the given type.
    ///
    /// Example:
    /// ```xml
    /// <core:new className='org.json.JSONObject' var='conn_object_payload'/>
    /// ```
    New {
        /// The class name.
        class_name: Name,
        /// The variable to bind this instance to.
        var: Ident,
        /// A vector of `Core::Arg`
        args: Vec<Core>,
    },
    /// A Tag which can invoke a static method on a class, without aninstance of the class being needed.
    /// Like the org.apache.commons.jelly.tags.core.InvokeTag, this tag can take a set ofarguments using the org.apache.commons.jelly.tags.core.ArgTag.
    InvokeStatic {
        /// The variable to assign the return of the method call to method
        var: Ident,
        /// The name of the static method to invoke className
        method: Ident,
        /// The name of the class containing the static method
        class_name: Name,

        args: Vec<Core>,
    },
    /// A tag which conditionally evaluates its body based on some condition.
    Choose {
        // Vec<Core::When>
        branches: Vec<Core>,
    },
    /// A tag which conditionally evaluates its body based on some condition.
    When {
        test: Expr,
        body: Vec<Tag>,
    },
    /// The otherwise block of a choose/when group of tags
    Otherwise {
        body: Vec<Tag>,
    },

    /// A tag which conditionally evaluates its body if my value attribute equals my ancestor <switch> tag's "on" attribute.
    /// This tag must be contained within the body of some <switch> tag.
    Case,
    ///A tag which catches exceptions thrown by its body.
    ///This allows conditional logic to be performed based on if exceptionsare thrown or to do some kind of custom exception logging logic.
    Catch {
        var: Ident,
        body: Vec<Tag>,
    },
    /// A tag which conditionally evaluates its body if none of its preceeding sibling <case> tags have been evaluated.
    /// This tag must be contained within the body of some <switch> tag.
    Default,
    /// A tag that pipes its body to a file denoted by the name attribute or to an in memory Stringwhich is then output to a variable denoted by the var variable.
    File,
    /// A tag which can retrieve the value of a static field of a given class.
    /// # Required attributes:
    /// - `var` - The variable to which to assign the resulting value.
    /// - `field` - The name of the static field to retrieve.
    /// - `className` - The name of the class containing the static field.
    ///
    /// # Example usage
    /// ```xml
    /// <j:getStatic var="closeOperation" className="javax.swing.JFrame" field="EXIT_ON_CLOSE"/>
    /// ```
    GetStatic {
        var: Ident,
        class_name: Name,
        field: Ident,
    },
    //A tag which conditionally evaluates its body based on some condition
    If {
        test: Expr,
        body: Vec<Tag>,
    },
    //Imports another script. By default, the imported script does not have access tothe parent script's variable context. This behaviourmay be modified using the inherit attribute.
    Import,
    //A tag which conditionally evaluates its body based on some condition
    Include,
    //A tag which calls a method in an object instantied by core:new
    Invoke,
    //A tag which executes its body but passing no output. Using this tag will still take the time to perform toString on each objectreturned to the output (but this toString value is discarded.A future version should go more internally so that this is avoided.
    Mute,
    //A tag which sets the bean properties on the given bean.So if you used it as follows, for example using the <j:new>tag. <j:new className="com.acme.Person" var="person"/> <j:setProperties object="${person}" name="James" location="${loc}"/> Then it would set the name and location properties on the bean denoted bythe expression ${person}. This tag can also be nested inside a bean tag such as the <useBean>tagor a JellySwing tag to set one or more properties, maybe inside some conditionallogic.
    SetProperties,
    //Executes the child <case>tag whose value equals my on attribute.Executes a child <default>tag when present and no <case>tag hasyet matched.
    Switch,
    //A tag which creates a List implementation and optionallyadds all of the elements identified by the items attribute.The exact implementation of List can be specified via theclass attribute
    UseList,
    //A tag which performs an iteration while the result of an expression is true.
    While {
        test: Expr,
        body: Vec<Tag>,
    },
    //A simple tag used to preserve whitespace inside its body
    Whitespace,
}

pub enum Tag {
    Core(Core),
    Gel(Gel),
    Sql(Sql),
    Text(Expr),
    Macro(Vec<Tag>),
    Noop,
}

impl std::fmt::Display for Gel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Gel::Log {
                level,
                category,
                message,
            } => {
                writeln!(
                    f,
                    "<gel:log category='{}' level='{}' message='{message}'/>",
                    category.clone().unwrap_or_default(),
                    level.as_str(),
                )
            }
            Gel::SetDatasource { db_id } => writeln!(f, "<gel:setDataSource dbId='{db_id}'/>"),
            Gel::Script { body: _ } => todo!(),
        }
    }
}

impl std::fmt::Display for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Core::Expr { expr } => {
                writeln!(f, "<core:expr value='{}'/>", expr.as_value(Context::Text))
            }
            Core::Set { var, value } => {
                writeln!(
                    f,
                    "<core:set value='{}' var='{var}'/>",
                    value.as_value(Context::Text)
                )
            }
            Core::ForEach {
                var,
                items: Expr::Range { start, end, step },
                body,
            } => {
                writeln!(
                    f,
                    "<core:forEach var='{var}' start='{start}' end='{end}' step ='{step}'>"
                )?;

                for stmt in body {
                    stmt.fmt(f)?
                }

                writeln!(f, "</core:forEach>")
            }
            Core::ForEach { var, items, body } => {
                writeln!(
                    f,
                    "<core:forEach var='{var}' items='{}'>",
                    items.as_value(Context::Text)
                )?;

                for stmt in body {
                    stmt.fmt(f)?
                }

                writeln!(f, "</core:forEach>")
            }
            Core::New {
                class_name,
                var,
                args,
            } => {
                // <core:new className='java.io.InputStreamReader' var='input_reader'>
                // <core:arg type='java.io.InputStream' value='${input_stream}' />
                // </core:new>
                write!(f, "<core:new className='{class_name}' var='{var}'",)?;

                if args.is_empty() {
                    writeln!(f, "/>")
                } else {
                    writeln!(f, ">")?;

                    for arg in args {
                        arg.fmt(f)?;
                    }
                    writeln!(f, "</core:new>")
                }
            }
            Core::Arg { r#type, value } => {
                let ty = match r#type {
                    Some(ty) => format!("type='{ty}'"),
                    None => "".to_string(),
                };
                writeln!(
                    f,
                    "<core:arg {} value='{}'/>",
                    ty,
                    value.as_value(Context::Text)
                )
            }
            Core::Scope { body } => {
                write!(f, "<core:scope>")?;
                for stmt in body {
                    stmt.fmt(f)?;
                }
                write!(f, "</core:scope>")
            }
            Core::While { test, body } => {
                writeln!(f, "<core:while test='{}'>", test.as_value(Context::Text))?;
                for stmt in body {
                    stmt.fmt(f)?
                }
                writeln!(f, "</core:while>")
            }
            Core::Break { test: None } => writeln!(f, "<core:break/>"),
            Core::Break { test: Some(test) } => {
                writeln!(f, "<core:break test='{}'/>", test.as_value(Context::Text))
            }
            Core::Case => todo!(),
            Core::Catch { var, body } => {
                writeln!(f, "<core:catch var='{var}'>")?;
                for stmt in body {
                    stmt.fmt(f)?;
                }

                writeln!(f, "</core:catch>")
            }
            Core::Default => todo!(),
            Core::File => todo!(),
            Core::GetStatic {
                var,
                class_name,
                field,
            } => writeln!(
                f,
                "<core:getStatic var='{var}' className='{class_name}' field='{field}'/>"
            ),
            Core::If { test, body } => {
                writeln!(f, "<core:if test='{}'>", test.as_value(Context::Text))?;
                for stmt in body {
                    stmt.fmt(f)?;
                }
                writeln!(f, "</core:if>")
            }
            Core::Import => todo!(),
            Core::Include => todo!(),
            Core::Invoke => todo!(),
            Core::InvokeStatic {
                var,
                class_name,
                method,
                args,
            } => {
                writeln!(
                    f,
                    "<core:invokeStatic var='{var}' className='{class_name}' method='{method}'>"
                )?;

                for arg in args {
                    arg.fmt(f)?;
                }

                writeln!(f, "</core:invokeStatic>")
            }
            Core::Choose { branches } => {
                writeln!(f, "<core:choose>")?;
                for branch in branches {
                    branch.fmt(f)?;
                }

                writeln!(f, "</core:choose>")
            }
            Core::When { test, body } => {
                writeln!(f, "<core:when test='{}'>", test.as_value(Context::Text))?;
                for stmt in body {
                    stmt.fmt(f)?;
                }
                writeln!(f, "</core:when>")
            }
            Core::Otherwise { body } => {
                writeln!(f, "<core:otherwise>")?;
                for stmt in body {
                    stmt.fmt(f)?;
                }
                writeln!(f, "</core:otherwise>")
            }
            Core::Mute => todo!(),
            Core::SetProperties => todo!(),
            Core::Switch => todo!(),
            Core::UseList => todo!(),
            Core::Whitespace => todo!(),
        }
    }
}

impl std::fmt::Display for Sql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sql::Query { var, sql } => {
                writeln!(
                    f,
                    "<sql:query escapeText='false' var='{var}'><![CDATA[/*sql*/ {sql}]]></sql:query>"
                )
            }
            Sql::Update { var, sql } => {
                writeln!(
                    f,
                    "<sql:update escapeText='false' var='{var}'><![CDATA[/*sql*/{sql}]]></sql:update>"
                )
            }
            Sql::Param { value: _ } => todo!(),
        }
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::Core(core) => core.fmt(f),
            Tag::Gel(gel) => gel.fmt(f),
            Tag::Macro(tags) => {
                for tag in tags {
                    tag.fmt(f)?;
                }
                Ok(())
            }
            Tag::Text(_) => todo!(),
            Tag::Noop => Ok(()),
            Tag::Sql(sql) => sql.fmt(f),
        }
    }
}
