use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::Router;
        use leptos::*;
        use leptos_axum::{generate_route_list, LeptosRoutes};
        use saas_frontend::app::App;

        #[tokio::main]
        async fn main() {
            simple_logger::init_with_level(log::Level::Info).expect("couldn't initialize logging");

            let conf = get_configuration(None).await.unwrap();
            let leptos_options = conf.leptos_options;
            let addr = leptos_options.site_addr;
            let routes = generate_route_list(App);

            let app = Router::new()
                .leptos_routes(&leptos_options, routes, App)
                .fallback(leptos_axum::file_and_error_handler)
                .with_state(leptos_options);

            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            log::info!("Frontend listening on {}", addr);
            axum::serve(listener, app).await.unwrap();
        }
    } else if #[cfg(feature = "hydrate")] {
        use leptos::*;
        use saas_frontend::app::App;

        pub fn main() {
            console_error_panic_hook::set_once();
            mount_to_body(|| {
                view! { <App/> }
            });
        }
    } else {
        fn main() {
            // No feature enabled. Build with --features ssr or --features hydrate.
            println!("Build with --features ssr for server-side rendering or --features hydrate for client-side hydration.");
        }
    }
}
