use std::any::Any;

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
