#[test]
fn dbg() {
    let d = crate::installer::installs_dir();
    eprintln!("installs_dir = {}", d.display());
    let m = d.join("node").join("26.6.0");
    eprintln!("managed = {}", m.display());
    eprintln!("is_managed = {}", crate::installer::is_managed(&m.to_string_lossy()));
    eprintln!("HOME = {:?}", std::env::var("HOME"));
    assert!(crate::installer::is_managed(&m.to_string_lossy()));
}
