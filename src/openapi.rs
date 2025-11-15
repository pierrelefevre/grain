use utoipa::OpenApi;

use crate::{admin, state};

#[derive(OpenApi)]
#[openapi(
    paths(
        admin::list_users,
        admin::create_user,
        admin::delete_user,
        admin::add_permission
    ),
    components(
        schemas(
            admin::CreateUserRequest,
            admin::AddPermissionRequest,
            state::User,
            state::Permission
        )
    ),
    tags(
        (name = "admin", description = "User and permission management endpoints")
    ),
    info(
        title = "Grain Registry - Admin API",
        version = "0.1.0",
        description = "Administration API for the Grain registry. Provides endpoints for managing users and their granular tag-level permissions.",
        contact(
            name = "Grain Registry",
            url = "https://github.com/pierrelefevre/grain"
        ),
        license(
            name = "MIT"
        )
    ),
    servers(
        (url = "/", description = "Local server")
    ),
    security(
        ("basic_auth" = [])
    ),
    modifiers(&SecurityAddon)
)]
pub struct AdminApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "basic_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Basic,
                    ),
                ),
            );
        }
    }
}
