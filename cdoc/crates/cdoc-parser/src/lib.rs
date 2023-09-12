pub mod document;
pub mod raw;

#[cfg(test)]
use pest_test_gen::pest_tests;

#[pest_tests(
    crate::raw::RawDocParser,
    crate::raw::Rule,
    "doc",
    dir = "tests/pest/doc",
    strict = false,
    lazy_static = true
)]
#[cfg(test)]
mod raw_doc_tests {}
