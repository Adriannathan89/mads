fn main() {
    println!("unsupported inspection entry point");

    let heartbeat = std::env::var_os("MADS_TEST_HEARTBEAT_MARKER")
        .expect("inspection test should provide a heartbeat marker");
    for sequence in 0_u64.. {
        std::fs::write(&heartbeat, sequence.to_string())
            .expect("heartbeat marker should remain writable");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
