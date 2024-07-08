pub struct AssemblyVariant {
    pub name: String,
    pub ref_des_list: Vec<String>,
}

impl AssemblyVariant {
    pub fn new(name: String, variant_refdes_list: Vec<String>) -> Self {
        Self {
            name,
            ref_des_list: variant_refdes_list
        }
    }
}