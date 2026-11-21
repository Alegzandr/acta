//! Creating a native web view (the chat, ADR 0009; the agent browser, ADR
//! 0010) outside every GPUI update.
//!
//! WebView2 creates its controller while pumping the thread's messages
//! (`webview2_com::wait_with_pump`). Those messages include GPUI's own: its
//! foreground tasks and window events run during the pump. Done inside an
//! update or an event handler, where GPUI holds the app borrowed, the first
//! of them that touches the app (a text field's cursor blink, a timer)
//! panics with "already borrowed". So the only way to a web view is
//! [`build_later`]: the build runs in a foreground task between two
//! updates, with nothing borrowed, and the result comes back in an update
//! of its own. A [`Parent`] exists only there, and [`build`] needs one.

use gpui_kit::{Context, Task, Window};
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

/// The GPUI window a web view is built in, handed to the build of
/// [`build_later`] only.
pub struct Parent(RawWindowHandle);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the handle is the window that owns the task running the
        // build: the task is dropped with the window, so it outlives this.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

/// Build a web view as a child of `parent`.
pub fn build(builder: wry::WebViewBuilder<'_>, parent: &Parent) -> wry::Result<wry::WebView> {
    builder.build_as_child(parent)
}

/// A builder made from the context the web view keeps (the closure form
/// the borrow needs).
pub fn builder<F>(f: F) -> F
where
    F: for<'a> FnOnce(&'a mut wry::WebContext) -> wry::WebViewBuilder<'a>,
{
    f
}

/// Run `make` (which calls [`build`]) after the current update, outside
/// any borrow, then `done` with what it returned, in an update of `T`.
pub fn build_later<T: 'static, R: 'static>(
    window: &Window,
    cx: &mut Context<T>,
    make: impl FnOnce(&Parent) -> wry::Result<R> + 'static,
    done: impl FnOnce(&mut T, wry::Result<R>, &mut Window, &mut Context<T>) + 'static,
) -> Task<()> {
    let parent = HasWindowHandle::window_handle(window).map(|h| Parent(h.as_raw()));
    cx.spawn_in(window, async move |this, cx| {
        // Nothing GPUI is borrowed here: the pump may run anything.
        let built = match parent {
            Ok(parent) => make(&parent),
            Err(err) => Err(err.into()),
        };
        let _ = this.update_in(cx, |this, window, cx| done(this, built, window, cx));
    })
}

#[cfg(test)]
mod tests {
    /// Every web view goes through [`super::build_later`]: nothing else in
    /// the app builds one (a build inside an update crashes the app).
    #[test]
    fn web_views_are_built_only_outside_updates() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![dir];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("src") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a source file");
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                // Spelled in two parts so this file does not match itself.
                let direct = ["WebView::", "build("].concat();
                let child = ["build_as", "_child("].concat();
                // wry's own `WebViewBuilder::build(&window)`.
                let wry = [".build(", "&"].concat();
                assert!(!text.contains(&direct), "{name} builds a web view in place");
                if name != "webview_build.rs" {
                    assert!(!text.contains(&child), "{name} builds a web view in place");
                    assert!(!text.contains(&wry), "{name} builds a web view in place");
                }
                checked += 1;
            }
        }
        assert!(checked > 10);
    }
}
