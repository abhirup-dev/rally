mod tracing_init;

fn main() {
    let _guard = tracing_init::init();
    // Phase 3: clap command tree, IPC client, output formatting wired here.
}
