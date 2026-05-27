use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum Receiver<K> {
    All,
    Concrete(K),
    ConcreteMulti(Vec<K>),
}
