//! The troop that rides in after the fact and writes down what a human responder actually did, because the agent log alone cannot be trusted.
//!
//! An automated run stalls, and a person steps in and rescues
//! it by hand. Cavalry is where that rescue is written down.
//! The library carries the records discipline, what a record
//! is, how it is validated and classified and stored and
//! confirmed, the judgment seam a recorded judgment crosses,
//! and the Python compatible JSON layer the byte parity of the
//! book rests on. Cavalry performs no rescue itself. Westering
//! rescues automatically on its ladder, and that ladder is
//! westering's own. Cavalry keeps the record, and the record is
//! the whole of its work.

pub mod judgment;
pub mod pyjson;
pub mod pytext;
pub mod records;
