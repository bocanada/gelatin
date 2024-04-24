use xml::name::Name;

#[derive(Debug, Clone, Copy)]
pub enum Gel {
    /// # Example:
    /// ```xml
    /// <gel:log level='{level}' category='' message='{message}'/>
    /// ```
    Log,
    /// # Example:
    /// ```xml
    /// <gel:setDataSource dbId="niku"/>
    /// ```
    SetDatasource,
    /// # Example:
    /// ```xml
    /// <gel:script .../>
    /// ```
    Script,
}

#[derive(Debug, Clone, Copy)]
pub enum Sql {
    Query,
    Update,
    Param,
}

#[derive(Debug, Clone, Copy)]
pub enum Soap {
    Invoke,
    Message,
}

#[derive(Debug, Clone, Copy)]
pub enum SoapEnv {
    Envelope,
    Header,
    Body,
}

#[derive(Debug, Clone, Copy)]
pub enum Core {
    /// A tag which evaluates an expression.
    ///
    /// # Example:
    /// ```xml
    /// <core:expr value='${1 + 1}'/>
    /// ```
    Expr,
    /// A tag which sets a variable from the result of an expression.
    ///
    /// # Example:
    /// ```xml
    /// <core:set var='name' value='${1 + 1}'/>
    /// ```
    Set,
    /// Iterates over a collection, iterator or an array of objects.
    /// Uses the same syntax as the JSTL `forEach` tag does.
    ///
    /// # Example:
    /// ```xml
    /// <core:forEach var='i' items='1, 2, 3'>
    ///
    /// </core:forEach>
    /// ```
    ForEach,
    /// An argument to a `org.apache.commons.jelly.tags.core.NewTag` or `org.apache.commons.jelly.tags.core.InvokeTag`.
    /// This tag MUST be enclosed within an `org.apache.commons.jelly.tags.core.ArgTagParentimplementation`.
    /// A tag which terminates the execution of the current <forEach> or <while> loop.
    ///
    /// # Example:
    /// ```xml
    /// <core:arg type='java.io.InputStream' value='${input_stream}'/>
    /// ```
    Arg,
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
    Break,
    /// A tag which creates a new child variable scope for its body.So any variables defined within its body will no longer be in scopeafter this tag.
    ///
    /// # Example:
    /// ```xml
    /// <core:scope>
    ///     <core:set var='scoped_var' value='123'/>
    /// </core:scope>
    /// ```
    Scope,
    /// A tag which creates a new object of the given type.
    ///
    /// Example:
    /// ```xml
    /// <core:new className='org.json.JSONObject' var='conn_object_payload'/>
    /// ```
    New,
    /// A Tag which can invoke a static method on a class, without aninstance of the class being needed.
    /// Like the org.apache.commons.jelly.tags.core.InvokeTag, this tag can take a set ofarguments using the org.apache.commons.jelly.tags.core.ArgTag.
    /// # Example: TODO
    InvokeStatic,
    /// A tag which conditionally evaluates its body based on some condition.
    Choose,
    /// A tag which conditionally evaluates its body based on some condition.
    When,
    /// The otherwise block of a choose/when group of tags
    Otherwise,
    /// A tag which conditionally evaluates its body if my value attribute equals my ancestor <switch> tag's "on" attribute.
    /// This tag must be contained within the body of some <switch> tag.
    Case,
    ///A tag which catches exceptions thrown by its body.
    ///This allows conditional logic to be performed based on if exceptionsare thrown or to do some kind of custom exception logging logic.
    Catch,
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
    GetStatic,
    /// A tag which conditionally evaluates its body based on some condition
    /// # Example:
    /// ```xml
    /// <core:if test="true">
    ///   <core:set var="isStrue" value="true"/>
    /// </core:if>
    If,
    //A tag which calls a method in an object instantied by core:new
    Invoke,
    //A tag which executes its body but passing no output. Using this tag will still take the time to perform toString on each objectreturned to the output (but this toString value is discarded.A future version should go more internally so that this is avoided.
    Mute,
    //A tag which sets the bean properties on the given bean.So if you used it as follows, for example using the <j:new>tag. <j:new className="com.acme.Person" var="person"/> <j:setProperties object="${person}" name="James" location="${loc}"/> Then it would set the name and location properties on the bean denoted bythe expression ${person}. This tag can also be nested inside a bean tag such as the <useBean>tagor a JellySwing tag to set one or more properties, maybe inside some conditionallogic.
    SetProperties,
    //Executes the child <case>tag whose value equals my on attribute.Executes a child <default>tag when present and no <case>tag hasyet matched.
    Switch,
    /// A tag which creates a List implementation and optionallyadds all of the elements identified by the items attribute.The exact implementation of List can be specified via theclass attribute
    UseList,
    /// A tag which performs an iteration while the result of an expression is true.
    /// # Example
    /// ```xml
    /// <core:while test="true">
    ///     <core:break/>
    /// </core:while>
    /// ```
    While,
    //A simple tag used to preserve whitespace inside its body
    Whitespace,
}

impl From<Core> for Name<'static> {
    fn from(value: Core) -> Self {
        value.as_str().into()
    }
}

impl From<Gel> for Name<'static> {
    fn from(value: Gel) -> Self {
        value.as_str().into()
    }
}

impl From<Sql> for Name<'static> {
    fn from(value: Sql) -> Self {
        value.as_str().into()
    }
}

impl Core {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expr => "core:expr",
            Self::Set => "core:set",
            Self::ForEach => "core:forEach",
            Self::Arg => "core:arg",
            Self::Break => "core:break",
            Self::Scope => "core:scope",
            Self::New => "core:new",
            Self::InvokeStatic => "core:invokeStatic",
            Self::Choose => "core:choose",
            Self::When => "core:when",
            Self::Otherwise => "core:otherwise",
            Self::Case => "core:case",
            Self::Catch => "core:catch",
            Self::Default => "core:default",
            Self::File => "core:file",
            Self::GetStatic => "core:getStatic",
            Self::If => "core:if",
            Self::Invoke => "core:invoke",
            Self::Mute => "core:mute",
            Self::SetProperties => "core:setProperties",
            Self::Switch => "core:switch",
            Self::UseList => "core:useList",
            Self::While => "core:while",
            Self::Whitespace => "core:whitespace",
        }
    }
}

impl Gel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Log => "gel:log",
            Self::SetDatasource => "gel:setDataSource",
            Self::Script => "gel:script",
        }
    }
}

impl Soap {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invoke => "soap:invoke",
            Self::Message => "soap:message",
        }
    }
}

impl SoapEnv {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Envelope => "soapenv:Envelope",
            Self::Header => "soapenv:Header",
            Self::Body => "soapenv:Body",
        }
    }
}

impl Sql {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Query => "sql:query",
            Self::Update => "sql:update",
            Self::Param => "sql:param",
        }
    }
}
