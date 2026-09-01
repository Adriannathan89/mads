fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    println!("cwd={}", std::env::current_dir().unwrap().display());
    println!("args={}", arguments.join("|"));
    if let Some(code) = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--exit="))
    {
        std::process::exit(code.parse().unwrap());
    }
}
