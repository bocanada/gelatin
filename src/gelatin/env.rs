use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Env<T> {
    Parent(HashMap<String, T>),
    Child {
        parent: Box<Env<T>>,
        bindings: HashMap<String, T>,
    },
}

impl<T> Env<T>
where
    T: Clone,
{
    /// Create a new environment.
    pub fn new() -> Self {
        Self::Parent(HashMap::new())
    }

    /// Create a scoped environment.
    pub(crate) fn scoped(&self) -> Self {
        Self::Child {
            parent: Box::new(self.clone()),
            bindings: HashMap::new(),
        }
    }

    pub fn resolve(&self, key: &str) -> Option<&T> {
        match self {
            Self::Parent(bindings) => bindings.get(key),
            Self::Child { parent, bindings } => {
                if let Some(expr) = bindings.get(key) {
                    return Some(expr);
                }
                parent.resolve(key)
            }
        }
    }

    pub fn bind(&mut self, key: String, val: T) -> Option<T> {
        match self {
            Self::Parent(bindings)
            | Self::Child {
                parent: _,
                bindings,
            } => bindings.insert(key, val),
        }
    }
}
