use leptos::prelude::*;
use leptos::children::ChildrenFn;

#[component]
pub fn Modal(
    #[prop(into)] title: String,
    show: Signal<bool>,
    children: ChildrenFn,
) -> impl IntoView {
    // Wrap in a signal so it can be cloned inside a Fn closure
    let title = RwSignal::new(title);
    view! {
        <Show when=move || show.get()>
            <div class="modal-overlay">
                <div class="modal">
                    <div class="modal-header">
                        <h3>{move || title.get()}</h3>
                    </div>
                    <div class="modal-body">
                        {children()}
                    </div>
                </div>
            </div>
        </Show>
    }
}
