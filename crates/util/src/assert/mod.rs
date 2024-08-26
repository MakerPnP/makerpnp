#[macro_use]
pub mod ordered {
    #[macro_export]
    macro_rules! assert_contains_inorder {
    ($content: expr, [$($element:expr,)+]) => {
        let _index = 0;
        let _remainder = &$content;
        $(
            let element = $element;
            let message = format!("unmatched ordered element\nindex: {}\nexpected:\n{}\ncontent:\n{}\n", _index, element, _remainder);
            let (_, _remainder) = _remainder.split_once(element).expect(message.as_str());
            let _index = _index + 1;
        )*
    }
}

    #[macro_export]
    macro_rules! assert_inorder {
    ($content:expr, $expected:expr) => {
        {
            let remainder = $content;
            let expected = $expected;
            let (_, remainder) = remainder.split_once(expected).expect(format!("expected string not found, expected: '{}', remaining content: '{}'", expected, remainder).as_str());

            remainder.to_string()
        }
    };
}
}