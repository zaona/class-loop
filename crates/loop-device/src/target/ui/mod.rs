//! Stock Manager list prefabs on real firmware pages, with Lyra lifecycle rules.
//! All widget access runs on the page-owner thread; paused pages have no timer.
use super::{
    firmware::*,
    native_app::{self, PAGE_COUNT},
};
use alloc::{format, string::String, vec::Vec};
use core::ffi::c_void;
use loop_core::{
    Route,
    view::{Card, PageView},
};

struct NativeRow {
    object: *mut c_void,
    card: Card,
}
struct Page {
    root: *mut c_void,
    content: *mut c_void,
    title: *mut c_void,
    summary: *mut c_void,
    timer: *mut c_void,
    rows: Vec<NativeRow>,
    view: Option<PageView>,
    revision: Option<u32>,
    pressed: Option<(usize, Card)>,
    active: bool,
    interactive: bool,
}
impl Page {
    const fn new() -> Self {
        Self {
            root: core::ptr::null_mut(),
            content: core::ptr::null_mut(),
            title: core::ptr::null_mut(),
            summary: core::ptr::null_mut(),
            timer: core::ptr::null_mut(),
            rows: Vec::new(),
            view: None,
            revision: None,
            pressed: None,
            active: false,
            interactive: false,
        }
    }
}
static mut PAGES: [Page; PAGE_COUNT] = [const { Page::new() }; PAGE_COUNT];
fn page(index: usize) -> &'static mut Page {
    unsafe { &mut *core::ptr::addr_of_mut!(PAGES).cast::<Page>().add(index) }
}
fn active_page() -> Option<usize> {
    (0..PAGE_COUNT).find(|&i| page(i).active && page(i).interactive)
}
fn c_text(text: &str) -> String {
    format!("{text}\0")
}
fn label_height(text: &str) -> i32 {
    // Same body font/line spacing as the Manager. Reserve every wrapped line.
    let columns = ((CONTENT_WIDTH - 16).max(24) / 12) as usize;
    text.split('\n')
        .map(|line| {
            let units: usize = line.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
            units.max(1).div_ceil(columns) as i32 * 32
        })
        .sum()
}
unsafe fn bind_hits(index: usize, slot: usize) {
    let object = page(index).rows[slot].object;
    let cookie = ((index << 16) | slot) as *mut c_void;
    unsafe { lvx_event_add(object, row_event, EVENT_ALL, cookie) };
    let hits = unsafe { row_hits(object) };
    for (i, &hit) in hits.iter().enumerate() {
        if !hit.is_null() && hit != object && !hits[..i].contains(&hit) {
            unsafe { lvx_event_add(hit, row_event, EVENT_ALL, cookie) };
        }
    }
}
fn render(index: usize) -> i32 {
    if !page(index).active || !page(index).interactive {
        return 0;
    }
    let Some(revision) = super::view_revision(index) else {
        return -1;
    };
    if page(index).revision == Some(revision) {
        return 0;
    }
    let Some(view) = super::page_view(index) else {
        return -1;
    };
    if page(index).view.as_ref() == Some(&view) {
        page(index).revision = Some(revision);
        return 0;
    }
    if page(index).content.is_null() {
        let content = unsafe { lvx_content_create(page(index).root) };
        if content.is_null() {
            return -1;
        }
        page(index).content = content;
        unsafe {
            lvx_object_set_size(content, CONTENT_WIDTH, CONTENT_HEIGHT);
            lvx_object_align(content, ALIGN_TOP_MID, 0, CONTENT_TOP_OFFSET);
            lvx_object_set_content_pad_bottom(content, 32, 0);
        }
    }
    if page(index).title.is_null() {
        let title = c_text(&view.title);
        let callback = if index == 0 {
            core::ptr::null()
        } else {
            title_back as *const ()
        };
        page(index).title = unsafe {
            lvx_page_title_create(
                page(index).root,
                title.as_ptr(),
                u32::from(index != 0),
                callback,
                core::ptr::null_mut(),
            )
        };
        if page(index).title.is_null() {
            return -1;
        }
    }
    if page(index).summary.is_null() {
        page(index).summary = unsafe { lvx_label_create(page(index).content) };
        if page(index).summary.is_null() {
            return -1;
        }
        unsafe {
            lvx_style_apply(
                page(index).summary,
                STYLE_MISANS_REGULAR_24 as *const c_void,
                255,
                0,
            );
            lvx_label_set_long_mode(page(index).summary, 0);
        }
    }
    let summary = c_text(&view.summary);
    unsafe {
        lvx_label_set_text(page(index).summary, summary.as_ptr());
        lvx_object_set_size(
            page(index).summary,
            CONTENT_WIDTH,
            label_height(&view.summary),
        );
        lvx_align_to(
            page(index).summary,
            page(index).content,
            ALIGN_TOP_MID,
            0,
            0,
        );
        lvx_set_hidden(page(index).summary, 0);
    }
    let mut previous = page(index).summary;
    for (slot, card) in view.cards.iter().enumerate() {
        let primary = c_text(&card.title);
        let detail = c_text(&card.detail);
        if slot == page(index).rows.len() {
            let kind = if card.target.is_some() {
                TRAILING_FORWARD
            } else {
                TRAILING_NONE
            };
            let object = unsafe {
                lvx_list_row_create(page(index).content, primary.as_ptr(), detail.as_ptr(), kind)
            };
            if object.is_null() {
                return -1;
            }
            page(index).rows.push(NativeRow {
                object,
                card: card.clone(),
            });
            unsafe { bind_hits(index, slot) };
        }
        let object = page(index).rows[slot].object;
        if page(index).view.is_none() || page(index).rows[slot].card != *card {
            unsafe {
                lvx_list_row_update(
                    object,
                    core::ptr::null(),
                    primary.as_ptr(),
                    detail.as_ptr(),
                    0,
                    1,
                )
            };
        }
        page(index).rows[slot].card = card.clone();
        unsafe {
            // Keep prefab typography and spacing. Long detail fields retain all text.
            lvx_object_set_size(
                object,
                CONTENT_WIDTH,
                96.max(40 + label_height(&card.detail)),
            );
            lvx_align_to(object, previous, ALIGN_OUT_BOTTOM_MID, 0, ROW_GAP);
            lvx_set_hidden(object, 0);
            let trailing = lvx_list_row_trailing(object);
            if !trailing.is_null() {
                lvx_set_hidden(trailing, u32::from(card.target.is_none()));
            }
        }
        previous = object;
    }
    for slot in view.cards.len()..page(index).rows.len() {
        let object = page(index).rows[slot].object;
        unsafe {
            lvx_set_hidden(object, 1);
            let trailing = lvx_list_row_trailing(object);
            if !trailing.is_null() {
                lvx_set_hidden(trailing, 1);
            }
        }
    }
    page(index).view = Some(view);
    page(index).revision = Some(revision);
    0
}
extern "C" fn refresh_timer(timer: *mut c_void) {
    let Some(index) = active_page() else {
        return;
    };
    if page(index).timer != timer {
        return;
    }
    super::ui_maintenance_tick();
    let _ = render(index);
}
fn start_timer(index: usize) -> i32 {
    if page(index).timer.is_null() {
        page(index).timer = unsafe { lvx_timer_create(refresh_timer, 100, core::ptr::null_mut()) };
    }
    if page(index).timer.is_null() { -1 } else { 0 }
}
pub fn page_create(index: usize, root: *mut c_void) -> i32 {
    if index >= PAGE_COUNT || root.is_null() {
        return -1;
    }
    *page(index) = Page::new();
    page(index).root = root;
    page(index).active = true;
    page(index).interactive = true;
    super::sync_resumed_page(index);
    super::ui_maintenance_tick();
    let rc = render(index);
    if rc != 0 {
        return rc;
    }
    start_timer(index)
}
pub fn page_resume(index: usize) -> i32 {
    if index >= PAGE_COUNT || !page(index).active {
        return -1;
    }
    page(index).interactive = true;
    super::sync_resumed_page(index);
    super::ui_maintenance_tick();
    let rc = render(index);
    if rc != 0 {
        return rc;
    }
    start_timer(index)
}
pub fn page_pause(index: usize) -> i32 {
    if index >= PAGE_COUNT {
        return -1;
    }
    page(index).interactive = false;
    page(index).pressed = None;
    let timer = core::mem::replace(&mut page(index).timer, core::ptr::null_mut());
    if !timer.is_null() {
        unsafe { lvx_timer_delete(timer) };
    }
    0
}
pub fn page_destroy(index: usize) -> i32 {
    if index >= PAGE_COUNT {
        return -1;
    }
    page_pause(index);
    // Firmware owns and destroys the root and its children.
    *page(index) = Page::new();
    0
}
pub fn navigate(index: usize) -> i32 {
    if let Some(source) = active_page() {
        page_pause(source);
    }
    let key = (native_app::app_id() << 16) | index as u32;
    unsafe { activity_navigate(key, 0, 0, 0) }
}
pub fn back(index: usize) -> i32 {
    page_pause(index);
    let argument = if BAND_11 {
        ((native_app::app_id() << 16) | index as u32) as usize as *mut firmware_page_descriptor
    } else {
        native_app::live_page(index)
    };
    if !argument.is_null() {
        unsafe { activity_finish(argument) }
    } else {
        -1
    }
}
extern "C" fn title_back(_: *mut c_void) {
    if let Some(index) = active_page().filter(|&i| i != Route::Home.page_index()) {
        super::handle_back(index);
    }
}
extern "C" fn row_event(event: *mut c_void) {
    if event.is_null() {
        return;
    }
    let cookie = unsafe { lvx_event_get_user_data(event) };
    let index = cookie >> 16;
    let slot = cookie & 0xffff;
    if index >= PAGE_COUNT || !page(index).active || !page(index).interactive {
        return;
    }
    match unsafe { lvx_event_get_code(event) } {
        EVENT_PRESSED => {
            page(index).pressed = page(index)
                .view
                .as_ref()
                .and_then(|v| v.cards.get(slot))
                .cloned()
                .map(|card| (slot, card));
            return;
        }
        EVENT_PRESS_LOST => {
            if page(index)
                .pressed
                .as_ref()
                .is_some_and(|(s, _)| *s == slot)
            {
                page(index).pressed = None;
            }
            return;
        }
        EVENT_CLICKED => {}
        _ => return,
    }
    let Some((pressed_slot, card)) = page(index).pressed.take() else {
        return;
    };
    if pressed_slot != slot {
        return;
    }
    let Some(target) = card.target else {
        return;
    };
    // Compare against PRESS, even if the timer already rendered an import.
    super::ui_maintenance_tick();
    let Some(current) = super::page_view(index) else {
        return;
    };
    if current.cards.get(slot) != Some(&card) {
        let _ = render(index);
        return;
    }
    let Some(revision) = super::view_revision(index) else {
        return;
    };
    super::handle_action(index, revision, target.action());
    if page(index).interactive {
        let _ = render(index);
    }
}
