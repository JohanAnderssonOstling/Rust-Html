use std::fmt::Alignment;
use std::rc::Rc;
use floem::event::EventPropagation;
use floem::prelude::{button, container, create_rw_signal, Decorators, dyn_view, h_stack, label, SignalGet, SignalUpdate, v_stack};
use floem::{dyn_view, IntoView, View};
use floem::style::{AlignContent, AlignItems, JustifyContent, TextOverflow};
use floem::views::{ScrollExt, stack_from_iter};
use crate::library::components::label_style;

#[derive(Clone)]
pub struct TocEntry {
    pub title: String,
    pub link: String, // When Some(link), clicking the title navigates.
    pub children: Vec<TocEntry>,
}

pub fn toc_view(entries: Vec<TocEntry>, on_toc_click: Rc<dyn Fn(String)>, level: i32) -> impl View {
    stack_from_iter(entries.into_iter()
        .map(|entry| hierarchical_toc_entry(entry, on_toc_click.clone(), level))
    ).style(|s| s.flex_col().height_full().flex_grow(1.0))

}

pub fn hierarchical_toc_entry(entry: TocEntry, on_toc_click: Rc<dyn Fn(String)>, level: i32) -> impl View {
    let collapsed = create_rw_signal(true);

    // ── arrow ───────────────────────────────────────────────
    let arrow_button = if !entry.children.is_empty() {
        label(move || if collapsed.get() { "▶" } else { "▼" }.to_string())
            .on_click({
                move |_| {
                    collapsed.update(|c| *c = !*c);
                    EventPropagation::Stop
                }
            })
            .style(|s| s.width(20).height(40).padding(0).align_content(AlignContent::Center)
                .justify_center()
                .items_center()
                
            )
    } else {
        label(move || "").style(|s| s.width(0).height(0).padding(0))
    };
    let arrow_button = label_style(arrow_button, 12);

    // ── title ───────────────────────────────────────────────
    let title_widget = label(move || entry.title.clone())
        .on_click({
            let link = entry.link.clone();
            let on_toc_click = on_toc_click.clone();
            move |_| {
                on_toc_click(link.clone());
                EventPropagation::Stop
            }
        });

    let title_widget = label_style(title_widget, 16)
        .style(move |s| s
            .padding(10)
            //.flex_grow(1.0)     // fill leftover space
            .width(260 - level * 30)

            //.flex_shrink(1.0)    // shrink when needed
            .min_width(0)        // critical for wrapping
            //.max_width_full()
            .text_overflow(TextOverflow::Wrap)
        );

    // ── header row: arrow then title ────────────────────────
    let header = h_stack((  arrow_button, title_widget))
        .style(|s| s.min_width(0));

    // ── children container (unchanged) ─────────────────────
    let children = dyn_view(move ||
    if collapsed.get() {
        container(label(|| "").style(|s| s.width(0).height(0)))
    } else {
        container(toc_view(entry.children.clone(), on_toc_click.clone(), level + 1))
            .style(|s| s)
    }
    );

    v_stack((header,children))
        .style(|s| s.padding_left(30))
}