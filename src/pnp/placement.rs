#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone)]
pub struct Placement {
    pub ref_des: String,
}

impl Placement {
    pub fn new(ref_des: String) -> Self {
        Self {
            ref_des
        }
    }
}
