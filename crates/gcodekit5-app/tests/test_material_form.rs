//! `MaterialForm` number rules: the optional tensile strength and melting point must be
//! finite and above 0, since `serde_json` saves `NaN`/`inf` as `null` (lost on reload).

use gcodekit5_app::material_form::MaterialForm;

/// A form that builds, apart from the field under test.
fn valid_form() -> MaterialForm {
    MaterialForm {
        id: "walnut_slab".into(),
        name: "Walnut slab".into(),
        subcategory: "Hardwood".into(),
        ..MaterialForm::default()
    }
}

#[test]
fn test_material_form_refuses_non_finite_and_negative_numbers() {
    for bad in ["nan", "NaN", "inf", "-inf", "-5", "0"] {
        let mut form = valid_form();
        form.tensile_strength = bad.into();
        assert_eq!(
            form.build(None).err(),
            Some("Tensile strength must be a number"),
            "tensile strength {bad:?}"
        );
        let mut form = valid_form();
        form.melting_point = bad.into();
        assert_eq!(
            form.build(None).err(),
            Some("Melting point must be a number"),
            "melting point {bad:?}"
        );
    }
}

#[test]
fn test_material_form_accepts_positive_and_empty_numbers() {
    let mut form = valid_form();
    form.tensile_strength = " 95.5 ".into();
    form.melting_point = "1400".into();
    let material = form.build(None).expect("valid");
    assert_eq!(material.tensile_strength, Some(95.5));
    assert_eq!(material.melting_point, Some(1400.0));
    let material = valid_form().build(None).expect("valid");
    assert_eq!(material.tensile_strength, None);
    assert_eq!(material.melting_point, None);
}
