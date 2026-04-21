fn main() {
    let json: serde_json::Value = serde_json::from_str(r#"[[["Hola","Hello",null,null,1]],null,"en"]"#).unwrap();
    let translated = json
        .get(0)
        .and_then(|v| v.get(0))
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    println!("{:?}", translated);
}