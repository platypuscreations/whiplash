//! AppKit's open and save panels, shown as free-standing windows.

use std::cell::Cell;
use std::path::PathBuf;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{NSArray, NSString, NSURL};

use super::{Pick, Spec};

/// Shows the panel and returns at once; `done` gets the chosen paths (none
/// if cancelled) when the panel closes. Must be called on the main thread.
pub fn show(spec: &Spec, done: impl FnOnce(Vec<PathBuf>) + 'static) {
    let mtm = MainThreadMarker::new().expect("dialogs are opened on the main thread");
    let panel: Retained<NSSavePanel> = match spec.pick {
        Pick::Save => NSSavePanel::savePanel(mtm),
        pick => {
            let open = NSOpenPanel::openPanel(mtm);
            open.setCanChooseFiles(pick != Pick::Folder);
            open.setCanChooseDirectories(pick == Pick::Folder);
            open.setAllowsMultipleSelection(pick == Pick::Files);
            Retained::into_super(open)
        }
    };
    let title = NSString::from_str(spec.title);
    panel.setTitle(Some(&title));
    panel.setMessage(Some(&title));
    if let Some((_, extensions)) = spec.filter {
        let types: Vec<_> = extensions.iter().map(|e| NSString::from_str(e)).collect();
        // The replacement (content types) needs a type for every extension;
        // extensions are what we have, and this still works.
        #[allow(deprecated)]
        panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&types)));
    }
    if let Some(name) = spec.file_name {
        panel.setNameFieldStringValue(&NSString::from_str(name));
    }

    // The handler runs once; it takes what it needs, which also lets the
    // panel go once it has closed.
    let state = Cell::new(Some((panel.clone(), done)));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        let Some((panel, done)) = state.take() else { return };
        let paths = if response == NSModalResponseOK { chosen(&panel) } else { Vec::new() };
        done(paths);
    });
    panel.beginWithCompletionHandler(&handler);
}

/// The paths chosen in a panel that closed with OK.
fn chosen(panel: &NSSavePanel) -> Vec<PathBuf> {
    let path = |url: &NSURL| url.path().map(|p| PathBuf::from(p.to_string()));
    match panel.downcast_ref::<NSOpenPanel>() {
        Some(open) => open.URLs().iter().filter_map(|url| path(&url)).collect(),
        None => panel.URL().and_then(|url| path(&url)).into_iter().collect(),
    }
}
