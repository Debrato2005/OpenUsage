#![cfg(target_os = "linux")]

use gtk::glib::variant::ToVariant;

// Exercise the C out-pointer through every affected iterator entry point.
// The workspace test profile compiles glib with optimizations enabled.
#[test]
fn variant_string_iteration_preserves_values_and_bounds() {
    let values = ["first", "", "नमस्ते", "last"];
    let variant = values.to_variant();
    assert_eq!(
        variant.array_iter_str().unwrap().collect::<Vec<_>>(),
        values
    );
    assert_eq!(
        variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        values.into_iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(variant.array_iter_str().unwrap().nth(2), Some("नमस्ते"));
    assert_eq!(variant.array_iter_str().unwrap().nth_back(2), Some(""));
    assert_eq!(variant.array_iter_str().unwrap().last(), Some("last"));
    assert_eq!(variant.array_iter_str().unwrap().nth(4), None);
    assert_eq!(variant.array_iter_str().unwrap().nth_back(4), None);

    let mut mixed = variant.array_iter_str().unwrap();
    let first = mixed.next().unwrap();
    assert_eq!(mixed.next_back(), Some("last"));
    assert_eq!(mixed.next(), Some(""));
    assert_eq!(mixed.next_back(), Some("नमस्ते"));
    assert_eq!(mixed.next(), None);
    assert_eq!(mixed.next_back(), None);
    assert_eq!(first, "first");

    let empty = Vec::<String>::new().to_variant();
    assert_eq!(empty.array_iter_str().unwrap().next(), None);
    assert_eq!(empty.array_iter_str().unwrap().next_back(), None);
    assert!(42u32.to_variant().array_iter_str().is_err());
}
