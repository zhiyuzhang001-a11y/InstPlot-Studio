use instplot_export::{resolve, to_pdf};
use instplot_render::{compile, fixed_figure};
use lopdf::{Document, Object};

#[test]
fn pdf_has_physical_page_vector_paths_embedded_fonts_and_searchable_text() {
    let display_list = compile(&fixed_figure()).unwrap();
    let resolved = resolve(&display_list);
    let pdf = to_pdf(&resolved).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));

    let document = Document::load_mem(&pdf).unwrap();
    let pages = document.get_pages();
    assert_eq!(pages.len(), 1);
    let page_id = *pages.values().next().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let media_box = resolve_array(&document, page.get(b"MediaBox").unwrap());
    assert_close(number(&media_box[2]), 252.28346, 0.001);
    assert_close(number(&media_box[3]), 184.25197, 0.001);

    let mut font_objects = 0;
    let mut embedded_font_streams = 0;
    let mut image_objects = 0;
    for object in document.objects.values() {
        let Ok(dictionary) = object.as_dict() else {
            continue;
        };
        match dictionary.get(b"Type").and_then(Object::as_name).ok() {
            Some(b"Font") => font_objects += 1,
            Some(b"XObject")
                if dictionary.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") =>
            {
                image_objects += 1;
            }
            _ => {}
        }
        if dictionary.has(b"FontFile")
            || dictionary.has(b"FontFile2")
            || dictionary.has(b"FontFile3")
        {
            embedded_font_streams += 1;
        }
    }
    assert!(font_objects >= 1, "expected an embedded publication font");
    assert!(
        embedded_font_streams >= 1,
        "expected an embedded font subset"
    );
    assert_eq!(image_objects, 0, "the page must not be rasterized");

    let extracted = pdf_extract::extract_text_from_mem(&pdf).unwrap();
    assert!(extracted.contains("Experiment"), "{extracted:?}");
    assert!(extracted.contains("Fit"), "{extracted:?}");
    let compact: String = extracted
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact.contains("T≤300K"), "{extracted:?}");
    assert!(compact.contains("μ0HDL(mT)"), "{extracted:?}");
    assert!(compact.contains("CurrentdensityJe(Am−2)"), "{extracted:?}");
}

fn resolve_array(document: &Document, object: &Object) -> Vec<Object> {
    match object {
        Object::Array(values) => values.clone(),
        Object::Reference(id) => document
            .get_object(*id)
            .unwrap()
            .as_array()
            .unwrap()
            .clone(),
        _ => panic!("expected array"),
    }
}

fn number(object: &Object) -> f64 {
    match object {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => *value as f64,
        other => panic!("expected number, got {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}
