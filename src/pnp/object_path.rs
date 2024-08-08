use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;
use crate::planning::UnitPath;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct ObjectPath {
    elements: Vec<(String, String)>
}

impl ObjectPath {
    pub fn push(&mut self, key: String, value: String) {
        self.elements.push((key, value));
    }

    pub fn try_from_unit_path_and_refdes(unit_path: &UnitPath, ref_des: &String) -> Result<Self, ObjectPathError> {
        let mut path: ObjectPath = ObjectPath::from_str(&unit_path.to_string())?;
        path.push("ref_des".to_string(), ref_des.clone());

        Ok(path)
    }

    // TODO only works for panel units right now, add support for single pcbs.
    pub fn pcb_unit(&self) -> ObjectPath {
        const PCB_UNIT_KEYS: [&str; 2] = ["panel", "unit"];
        
        self.elements.iter().fold(ObjectPath::default(), | mut object_path, (key, value) | {

            if PCB_UNIT_KEYS.contains(&key.as_str()) {
                object_path.push(key.clone(), value.to_string())
            }
            
            object_path  
        })
    }
}
 
#[cfg(test)]
mod pcb_unit_tests {
    use super::*;

    #[test]
    pub fn pcb_unit() {
        // given
        let object_path = ObjectPath::from_str("panel=1::unit=1::ref_des=R1").expect("always ok");
        
        // and
        let expected_result = ObjectPath::from_str("panel=1::unit=1").expect("always ok");
        
        // when
        let result = object_path.pcb_unit();
        
        // then
        assert_eq!(result, expected_result);
    }
}

impl Display for ObjectPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted_pairs: Vec<String> = self.elements.iter()
            .map(|pair| format!("{}={}", pair.0, pair.1))
            .collect();
        
        write!(f, "{}",
           formatted_pairs.join("::")
        )
    }
}

impl FromStr for ObjectPath {
    type Err = ObjectPathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.split("::")
            .fold(Ok(ObjectPath::default()), |mut acc, chunk| {
                match &mut acc {
                    Ok(object_path) => {
                        let parts: Vec<&str> = chunk.split('=').collect();
                        if parts.len() == 2 {
                            let pair = (parts[0].to_string(), parts[1].to_string());
                            object_path.elements.push(pair);
                        } else {
                            acc = Err(ObjectPathError::Invalid(value.to_string()))
                        }
                    },
                    _ => ()
                }

                acc
            })
    }
}

#[derive(Error, Debug)]
pub enum ObjectPathError {
    #[error("Invalid object path. value: '{0:}'")]
    Invalid(String)
}