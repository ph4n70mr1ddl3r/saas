use leptos::prelude::*;

#[component]
pub fn TextInput(
    #[prop(into)] label: String,
    #[prop(into)] name: String,
    #[prop(optional)] value: String,
    #[prop(optional, into)] input_type: Option<String>,
) -> impl IntoView {
    let input_type = input_type.unwrap_or_else(|| "text".to_string());
    view! {
        <div class="form-group">
            <label for={name.clone()}>{label}</label>
            <input type={input_type} id={name.clone()} name={name} value={value} />
        </div>
    }
}

#[component]
pub fn SubmitButton(#[prop(into)] label: String) -> impl IntoView {
    view! {
        <button type="submit" class="btn btn-primary">{label}</button>
    }
}
