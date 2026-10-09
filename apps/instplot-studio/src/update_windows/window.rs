//! Native root-window observation. Rendering alone is not visible GUI health.
use std::io;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible,
};

#[derive(Clone, Copy, Debug)]
struct WindowObservation {
    visible: bool,
    minimized: bool,
    cloaked: bool,
    positive_area: bool,
    on_monitor: bool,
}

impl WindowObservation {
    fn ready(self) -> bool {
        self.visible && !self.minimized && !self.cloaked && self.positive_area && self.on_monitor
    }
}

pub(super) fn visible_owned_root(raw: isize, process_id: u32) -> io::Result<bool> {
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    let mut owner = 0;
    let mut rect = RECT::default();
    let mut cloaked = 0u32;
    // The handle comes from eframe's actual root window, not window-title lookup.
    // Recheck native identity on every observation; handle reuse cannot authorize
    // another process or a child/dialog window.
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool()
            || GetWindowThreadProcessId(hwnd, Some(&mut owner)) == 0
            || owner != process_id
            || GetAncestor(hwnd, GA_ROOT) != hwnd
        {
            return Err(super::invalid("GUI root window identity differs"));
        }
        GetWindowRect(hwnd, &mut rect).map_err(super::invalid)?;
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
        .map_err(super::invalid)?;
        let observation = WindowObservation {
            visible: IsWindowVisible(hwnd).as_bool(),
            minimized: IsIconic(hwnd).as_bool(),
            cloaked: cloaked != 0,
            positive_area: rect.right > rect.left && rect.bottom > rect.top,
            on_monitor: !MonitorFromRect(&rect, MONITOR_DEFAULTTONULL).0.is_null(),
        };
        Ok(observation.ready())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_minimized_cloaked_offscreen_or_empty_is_not_gui_health() {
        let ready = WindowObservation {
            visible: true,
            minimized: false,
            cloaked: false,
            positive_area: true,
            on_monitor: true,
        };
        assert!(ready.ready());
        for rejected in [
            WindowObservation {
                visible: false,
                ..ready
            },
            WindowObservation {
                minimized: true,
                ..ready
            },
            WindowObservation {
                cloaked: true,
                ..ready
            },
            WindowObservation {
                positive_area: false,
                ..ready
            },
            WindowObservation {
                on_monitor: false,
                ..ready
            },
        ] {
            assert!(!rejected.ready());
        }
    }

    #[test]
    fn nonexistent_native_window_is_rejected() {
        assert!(visible_owned_root(0, std::process::id()).is_err());
    }

    #[test]
    fn real_owned_root_must_be_visible_and_rejects_child_foreign_and_destroyed_handles() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GetDesktopWindow, SW_HIDE, SW_MINIMIZE, SW_RESTORE,
            SW_SHOW, ShowWindow, WINDOW_EX_STYLE, WS_CHILD, WS_OVERLAPPEDWINDOW,
        };
        use windows::core::w;
        struct OwnedWindow(HWND);
        impl Drop for OwnedWindow {
            fn drop(&mut self) {
                let _ = unsafe { DestroyWindow(self.0) };
            }
        }
        let pid = std::process::id();
        let root = OwnedWindow(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Studio native health fixture"),
                WS_OVERLAPPEDWINDOW,
                20,
                20,
                400,
                300,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        });
        let raw = root.0.0 as isize;
        assert!(!visible_owned_root(raw, pid).unwrap());
        unsafe {
            let _ = ShowWindow(root.0, SW_SHOW);
        }
        assert!(visible_owned_root(raw, pid).unwrap());
        assert!(visible_owned_root(raw, pid.wrapping_add(1)).is_err());
        let child = OwnedWindow(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Child"),
                WS_CHILD,
                5,
                5,
                100,
                100,
                Some(root.0),
                None,
                None,
                None,
            )
            .unwrap()
        });
        assert!(visible_owned_root(child.0.0 as isize, pid).is_err());
        assert!(visible_owned_root(unsafe { GetDesktopWindow() }.0 as isize, pid).is_err());
        unsafe {
            let _ = ShowWindow(root.0, SW_MINIMIZE);
        }
        assert!(!visible_owned_root(raw, pid).unwrap());
        unsafe {
            let _ = ShowWindow(root.0, SW_RESTORE);
        }
        assert!(visible_owned_root(raw, pid).unwrap());
        unsafe {
            let _ = ShowWindow(root.0, SW_HIDE);
        }
        assert!(!visible_owned_root(raw, pid).unwrap());
        drop(child);
        drop(root);
        assert!(visible_owned_root(raw, pid).is_err());
    }
}
