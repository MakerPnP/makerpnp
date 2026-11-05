#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct AssemblyRule {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
}
