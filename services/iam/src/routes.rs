use crate::handlers;
use crate::service::{AuthService, RoleService, UserService};
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthState {
    pub auth_service: AuthService,
    pub user_service: UserService,
    pub role_service: RoleService,
}

/// Build services from pool and bus, returning them for event registration.
pub fn build_services(
    pool: SqlitePool,
    bus: NatsBus,
) -> (AuthService, UserService, RoleService) {
    let auth_service = AuthService::new(pool.clone(), bus.clone());
    let user_service = UserService::new(pool.clone(), bus.clone());
    let role_service = RoleService::new(pool, bus);
    (auth_service, user_service, role_service)
}

/// Build a router from pre-created services.
pub fn build_router_from_services(
    auth_service: AuthService,
    user_service: UserService,
    role_service: RoleService,
) -> Router {
    Router::<AuthState>::new()
        // Auth routes (public)
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/refresh", post(handlers::auth::refresh))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        // User routes
        .route(
            "/api/v1/users",
            get(handlers::users::list_users).post(handlers::users::create_user),
        )
        .route(
            "/api/v1/users/{id}",
            get(handlers::users::get_user)
                .put(handlers::users::update_user)
                .delete(handlers::users::delete_user),
        )
        .route(
            "/api/v1/users/{id}/password",
            put(handlers::users::change_password),
        )
        .route(
            "/api/v1/users/{id}/roles",
            put(handlers::users::assign_roles),
        )
        // Role routes
        .route(
            "/api/v1/roles",
            get(handlers::roles::list_roles).post(handlers::roles::create_role),
        )
        .route(
            "/api/v1/roles/{id}",
            get(handlers::roles::get_role)
                .put(handlers::roles::update_role)
                .delete(handlers::roles::delete_role),
        )
        .route(
            "/api/v1/roles/{id}/permissions",
            put(handlers::roles::set_permissions),
        )
        .route(
            "/api/v1/permissions",
            get(handlers::roles::list_permissions),
        )
        // Health
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
        )
        .with_state(AuthState {
            auth_service,
            user_service,
            role_service,
        })
}
