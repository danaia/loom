use std::{
    ffi::CString,
    sync::{
        OnceLock,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    time::{Duration, Instant},
};

use metal::{MetalDrawable, MetalLayer, foreign_types::ForeignType};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    rc::autoreleasepool,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};

use crate::{RuntimeDiagnostic, RuntimeDiagnosticCode};

pub(crate) struct DisplayUpdate {
    pub drawable: MetalDrawable,
    pub target_timestamp: f64,
    pub target_presentation_timestamp: f64,
}

pub(crate) struct DisplayLinkDriver {
    updates: Receiver<DisplayUpdate>,
    link: *mut Object,
    delegate: *mut Object,
    mode: *mut Object,
    sender_pointer: *mut Sender<DisplayUpdate>,
}

impl DisplayLinkDriver {
    pub fn start(layer: &MetalLayer) -> Result<Self, RuntimeDiagnostic> {
        unsafe { Self::start_on_main_run_loop(layer) }
    }

    pub fn next(&self, timeout: Duration) -> Result<Option<DisplayUpdate>, RuntimeDiagnostic> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.updates.try_recv() {
                Ok(update) => return Ok(Some(update)),
                Err(TryRecvError::Disconnected) => {
                    return Err(RuntimeDiagnostic::new(
                        RuntimeDiagnosticCode::CommandBufferFailed,
                        "CAMetalDisplayLink update stream disconnected",
                    ));
                }
                Err(TryRecvError::Empty) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            autoreleasepool(|| unsafe {
                let run_loop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
                let date: *mut Object = msg_send![
                    class!(NSDate),
                    dateWithTimeIntervalSinceNow: remaining.as_secs_f64()
                ];
                let _: () = msg_send![run_loop, runUntilDate: date];
            });
        }
    }

    pub fn discard_pending(&self) {
        while self.updates.try_recv().is_ok() {}
    }

    unsafe fn start_on_main_run_loop(layer: &MetalLayer) -> Result<Self, RuntimeDiagnostic> {
        let is_main_thread: bool = msg_send![class!(NSThread), isMainThread];
        if !is_main_thread {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::WindowCreationFailed,
                "CAMetalDisplayLink must be created on the main AppKit thread",
            ));
        }
        let Some(link_class) = Class::get("CAMetalDisplayLink") else {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::WindowCreationFailed,
                "CAMetalDisplayLink requires macOS 14 or newer",
            ));
        };
        let (sender, updates) = mpsc::channel();
        let sender_pointer = Box::into_raw(Box::new(sender));
        let delegate_class = display_link_delegate_class();
        let delegate: *mut Object = msg_send![delegate_class, new];
        unsafe {
            (*delegate).set_ivar("pqoUpdateSender", sender_pointer as usize);
        }

        let allocated: *mut Object = msg_send![link_class, alloc];
        let layer_object = layer.as_ref() as *const metal::MetalLayerRef as *mut Object;
        let link: *mut Object = msg_send![allocated, initWithMetalLayer: layer_object];
        if link.is_null() {
            unsafe {
                drop(Box::from_raw(sender_pointer));
            }
            let _: () = msg_send![delegate, release];
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::WindowCreationFailed,
                "CAMetalDisplayLink initialization returned nil",
            ));
        }

        let mode_text = CString::new("kCFRunLoopDefaultMode").expect("static run-loop mode");
        let mode_allocated: *mut Object = msg_send![class!(NSString), alloc];
        let mode: *mut Object = msg_send![mode_allocated, initWithUTF8String: mode_text.as_ptr()];
        let run_loop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
        let _: () = msg_send![link, setDelegate: delegate];
        let _: () = msg_send![link, setPreferredFrameLatency: 1.0_f32];
        let _: () = msg_send![link, addToRunLoop: run_loop forMode: mode];
        let _: () = msg_send![link, setPaused: false];
        Ok(Self {
            updates,
            link,
            delegate,
            mode,
            sender_pointer,
        })
    }
}

impl Drop for DisplayLinkDriver {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.link, setPaused: true];
            let _: () = msg_send![self.link, invalidate];
            let _: () = msg_send![self.link, setDelegate: std::ptr::null_mut::<Object>()];
            let _: () = msg_send![self.mode, release];
            let _: () = msg_send![self.link, release];
            let _: () = msg_send![self.delegate, release];
            drop(Box::from_raw(self.sender_pointer));
        }
    }
}

fn display_link_delegate_class() -> &'static Class {
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut declaration = ClassDecl::new("PqoMetalDisplayLinkDelegate", class!(NSObject))
            .expect("display-link delegate class must be unique");
        declaration.add_ivar::<usize>("pqoUpdateSender");
        unsafe {
            declaration.add_method(
                sel!(metalDisplayLink:needsUpdate:),
                display_link_update as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
            );
        }
        declaration.register()
    })
}

extern "C" fn display_link_update(
    delegate: &Object,
    _selector: Sel,
    _link: *mut Object,
    update: *mut Object,
) {
    unsafe {
        let sender_pointer = *delegate.get_ivar::<usize>("pqoUpdateSender");
        let sender = &*(sender_pointer as *const Sender<DisplayUpdate>);
        let drawable: *mut Object = msg_send![update, drawable];
        let drawable: *mut Object = msg_send![drawable, retain];
        let target_timestamp: f64 = msg_send![update, targetTimestamp];
        let target_presentation_timestamp: f64 = msg_send![update, targetPresentationTimestamp];
        let drawable = MetalDrawable::from_ptr(drawable.cast());
        let _ = sender.send(DisplayUpdate {
            drawable,
            target_timestamp,
            target_presentation_timestamp,
        });
    }
}
