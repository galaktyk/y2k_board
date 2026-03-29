#[test]
fn test_woff_decoding() {
    let symbols2 = std::fs::read("symbols2.woff2").unwrap();
    let decoded = woff2_patched::convert_woff2_to_ttf(&mut std::io::Cursor::new(symbols2)).unwrap();
    println!("symbols2 decoded: {:?}", decoded.len());
    let mut db = cosmic_text::fontdb::Database::new();
    let ids = db.load_font_source(cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(
        decoded,
    )));
    println!("symbols2 loaded faces: {:?}", ids.len());
    for face in db.faces() {
        let n: Vec<_> = face.families.iter().map(|f| f.0.clone()).collect();
        println!("face: {:?}", n);
    }
}
