mod tracing_init;

fn main() {
    let _guard = tracing_init::init();
    // Phase 3: daemon startup — tokio runtime, IPC server, services wired here.
}
