use floem::unit::UnitExt;
use floem::view::View;
use floem::views::{container, Decorators, stack, v_stack};

use crate::book_renderer::book_renderer;

mod book_renderer;
mod epub;
mod book_elem;


fn app_view() -> impl View {
    container(
        book_renderer("/home/johan/Hem/Downloads/A Concise History of Switzerland -- Clive H_ Church; Randolph C_ Head -- 2013 -- Cambridge University Press -- 9780521143820 -- 046e534312eeda8990a82749ebab90fc -- Anna’s Archive.epub").style(move |s|  {
            s.width(100.pct())
        }),
    ).style(move |s|  {
        s.width(100.pct())
    })
}


fn main() {
    floem::launch(app_view);
    println!("Hello, world!");
}
