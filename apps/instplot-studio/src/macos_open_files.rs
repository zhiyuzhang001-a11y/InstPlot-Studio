//! Bridges macOS Launch Services document-open events into the egui update loop.
//!
//! Winit owns the `NSApplication` delegate and relies on its exact class, so
//! replacing that delegate would break the event loop. Instead, this module
//! adds the optional AppKit document-open selector to Winit's registered
//! delegate class before the native event loop begins running.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui;
use objc2::ffi;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;
use objc2_foundation::{NSArray, NSURL};

struct OpenHandler {
    sender: Sender<Vec<PathBuf>>,
    egui_context: Mutex<Option<egui::Context>>,
}

static OPEN_HANDLER: OnceLock<OpenHandler> = OnceLock::new();

unsafe extern "C-unwind" fn application_open_urls(
    _delegate: &AnyObject,
    _selector: Sel,
    _application: &AnyObject,
    urls: &NSArray<NSURL>,
) {
    let paths = urls
        .iter()
        .filter_map(|url| url.to_file_path())
        .collect::<Vec<_>>();
    let Some(handler) = OPEN_HANDLER.get() else {
        return;
    };
    if !paths.is_empty()
        && handler.sender.send(paths).is_ok()
        && let Some(context) = handler
            .egui_context
            .lock()
            .expect("the document-open context lock is not poisoned")
            .as_ref()
    {
        context.request_repaint();
    }
}

pub(super) struct MacOpenFiles {
    receiver: Receiver<Vec<PathBuf>>,
    registration: Receiver<Result<(), &'static str>>,
}

impl MacOpenFiles {
    pub(super) fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        assert!(
            OPEN_HANDLER
                .set(OpenHandler {
                    sender,
                    egui_context: Mutex::new(None),
                })
                .is_ok(),
            "the macOS document-open bridge is installed exactly once"
        );

        let (registration_sender, registration) = mpsc::channel();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(class) = AnyClass::get(c"WinitApplicationDelegate") {
                    let selector = sel!(application:openURLs:);
                    // SAFETY: The selector is an optional
                    // NSApplicationDelegate method. The callback has
                    // Objective-C's `void self, _cmd, object, object` ABI,
                    // and the class remains registered for the process
                    // lifetime.
                    let added = unsafe {
                        let implementation: Imp = std::mem::transmute(
                            application_open_urls
                                as unsafe extern "C-unwind" fn(
                                    &AnyObject,
                                    Sel,
                                    &AnyObject,
                                    &NSArray<NSURL>,
                                ),
                        );
                        ffi::class_addMethod(
                            std::ptr::from_ref(class).cast_mut(),
                            selector,
                            implementation,
                            c"v@:@@".as_ptr(),
                        )
                    };
                    let result = if added.as_bool() {
                        Ok(())
                    } else {
                        Err("winit's delegate already handles document URLs")
                    };
                    let _ = registration_sender.send(result);
                    return;
                }
                if Instant::now() >= deadline {
                    let _ = registration_sender.send(Err("winit delegate registration timed out"));
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        Self {
            receiver,
            registration,
        }
    }

    pub(super) fn finish_install(&self, egui_context: egui::Context) {
        let registration = self
            .registration
            .recv_timeout(Duration::from_secs(5))
            .expect("the macOS document-open bridge registration thread stopped");
        assert!(registration.is_ok(), "{}", registration.unwrap_err());
        *OPEN_HANDLER
            .get()
            .expect("the macOS document-open handler exists")
            .egui_context
            .lock()
            .expect("the document-open context lock is not poisoned") = Some(egui_context);
    }

    pub(super) fn drain(&self) -> Vec<PathBuf> {
        self.receiver
            .try_iter()
            .flat_map(|paths| paths.into_iter())
            .collect()
    }
}
