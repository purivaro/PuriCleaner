//! cargo run --release -p puri_core --example memory
use puri_core::clean::fmt_size;

fn main() {
    let mut m = puri_core::memory::Memory::default();
    let s = m.snapshot();
    println!("RAM {} / {}  swap {} / {}  pressure {:?}", fmt_size(s.used), fmt_size(s.total), fmt_size(s.swap_used), fmt_size(s.swap_total), s.pressure);
    for a in s.apps.iter().take(25) {
        println!("{:>9} {:>6.1}% {:>3}p {:>6}m  {:<10} {} {}", fmt_size(a.memory), a.cpu, a.processes, a.running_for / 60, format!("{:?}", a.kind), if a.can_quit { "QUIT" } else { "    " }, a.name);
    }
}
