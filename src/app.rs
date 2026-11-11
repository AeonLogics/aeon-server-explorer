use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    ParamSegment, StaticSegment,
};

use crate::components::auth::AuthState;
use crate::pages::{
    AuditPage, Gate, ImagesPage, NotFound, OverviewPage, ProjectPage, ServicePage, StackPage, StacksPage,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="theme-color" content="#f4f2f9"/>
                <meta name="robots" content="noindex, nofollow"/>
                <link rel="icon" href="/favicon.ico" sizes="32x32"/>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml"/>
                <link rel="apple-touch-icon" href="/apple-touch-icon.png"/>
                <link rel="manifest" href="/site.webmanifest"/>
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin=""/>
                <link
                    rel="stylesheet"
                    href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,400..700&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap"
                />
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    AuthState::provide();

    view! {
        <Stylesheet id="leptos" href="/pkg/explorer.css"/>
        <Title text="Server Explorer · Aeon Logics"/>

        <Router>
            <Routes fallback=NotFound>
                <ParentRoute path=StaticSegment("") view=Gate>
                    <Route path=StaticSegment("") view=OverviewPage />
                    <Route path=StaticSegment("audit") view=AuditPage />
                    <Route path=StaticSegment("stacks") view=StacksPage />
                    <Route path=StaticSegment("images") view=ImagesPage />
                    <Route path=(StaticSegment("stacks"), ParamSegment("stack")) view=StackPage />
                    <Route path=(StaticSegment("p"), ParamSegment("project")) view=ProjectPage />
                    <Route
                        path=(StaticSegment("p"), ParamSegment("project"), StaticSegment("s"), ParamSegment("container"))
                        view=ServicePage
                    />
                </ParentRoute>
            </Routes>
        </Router>
    }
}
