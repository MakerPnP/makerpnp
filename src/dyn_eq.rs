use std::any::Any;
use crate::as_any::AsAny;

pub trait DynEq: AsAny {
    fn dyn_eq(&self, other: &dyn Any) -> bool;
}

impl<T: PartialEq + 'static> DynEq for T {
    fn dyn_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref().map_or(false, |other|
            self == other
        )
    }
}
