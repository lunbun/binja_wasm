use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub struct ArcIdentity<T>(pub Arc<T>);

impl<T> Clone for ArcIdentity<T> {
    fn clone(&self) -> Self {
        ArcIdentity(self.0.clone())
    }
}

impl<T> PartialEq for ArcIdentity<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Eq for ArcIdentity<T> {}

impl<T> Hash for ArcIdentity<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use the pointer address for hashing, which is stable for the Rc allocation.
        std::ptr::hash(Arc::as_ptr(&self.0), state)
    }
}

impl<T> AsRef<T> for ArcIdentity<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> ArcIdentity<T> {
    pub fn new(value: T) -> Self {
        ArcIdentity(Arc::new(value))
    }
}
