use leptos::prelude::*;

#[component]
pub fn DataTable<T, F, V>(
    #[prop(into)] headers: Vec<String>,
    rows: Vec<T>,
    render_row: F,
) -> impl IntoView
where
    F: Fn(&T) -> V + 'static,
    V: IntoView + 'static,
    T: Clone + 'static,
{
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    {headers.into_iter().map(|h| view! { <th>{h}</th> }).collect_view()}
                </tr>
            </thead>
            <tbody>
                {rows.iter().map(|row| {
                    let cells = render_row(row);
                    view! { <tr>{cells}</tr> }
                }).collect_view()}
            </tbody>
        </table>
    }
}
