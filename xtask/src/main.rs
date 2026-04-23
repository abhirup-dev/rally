fn main() {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("schema") => eprintln!("xtask schema: not yet implemented"),
        Some("ci") => eprintln!("xtask ci: not yet implemented"),
        Some(t) => eprintln!("unknown task: {t}"),
        None => eprintln!("usage: cargo xtask <task>"),
    }
}
