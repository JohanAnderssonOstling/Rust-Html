use std::rc::Rc;
use floem::event::EventPropagation;
use floem::prelude::{button, container, create_rw_signal, Decorators, dyn_view, h_stack, label, SignalGet, SignalUpdate, v_stack};
use floem::{dyn_view, IntoView, View};
use floem::views::stack_from_iter;
use crate::library::components::label_style;

#[derive(Clone)]
pub struct TocEntry {
    pub title: String,
    pub link: String, // When Some(link), clicking the title navigates.
    pub children: Vec<TocEntry>,
}

pub fn toc_view(entries: Vec<TocEntry>, on_toc_click: Rc<dyn Fn(String)>) -> impl View {
    container(stack_from_iter(entries.into_iter()
        .map(|entry| hierarchical_toc_entry(entry, on_toc_click.clone()))).style(|s| s.flex_col().width(200))
    ).style(move |s| s.max_width_pct(100.))
}

pub fn hierarchical_toc_entry(entry: TocEntry, on_toc_click: Rc<dyn Fn(String)>) -> impl View {
    let collapsed = create_rw_signal(true);
    let mut arrow_button = if !entry.children.is_empty() {
        label (move || if collapsed.get() { ">" } else { "v" }.to_string())
            .on_click({ move |_| {
                    collapsed.update(|c| *c = !*c);
                    EventPropagation::Stop
                }
            })
            .style(|s| s.width(20).height(20).padding_right(4))
    } else {
        label(move || "").style(|s| s.width(20).height(20).margin_right(4))
    };
    arrow_button = label_style(arrow_button, 16);


    // Build the title widget. If a link exists, make it clickable.
    let mut title_widget = label(move || entry.title.clone())
            .on_click({
                let link = entry.link.clone();
                let on_toc_click = on_toc_click.clone();
                move |_| {
                    on_toc_click(link.clone());
                    EventPropagation::Stop
                }
            })
            .style(|s| s.padding(4).text_ellipsis().width_full());
    title_widget = label_style(title_widget, 16);


    // The header row combining the arrow button and title.
    let header = h_stack((arrow_button, title_widget)).style(|s| s.flex_row().width_full());

    // Build a container for the children that is reactive to the collapse signal.
    let children = dyn_view(move ||
        match collapsed.get() {
            false => container(toc_view(entry.children.clone(), on_toc_click.clone())),
            true => container(label(move || "").style(|s| s.width(0).height(0)))
        }
    );

    // Stack the header and children vertically.
    v_stack((header, children))
        .style(|s| s.padding_left(0).width_full().flex_col())
}