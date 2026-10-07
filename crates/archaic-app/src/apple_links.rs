//! AppKit URL-open events feed the same bounded activation queue as command-line links.
use objc2::{AnyThread, DefinedClass, define_class, msg_send, rc::Retained, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject};
use std::sync::mpsc::SyncSender;
struct HandlerIvars {
    sender: SyncSender<crate::instance::Open>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[name="ArchaicMatrixURLHandler"]
    #[ivars=HandlerIvars]
    struct Handler;
    impl Handler {
        #[unsafe(method(handleURL:withReply:))]
        fn handle_url(&self,event:&NSAppleEventDescriptor,_reply:&NSAppleEventDescriptor) {
            if let Some(value)=event.descriptorForKeyword(u32::from_be_bytes(*b"----")).and_then(|d|d.stringValue()) {
                let link=value.to_string();
                if archaic_matrix::links::parse(&link).is_ok() {let _=self.ivars().sender.try_send(crate::instance::Open{link:Some(link),profile:None});}
            }
        }
    }
);
pub struct AppleLinks {
    _handler: Retained<Handler>,
}
impl AppleLinks {
    pub fn new(sender: SyncSender<crate::instance::Open>) -> Self {
        let object = Handler::alloc().set_ivars(HandlerIvars { sender });
        // The superclass NSObject initializer returns this allocated handler.
        let handler: Retained<Handler> = unsafe { msg_send![super(object), init] };
        // Selector signature is declared above with the two Apple-event descriptors required by AppKit.
        unsafe {
            NSAppleEventManager::sharedAppleEventManager()
                .setEventHandler_andSelector_forEventClass_andEventID(
                    &handler,
                    sel!(handleURL:withReply:),
                    u32::from_be_bytes(*b"GURL"),
                    u32::from_be_bytes(*b"GURL"),
                );
        }
        Self { _handler: handler }
    }
}
impl Drop for AppleLinks {
    fn drop(&mut self) {
        NSAppleEventManager::sharedAppleEventManager().removeEventHandlerForEventClass_andEventID(
            u32::from_be_bytes(*b"GURL"),
            u32::from_be_bytes(*b"GURL"),
        );
    }
}
