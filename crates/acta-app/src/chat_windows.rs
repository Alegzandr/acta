//! WebView2 events that wry does not expose, for the chat panel (ADR 0009):
//! the keys Acta keeps while the page has the keyboard
//! (`ICoreWebView2Controller::AcceleratorKeyPressed`), the page taking
//! focus, and a failed top-level navigation. The handlers only look at the
//! key and at the navigation's outcome; nothing reads from the page.
//!
//! WebView2 raises these on the UI thread, outside any GPUI context, so
//! they post a [`Signal`] for the panel to act on.

use async_channel::Sender;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_PHYSICAL_KEY_STATUS,
    COREWEBVIEW2_WEB_ERROR_STATUS, ICoreWebView2, ICoreWebView2Controller,
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, FocusChangedEventHandler, NavigationCompletedEventHandler,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use wry::WebViewExtWindows as _;

use crate::chat::{self, Mods};
use crate::chat_panel::Signal;

/// The registered handlers; dropping this removes them.
pub struct Hooks {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    keys: i64,
    focus: i64,
    navigation: i64,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        // SAFETY: the tokens were returned by the matching `add_` calls on
        // these same objects, which this struct keeps alive.
        unsafe {
            let _ = self.controller.remove_AcceleratorKeyPressed(self.keys);
            let _ = self.controller.remove_GotFocus(self.focus);
            let _ = self.webview.remove_NavigationCompleted(self.navigation);
        }
    }
}

fn down(key: u16) -> bool {
    // SAFETY: a plain query of the keyboard state.
    (unsafe { GetKeyState(i32::from(key)) } as u16 & 0x8000) != 0
}

pub fn install(web: &wry::WebView, signals: Sender<Signal>) -> windows::core::Result<Hooks> {
    let controller = web.controller();
    let webview = web.webview();
    let (mut keys, mut focus, mut navigation) = (0i64, 0i64, 0i64);

    let to_keys = signals.clone();
    let key_handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
        let mut virtual_key = 0u32;
        let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
        // SAFETY: out-parameters of the event's own arguments.
        unsafe {
            args.KeyEventKind(&mut kind)?;
            args.VirtualKey(&mut virtual_key)?;
            args.PhysicalKeyStatus(&mut status)?;
        }
        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
            && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
        {
            return Ok(());
        }
        let mods = Mods {
            ctrl: down(VK_CONTROL),
            shift: down(VK_SHIFT),
            alt: down(VK_MENU),
            cmd: down(VK_LWIN) || down(VK_RWIN),
        };
        let key = chat::windows_key(virtual_key, status.ScanCode);
        if let Some(kept) = chat::kept(key, mods) {
            // SAFETY: as above.
            unsafe { args.SetHandled(true)? };
            // Auto-repeat is swallowed, not repeated.
            if !status.WasKeyDown.as_bool() {
                let _ = to_keys.try_send(Signal::Key(kept));
            }
        }
        Ok(())
    }));

    let to_focus = signals.clone();
    let focus_handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
        let _ = to_focus.try_send(Signal::Focused);
        Ok(())
    }));

    let navigation_handler = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut success = windows::core::BOOL::default();
        let mut error = COREWEBVIEW2_WEB_ERROR_STATUS::default();
        // SAFETY: out-parameters of the event's own arguments.
        unsafe {
            args.IsSuccess(&mut success)?;
            if !success.as_bool() {
                args.WebErrorStatus(&mut error)?;
            }
        }
        if !success.as_bool()
            && let Some(reason) = chat::failure_of(error.0)
        {
            let _ = signals.try_send(Signal::LoadFailed(reason));
        }
        Ok(())
    }));

    // SAFETY: COM calls on live objects owned by the wry web view; the
    // tokens are kept to remove the handlers in `Drop`.
    unsafe {
        controller.add_AcceleratorKeyPressed(&key_handler, &mut keys)?;
        controller.add_GotFocus(&focus_handler, &mut focus)?;
        webview.add_NavigationCompleted(&navigation_handler, &mut navigation)?;
    }
    Ok(Hooks {
        controller,
        webview,
        keys,
        focus,
        navigation,
    })
}

#[cfg(test)]
mod tests {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;

    use crate::chat::{Reason, failure_of};

    #[test]
    fn the_error_numbers_match_webview2() {
        assert_eq!(
            failure_of(COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED.0),
            Some(Reason::Offline)
        );
        assert_eq!(
            failure_of(COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED.0),
            Some(Reason::Offline)
        );
        for status in [
            COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT,
            COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT,
            COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE,
            COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET,
            COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID,
            COREWEBVIEW2_WEB_ERROR_STATUS_UNEXPECTED_ERROR,
        ] {
            assert_eq!(failure_of(status.0), Some(Reason::Unknown), "{status:?}");
        }
        for status in [
            COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED,
            COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN,
            COREWEBVIEW2_WEB_ERROR_STATUS_REDIRECT_FAILED,
        ] {
            assert_eq!(failure_of(status.0), None, "{status:?}");
        }
    }
}
