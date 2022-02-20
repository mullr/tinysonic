// use std::borrow::BorrowMut;

// use qmetaobject::{QObject, QPointer, queued_callback};


// trait CallbackHelper {
//     fn capturing_queued_callback<T: Send>(
//         &mut self,
//         func: fn(&mut Self, T),
//     ) -> Box<dyn Fn(T) + Send + Sync>;
// }

// impl<Q: QObject> CallbackHelper for Q {
//     fn capturing_queued_callback<T: Send>(
//         &mut self,
//         func: fn(&mut Self, T),
//     ) -> Box<dyn Fn(T) + Send + Sync> {
//         let self_ptr = QPointer::from(self as &Self);
//         Box::new(queued_callback(move |x: T| {
//             if let Some(self_) = self_ptr.as_pinned().borrow_mut() {
//                 let mut self_mut = (*self_).borrow_mut();
//                 func(&mut self_mut, x);
//             }
//         }))
//     }
// }
