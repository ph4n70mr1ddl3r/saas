use crate::components::nav::NavBar;
use crate::components::sidebar::SideBar;
use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn Layout(module: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="app-layout">
            <NavBar/>
            <div class="app-body">
                <SideBar module={module}/>
                <main class="app-content">
                    {children()}
                </main>
            </div>
        </div>
    }
}
