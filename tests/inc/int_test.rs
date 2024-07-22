use predicates::function::FnPredicate;
use predicates::prelude::predicate;

pub fn print(message: &str) -> FnPredicate<fn(&str) -> bool, str> {
    println!("{}:", message);
    predicate::function(|content| {
        println!("{}", content);
        true
    })
}
