use std::{ops::Deref, rc::Rc, sync::Arc};

pub trait ReferenceCounter: Clone + Deref<Target = Self::Inner> {
    type Inner;
    fn new(obj: Self::Inner) -> Self;
}

impl<T> ReferenceCounter for Rc<T> {
    type Inner = T;
    fn new(obj: Self::Inner) -> Self {
        Rc::new(obj)
    }
}

impl<T> ReferenceCounter for Arc<T> {
    type Inner = T;
    fn new(obj: Self::Inner) -> Self {
        Arc::new(obj)
    }
}
