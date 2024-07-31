use crate::pnp::part::Part;

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Placement {
    pub ref_des: String,
    pub part: Part,
    pub place: bool,
}
