fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>().join("|");
    println!("matrix server args={arguments}");
}
