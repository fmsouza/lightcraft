//! The phone's "…" as the system's own menu (iOS: a UIMenu, as Lightroom and the system apps have
//! it): the host puts a native button over the "…" icon ([`crate::Services::menu_button`]) and
//! opens its menu itself, with sections, symbols, check marks and submenus. What is
//! picked comes back through [`pick`] and runs on the next frame. A host without one keeps the
//! drawn sheet ([`crate::panels::more_sheet`]).

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::menubar::MenuNode;

/// One row of the menu, or a group of them.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeItem {
    /// A command: `symbol` is the system's symbol name (SF Symbols on iOS; empty: none).
    Action { id: String, params: Value, label: String, symbol: &'static str, enabled: bool, checked: bool, destructive: bool },
    /// Rows shown together under `title` (empty: a group without a title), between lines.
    Section { title: String, items: Vec<NativeItem> },
    /// A row that opens `items` as the next menu.
    Submenu { label: String, symbol: &'static str, items: Vec<NativeItem> },
}

/// Where "…" is this frame (egui points) and its menu; `None`: no "…" on screen, or something
/// is over it (a dialog, a sheet), so the native button steps aside.
pub type MenuRequest = Option<(egui::Rect, Arc<Vec<NativeItem>>)>;

/// Picks the host's menu made, waiting for the next frame.
static PICKS: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

/// The host's menu ran `id` with `params` (call `request_repaint` after it).
pub fn pick(id: &str, params: Value) {
    PICKS.lock().unwrap_or_else(PoisonError::into_inner).push((id.to_string(), params));
}

/// Where the compact bar drew "…" this frame, and whether it is the editor's.
#[derive(Clone, Copy, Debug)]
struct Placed {
    rect: egui::Rect,
    editor: bool,
}

fn placed_id() -> egui::Id {
    egui::Id::unique("native-more")
}

/// The compact bar drew "…" at `rect`: the native button goes there at the end of the frame.
pub(crate) fn place(ctx: &egui::Context, rect: egui::Rect, editor: bool) {
    ctx.data_mut(|d| d.insert_temp(placed_id(), Placed { rect, editor }));
}

/// Run what the host's menu picked since the last frame.
pub(crate) fn run_picks(app: &mut LightcraftApp, ctx: &egui::Context) {
    let picks = std::mem::take(&mut *PICKS.lock().unwrap_or_else(PoisonError::into_inner));
    for (id, params) in picks {
        if let Err(e) = crate::menubar::run_item(app, &id, params) {
            app.toast(ctx, e);
        }
    }
}

/// End of frame: tell the host where "…" is and what its menu holds (rebuilt at most every
/// 0.3 s: the menus change with the screen, never faster than a person opens one).
pub(crate) fn sync(app: &mut LightcraftApp, ctx: &egui::Context) {
    if app.services.menu_button.is_none() {
        return;
    }
    let placed = ctx.data_mut(|d| {
        let p = d.get_temp::<Placed>(placed_id());
        d.remove::<Placed>(placed_id());
        p
    });
    let covered = app.ui.dialog.is_some() || ctx.memory(|m| m.top_modal_layer().is_some()) || egui::Popup::is_any_open(ctx);
    let request = match placed {
        Some(p) if !covered => {
            let now = ctx.input(|i| i.time);
            let cache = egui::Id::unique("native-more-items");
            let items = match ctx.data(|d| d.get_temp::<(f64, bool, Arc<Vec<NativeItem>>)>(cache)) {
                Some((at, editor, items)) if editor == p.editor && now - at < 0.3 => items,
                _ => {
                    let items = Arc::new(more_items(app, p.editor));
                    ctx.data_mut(|d| d.insert_temp(cache, (now, p.editor, items.clone())));
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                    items
                }
            };
            Some((p.rect, items))
        }
        _ => None,
    };
    if let Some(f) = app.services.menu_button.as_mut() {
        f(request);
    }
}

/// What "…" holds: the screen's own commands in sections, as a phone photo app has them (the
/// library's selection, import, view options and review; the editor's settings, photo and view).
/// The host's menu and the drawn sheet ([`crate::panels::more_sheet`]) both show this list.
pub fn more_items(app: &LightcraftApp, editor: bool) -> Vec<NativeItem> {
    let bar = crate::menubar::menu_bar(app);
    // a command as the menus have it now (enabled, checked), or enabled when no menu lists it
    let state = |id: &str| find(&bar, id).unwrap_or((true, false));
    let action = |id: &str, label: &str, symbol: &'static str| {
        let (enabled, checked) = state(id);
        NativeItem::Action { id: id.into(), params: json!({}), label: crate::i18n::tr(label).into(), symbol, enabled, checked, destructive: false }
    };
    let section = |title: &str, items: Vec<NativeItem>| NativeItem::Section { title: crate::i18n::tr(title).into(), items };
    // a desktop submenu's items (with their parameters) as a submenu of "…"; none when it is empty
    let submenu = |menu: &str, name: &str, symbol: &'static str| {
        let items = sub_nodes(&bar, menu, name).map(nodes_items).unwrap_or_default();
        (!items.is_empty()).then(|| NativeItem::Submenu { label: crate::i18n::tr(name).into(), symbol, items })
    };
    if editor {
        let photo = app.session.active().is_some();
        let mut delete = destructive(action("photo.delete", "Delete Photo", "trash"));
        if let NativeItem::Action { enabled, .. } = &mut delete {
            *enabled &= photo;
        }
        use crate::state::BeforeAfter as B;
        let before_after = [
            ("view.beforeAfter", "Side by Side", "rectangle.split.2x1", B::SideBySide),
            ("view.beforeAfterSplit", "Split", "square.split.2x1", B::Split),
            ("view.beforeAfterTopBottom", "Top/Bottom", "rectangle.split.1x2", B::TopBottom),
            ("view.beforeAfterSplitTopBottom", "Split Top/Bottom", "square.split.1x2", B::SplitTopBottom),
        ]
        .into_iter()
        .map(|(id, label, symbol, mode)| {
            let mut a = action(id, label, symbol);
            // (the menus check only some of them)
            if let NativeItem::Action { checked, .. } = &mut a {
                *checked = app.ui.before_after == mode;
            }
            a
        })
        .collect();
        let mut photo_items = vec![
            action("photo.rotateLeft", "Rotate Left", "rotate.left"),
            action("photo.rotateRight", "Rotate Right", "rotate.right"),
            action("photo.virtualCopy", "Create Virtual Copy", "plus.square.on.square"),
        ];
        photo_items.extend(submenu("Photo", "Add to Album", "rectangle.stack.badge.plus"));
        photo_items.push(action("library.findSimilar", "Find Similar Photos", "magnifyingglass"));
        photo_items.push(action("dialog.rename", "Rename…", "pencil"));
        vec![
            section(
                "Edits",
                vec![
                    action("develop.copy", "Copy Settings", "doc.on.doc"),
                    action("dialog.copySettings", "Choose Settings to Copy…", "checklist"),
                    action("develop.paste", "Paste Settings", "doc.on.clipboard"),
                    action("dialog.pasteSettings", "Paste Selected Settings…", "list.clipboard"),
                    action("develop.pastePrevious", "Paste from Previous", "clock.arrow.circlepath"),
                    action("develop.reset", "Reset Edits", "arrow.counterclockwise"),
                ],
            ),
            section("Photo", photo_items),
            section(
                "View",
                vec![
                    action("view.histogram", "Histogram", "chart.bar"),
                    action("view.infoOverlay", "Info Overlay", "info.circle"),
                    NativeItem::Submenu { label: crate::i18n::tr("Before/After").into(), symbol: "rectangle.split.2x1", items: before_after },
                    action("view.focusPoint", "Focus Point", "scope"),
                    action("view.faceBoxes", "Face Boxes", "face.smiling"),
                    action("view.viewOptions", "View Options…", "slider.horizontal.3"),
                ],
            ),
            section("", vec![delete]),
        ]
    } else {
        let mut view =
            vec![action("view.photoGrid", "Photo Grid", "rectangle.grid.2x2"), action("view.squareGrid", "Square Grid", "square.grid.2x2")];
        view.extend(submenu("View", "Grid Info", "text.below.photo"));
        view.push(action("view.people", "People", "person.2"));
        view.push(action("view.slideshow", "Start Slideshow", "play.fill"));
        let mut items = vec![
            section(
                "Select",
                vec![action("view.selectMode", "Select", "checkmark.circle"), action("library.selectAll", "Select All", "checkmark.circle.fill")],
            ),
            section(
                "Add photos",
                vec![action("file.addPhotos", "From Photos", "photo.on.rectangle"), action("file.addFolder", "From Files…", "folder.badge.plus")],
            ),
            section("View options", view),
            section("Review", vec![action("dialog.cull", "Assisted Culling…", "sparkles")]),
        ];
        // only where the menus have it: while Recently Deleted is on screen
        if find(&bar, "library.emptyRecentlyDeleted").is_some() {
            items.push(section("", vec![destructive(action("library.emptyRecentlyDeleted", "Empty Recently Deleted", "trash"))]));
        }
        // (the library has no Undo of its own on a phone)
        items.push(section("", vec![action("edit.undo", "Undo", "arrow.uturn.backward"), action("edit.redo", "Redo", "arrow.uturn.forward")]));
        items.push(section("", vec![action("app.settings", "App Settings", "gearshape"), action("app.about", "About LightCraft", "info.circle")]));
        items
    }
}

/// `items` as JSON for a host that builds its menu in another language (Android's Java):
/// `{"t":"a", id, params (JSON text), label, enabled, checked, destructive}`, `{"t":"s", title,
/// items}` for a section, `{"t":"m", label, items}` for a submenu.
pub fn to_json(items: &[NativeItem]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|i| match i {
                NativeItem::Action { id, params, label, enabled, checked, destructive, .. } => json!({
                    "t": "a", "id": id, "params": params.to_string(), "label": label,
                    "enabled": enabled, "checked": checked, "destructive": destructive,
                }),
                NativeItem::Section { title, items } => json!({"t": "s", "title": title, "items": to_json(items)}),
                NativeItem::Submenu { label, items, .. } => json!({"t": "m", "label": label, "items": to_json(items)}),
            })
            .collect(),
    )
}

/// `item` marked as destructive (the system draws it red).
fn destructive(mut item: NativeItem) -> NativeItem {
    if let NativeItem::Action { destructive, .. } = &mut item {
        *destructive = true;
    }
    item
}

/// The children of the submenu `name` in the menu titled `menu`.
fn sub_nodes<'a>(bar: &'a [(String, Vec<MenuNode>)], menu: &str, name: &str) -> Option<&'a [MenuNode]> {
    bar.iter().find(|(m, _)| m == menu)?.1.iter().find_map(|n| match n {
        MenuNode::Submenu { label, children } if label == name => Some(children.as_slice()),
        _ => None,
    })
}

/// A desktop menu's items: runs between separators become sections without a title.
fn nodes_items(nodes: &[MenuNode]) -> Vec<NativeItem> {
    let mut groups: Vec<Vec<NativeItem>> = vec![Vec::new()];
    for n in nodes {
        match n {
            MenuNode::Separator => {
                if groups.last().is_some_and(|g| !g.is_empty()) {
                    groups.push(Vec::new());
                }
            }
            MenuNode::Submenu { label, children } => {
                if let Some(g) = groups.last_mut() {
                    g.push(NativeItem::Submenu { label: crate::i18n::tr(label).into(), symbol: "", items: nodes_items(children) });
                }
            }
            MenuNode::Item { id, params, label, enabled, checked, .. } => {
                if let Some(g) = groups.last_mut() {
                    g.push(NativeItem::Action {
                        id: id.clone(),
                        params: params.clone(),
                        label: crate::menubar::display_item_label(id, params, label).to_string(),
                        symbol: "",
                        enabled: *enabled,
                        checked: *checked == Some(true),
                        destructive: false,
                    });
                }
            }
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.len() == 1 {
        return groups.pop().unwrap_or_default();
    }
    groups.into_iter().map(|items| NativeItem::Section { title: String::new(), items }).collect()
}

/// `id`'s (enabled, checked) in the menus.
fn find(bar: &[(String, Vec<MenuNode>)], id: &str) -> Option<(bool, bool)> {
    fn walk(nodes: &[MenuNode], id: &str) -> Option<(bool, bool)> {
        nodes.iter().find_map(|n| match n {
            MenuNode::Item { id: i, enabled, checked, .. } if i == id => Some((*enabled, *checked == Some(true))),
            MenuNode::Submenu { children, .. } => walk(children, id),
            _ => None,
        })
    }
    bar.iter().find_map(|(_, nodes)| walk(nodes, id))
}
