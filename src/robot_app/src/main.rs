use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = signal(0);

    view! {
        <button on:click=move |_| {
            set_count.set(3)
        }>

            "Click me: " {count}
        </button>
        <p>"Double count: " {move || count.get() * 2}</p>
    }
}

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    leptos::mount::mount_to_body(|| {
        view! { <p>"Hello, world!"</p> }
    })
}
