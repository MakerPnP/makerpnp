use std::any::Any;
use crate::util::dynamic::as_any::AsAny;

pub trait DynamicEq {
    fn dynamic_eq(&self, other: &dyn Any) -> bool;
}

impl<T: PartialEq + 'static> DynamicEq for T {
    fn dynamic_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref().map_or(false, |other|
            self == other
        )
    }
}
