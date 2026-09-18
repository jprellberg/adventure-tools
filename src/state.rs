use std::rc::Rc;

use dioxus::prelude::*;

use crate::data::Library;

pub type LibrarySignal = Signal<Option<Rc<Library>>>;
