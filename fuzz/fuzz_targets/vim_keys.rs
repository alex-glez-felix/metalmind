//! Teclas arbitrarias al motor vim sobre un texto arbitrario, con uno o
//! varios cursores, y comandos de varias teclas (Visual, registros, macros,
//! marks y búsqueda) que el fuzzer no armaría solo.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nota_fuzz::{CountLimit, Host, Input};
use nota_vim::Vim;

#[derive(Debug, Arbitrary)]
struct Case {
    text: String,
    cursors: Vec<u16>,
    inputs: Vec<Input>,
}

fuzz_target!(|case: Case| {
    let mut host = Host::new(&case.text, &case.cursors);
    let mut vim = Vim::new();
    let mut counts = CountLimit::default();
    for input in &case.inputs {
        if let Input::Key(key) = input
            && !counts.allows(key.key())
        {
            continue;
        }
        host.feed(&mut vim, input);
        host.check(&vim);
        if host.too_long() {
            return;
        }
    }
});
