use config::Config;
use diesel::pg::PgConnection;
use r2d2;
use r2d2_diesel::ConnectionManager;
use iron::prelude::*;
use iron::typemap::Key;
use iron::BeforeMiddleware;
use std::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct Database(pub r2d2::Pool<ConnectionManager<PgConnection>>);

impl Database {
    pub fn new(config: &Config) -> Result<Database, r2d2::InitializationError> {
        let db_url = config.database_url.as_str();
        let db_manager = ConnectionManager::new(db_url);
        let db = r2d2::Pool::new(Default::default(), db_manager)?;
        Ok(Database(db))
    }
}

impl BeforeMiddleware for Database {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        req.extensions
            .entry::<Database>()
            .or_insert(self.clone());

        Ok(())
    }
}

impl Key for Database {
    type Value = Database;
}

impl Deref for Database {
    type Target = r2d2::Pool<ConnectionManager<PgConnection>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for Database {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
