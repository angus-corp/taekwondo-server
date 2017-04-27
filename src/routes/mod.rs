use iron::prelude::*;
use router::Router;
use juniper::iron_handlers::{GraphQLHandler, GraphiQLHandler};
use self::query::{Mutate, Query, Context};

mod auth;
mod query;

pub fn build() -> Router {
    let mut router = Router::new();

    router.post("/auth/login", auth::login, "auth/login");
    router.post("/auth/forgot", auth::forgot, "auth/forgot");
    router.post("/auth/reset", auth::reset, "auth/reset");

    let graphql = GraphQLHandler::new(ctxfactory, Query, Mutate);
    let graphiql = GraphiQLHandler::new("/");
    router.post("/", graphql, "graphql");
    router.get("/graphiql", graphiql, "graphiql");

    router
}

fn ctxfactory(req: &mut Request) -> Context {
    Context::from(req)
}
