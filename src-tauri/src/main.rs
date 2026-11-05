// Sin ventana de consola en las compilaciones de Windows (no afecta a Linux).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    virelo_lib::run()
}
