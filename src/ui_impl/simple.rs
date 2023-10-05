use crate::ui_interface::{SimpleTrait, SimpleEmitter};

use super::Albums;


pub struct Simple {
    emit: SimpleEmitter,
    message: String,
    albums: u64
}

impl Simple {
    fn albums(&self) -> &Albums {
        unsafe { &*(self.albums as *const Albums) }
    }
}

impl SimpleTrait for Simple {
    fn new(emit: SimpleEmitter) -> Simple {
        Simple {
            emit,
            message: String::new(),
            albums: 0
        }
    }

    fn emit(&mut self) -> &mut SimpleEmitter {
        &mut self.emit
    }

    fn message(&self) -> &str {
        "Hello World!" 
    }

    fn set_message(&mut self, value: String) {
        self.message = value;
        self.emit.message_changed();
    }
}
