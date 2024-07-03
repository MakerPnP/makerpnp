use crate::part::Part;
use crate::part_mapper::criteria::PartMappingCriteria;

#[derive(Debug, PartialEq)]
pub struct PartMapping<'part>
{
    pub part: &'part Part,
    pub criteria: Vec<Box<dyn PartMappingCriteria>>,
}

impl<'part> PartMapping<'part> {
    pub fn new(part: &'part Part, criteria: Vec<Box<dyn PartMappingCriteria>>) -> Self {
        Self {
            part,
            criteria
        }
    }
}