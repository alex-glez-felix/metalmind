//! Teclas arbitrarias al motor vim sobre un texto arbitrario, con uno o
//! varios cursores.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use nota_buffer::Buffer;
use nota_fuzz::{CountLimit, Input, MAX_BUFFER_LEN, check_engine, feed, initial_selections};
use nota_vim::Vim;

#[derive(Debug, Arbitrary)]
struct Case {
    text: String,
    cursors: Vec<u16>,
    inputs: Vec<Input>,
}

fuzz_target!(|case: Case| {
    let mut buffer = Buffer::from(case.text.as_str());
    let mut selections = initial_selections(&buffer, &case.cursors);
    let mut vim = Vim::new();
    let mut counts = CountLimit::default();
    for input in &case.inputs {
        if let Input::Key(key) = input
            && !counts.allows(key.key())
        {
            continue;
        }
        feed(&mut vim, input, &mut buffer, &mut selections);
        check_engine(&vim, &buffer, &selections);
        if buffer.len().get() > MAX_BUFFER_LEN {
            return;
        }
    }
});
