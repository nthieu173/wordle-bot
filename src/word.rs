pub fn is_clean(word: &str) -> bool {
    let mut chars = word.chars();
    let first_char = chars.next().unwrap();
    if first_char.is_uppercase() {
        return false;
    }
    for c in chars {
        if !c.is_ascii_alphabetic() {
            return false;
        }
    }
    true
}
