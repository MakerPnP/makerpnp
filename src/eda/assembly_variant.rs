pub struct AssemblyVariant {
    pub name: String,
    pub ref_des_list: Vec<String>,
}

impl Default for AssemblyVariant {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            ref_des_list: vec![],
        }
    }
}

impl AssemblyVariant {
    pub fn new(name: String, variant_refdes_list: Vec<String>) -> Self {
        Self {
            name,
            ref_des_list: variant_refdes_list
        }
    }
}