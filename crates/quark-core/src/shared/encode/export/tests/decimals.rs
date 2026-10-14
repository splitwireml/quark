//! Decimal export text must equal Python's `str(Decimal)`.

use super::{cell, text};
use crate::shared::encode::export::ExportCell;

fn decimal(value: &str) -> ExportCell {
    ExportCell::Decimal(value.to_owned())
}

#[test]
fn small_decimals_use_scientific_notation() {
    assert_eq!(
        cell("0.00000012::DECIMAL(18,8)", "DECIMAL(18,8)"),
        decimal("1.2E-7")
    );
    assert_eq!(
        cell("0.0000001230::DECIMAL(18,10)", "DECIMAL(18,10)"),
        decimal("1.230E-7")
    );
}

#[test]
fn zero_with_a_large_scale_keeps_its_exponent() {
    assert_eq!(cell("0::DECIMAL(18,8)", "DECIMAL(18,8)"), decimal("0E-8"));
    assert_eq!(
        cell("0::DECIMAL(18,6)", "DECIMAL(18,6)"),
        decimal("0.000000")
    );
}

#[test]
fn plain_decimals_keep_their_scale() {
    assert_eq!(
        cell("0.000001::DECIMAL(18,8)", "DECIMAL(18,8)"),
        decimal("0.00000100")
    );
    assert_eq!(
        cell("-12.3400::DECIMAL(10,4)", "DECIMAL(10,4)"),
        decimal("-12.3400")
    );
    assert_eq!(cell("42::DECIMAL(10,0)", "DECIMAL(10,0)"), decimal("42"));
}

#[test]
fn negative_small_decimals_keep_the_sign() {
    assert_eq!(
        cell("-0.00000005::DECIMAL(18,8)", "DECIMAL(18,8)"),
        decimal("-5E-8")
    );
}

#[test]
fn wide_small_decimals_use_scientific_text() {
    assert_eq!(
        cell(
            "0.000000000000000123456789012345678::DECIMAL(38,33)",
            "DECIMAL(38,33)"
        ),
        text("1.23456789012345678E-16")
    );
}

#[test]
fn nested_small_decimals_use_python_str() {
    assert_eq!(
        cell("[0.00000012::DECIMAL(18,8)]", "DECIMAL(18,8)[]"),
        text(r#"["1.2E-7"]"#)
    );
}
