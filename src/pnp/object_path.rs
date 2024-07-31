use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct ObjectPath {
    elements: Vec<(String, String)>
}

impl ObjectPath {
    pub fn push(&mut self, key: String, value: String) {
        self.elements.push((key, value));
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