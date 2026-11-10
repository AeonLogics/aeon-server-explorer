use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthQuery {
    AdminExists,
    SeedAdmin,
    LoginUser,
    CurrentUser,
    Logout,
    ChangePassword,
}

impl AuthQuery {
    pub const fn surql(self) -> &'static str {
        match self {
            Self::AdminExists => include_str!("auth/admin_exists.surql"),
            Self::SeedAdmin => include_str!("auth/seed_admin.surql"),
            Self::LoginUser => include_str!("auth/login.surql"),
            Self::CurrentUser => include_str!("auth/current_user.surql"),
            Self::Logout => include_str!("auth/logout.surql"),
            Self::ChangePassword => include_str!("auth/change_password.surql"),
        }
    }
}

impl<'a> From<AuthQuery> for Cow<'a, str> {
    fn from(query: AuthQuery) -> Self {
        Cow::Borrowed(query.surql())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditQuery {
    Record,
    List,
}

impl AuditQuery {
    pub const fn surql(self) -> &'static str {
        match self {
            Self::Record => include_str!("audit/record.surql"),
            Self::List => include_str!("audit/list.surql"),
        }
    }
}

impl<'a> From<AuditQuery> for Cow<'a, str> {
    fn from(query: AuditQuery) -> Self {
        Cow::Borrowed(query.surql())
    }
}

pub const SCHEMA: &[(&str, &str)] = &[
    ("001_user", include_str!("../../../surql_flux/schema/001_user.surql")),
    ("002_session", include_str!("../../../surql_flux/schema/002_session.surql")),
    ("003_audit", include_str!("../../../surql_flux/schema/003_audit.surql")),
];
