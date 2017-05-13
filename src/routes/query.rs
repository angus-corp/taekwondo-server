use auth::{self, SealedCookie};
use conditions::{self, Cache};
use config::Config;
use database::Database;
use iron::prelude::*;
use juniper;
use persistent::Read;
use diesel::{self, select};
use diesel::expression::exists;
use diesel::prelude::*;
use diesel::pg::PgQueryBuilder;
use diesel::query_builder::BuildQueryResult;
use diesel::query_builder::QueryFragment;
use diesel::types::Float;
use diesel::types::Text;
use diesel::expression::AsExpression;
use diesel::query_builder::QueryBuilder;
use diesel::pg::Pg;
use std::sync::Mutex;
use schema::{users, locations, instructor_locations};

//LONG: Move the caches into the context or something, to improve performance.
//LONG: MODULARISE.
//LONG: More robust solution to case-insensitive usernames and emails.

//LONG: Stop being lazy.
macro_rules! sleiss {
    ($req:expr, $offset:expr, $limit:expr, $db:expr) => {
        match ($offset, $limit) {
            (Some(offset), Some(limit)) =>
                $req.offset(offset).limit(limit).get_results($db),
            (Some(offset), None) =>
                $req.offset(offset).get_results($db),
            (None, Some(limit)) =>
                $req.limit(limit).get_results($db),
            (None, None) =>
                $req.get_results($db)
        }
    }
}

pub struct Context {
    database: Database,
    user: Option<i64>
}

pub struct Query;
pub struct Mutate;

impl juniper::Context for Context {}

impl<'a, 'b, 'c> From<&'a mut Request<'b, 'c>> for Context {
    fn from(req: &mut Request) -> Context {
        let user = if let Some(sealed) = req.headers.get::<SealedCookie>() {
            let config = req.extensions.get::<Read<Config>>().unwrap();
            match sealed.unseal(config.secret) {
                Ok(token) if token.valid() => Some(token.id),
                _ => None
            }
        } else {
            None
        };

        Context {
            database: req.extensions.get::<Database>().unwrap().clone(),
            user: user
        }
    }
}

#[derive(Identifiable, Queryable, Associations)]
#[has_many(instructor_locations, foreign_key="instructor_id")]
#[table_name="users"]
pub struct User {
    id: i64,
    first_name: String,
    last_name: String,
    username: String,
    email: String,
    password: Vec<u8>,
    training_location: Option<i64>,
    role: i64
}

pub struct UserWrapper<'a> {
    user: User,
    cache: Mutex<Cache<'a>>
}

#[derive(Identifiable, Queryable, Associations)]
#[has_many(users, foreign_key="training_location")]
#[has_many(instructor_locations, foreign_key="location_id")]
#[table_name="locations"]
pub struct Location {
    id: i64,
    name: String,
    address: String,
    lat: f64,
    lng: f64
}

#[derive(Queryable, Identifiable, Associations)]
#[belongs_to(User, foreign_key="instructor_id")]
#[belongs_to(Location, foreign_key="location_id")]
#[table_name="instructor_locations"]
pub struct InstructorLocation {
    id: i64,
    instructor_id: i64,
    location_id: i64
}

pub struct Role(conditions::Role);

graphql_object!(Role: Context as "Role" |&self| {
    description: "A role, such as 'admin', that defines users' privileges."

    field id() -> i64
    as "A unique numeric ID." {
        self.0 as i64
    }

    field name() -> &str
    as "A snake-cased name." {
        self.0.to_string()
    }
});

graphql_object!(Location: Context as "Location" |&self| {
    description: "A place where people train."

    field id() -> i64
    as "A unique numeric ID for the location." {
        self.id as i64
    }

    field name() -> &str
    as "The name of the location." {
        &self.name
    }

    field address() -> &str
    as "A human-readable address." {
        &self.address
    }

    field lat() -> f64
    as "The latitude of the location." {
        self.lat
    }

    field lng() -> f64
    as "The longitude of the location." {
        self.lng
    }

    field instructors(&executor) -> Result<Vec<UserWrapper>, String>
    as "The people instructing at this location." {
        let ctx = executor.context();
        let mut first_cache = cache(ctx, None); //LONG: Should have targets.
        check(&mut first_cache, conditions::read_instructors)?;
        instructor_locations::table
            .inner_join(users::table)
            .filter(instructor_locations::location_id.eq(self.id))
            .select((
                users::id,
                users::first_name,
                users::last_name,
                users::username,
                users::email,
                users::password,
                users::training_location,
                users::role
            ))
            .get_results(&**first_cache.database())
            .map(|users| users.into_iter()
                .map(|user: User| UserWrapper {
                    cache: Mutex::new(Cache {
                        user: ctx.user,
                        target: Some(user.id),
                        pool: first_cache.pool,
                        database: None, //LONG: Share database connection.
                        role: first_cache.role,
                        student: None
                    }),
                    user: user
                }).collect())
            .map_err(stringify_error)
    }

    field students(&executor) -> Result<Vec<UserWrapper>, String>
    as "The people training at this location." {
        let ctx = executor.context();

        let mut first_cache = cache(ctx, None);
        if let Some(user) = first_cache.user {
            first_cache.student = Some(
                select(exists(
                    instructor_locations::table.filter(
                        instructor_locations::location_id.eq(self.id)
                        .and(instructor_locations::instructor_id.eq(user))
                    )
                ))
                .get_result(&**first_cache.database())
                .map_err(stringify_error)?
            );
        } else {
            first_cache.student = Some(false);
        }

        check(&mut first_cache, conditions::read_students)?;
        users::table
            .filter(users::training_location.eq(self.id))
            .get_results(&**first_cache.database())
            .map(|users| users.into_iter()
                .map(|user: User| UserWrapper {
                    cache: Mutex::new(Cache {
                        user: ctx.user,
                        target: Some(user.id),
                        pool: first_cache.pool,
                        database: None, //LONG: Share database connection.
                        role: first_cache.role,
                        student: first_cache.student
                    }),
                    user: user
                }).collect())
            .map_err(stringify_error)
    }
});

graphql_object!(<'a> UserWrapper<'a>: Context as "User" |&self| {
    description: "A user (forbidden fields will be nulled)."

    field id() -> i64
    as "A unique numeric ID for the user." {
        self.user.id
    }

    field firstName(&executor) -> Option<&str>
    as "The user's first name." {
        check(&mut *self.cache.lock().unwrap(), conditions::read_name)
            .ok()
            .and(Some(&*self.user.first_name))
    }

    field lastName(&executor) -> Option<&str>
    as "The user's last name." {
        check(&mut *self.cache.lock().unwrap(), conditions::read_name)
            .ok()
            .and(Some(&*self.user.last_name))
    }

    field username(&executor) -> Option<&str>
    as "The user's username." {
        check(&mut *self.cache.lock().unwrap(), conditions::read_username)
            .ok()
            .and(Some(&*self.user.username))
    }

    field email(&executor) -> Option<&str>
    as "The user's email address." {
        check(&mut *self.cache.lock().unwrap(), conditions::read_email_address)
            .ok()
            .and(Some(&*self.user.email))
    }

    field role(&executor) -> Option<Role>
    as "The user's role." {
        let mut cache = self.cache.lock().unwrap();
        check(&mut *cache, conditions::read_role)
            .ok()
            .and(conditions::Role::from_int(self.user.role).map(Role))
    }

    field training_location(&executor) -> Result<Option<Location>, String>
    as "The place where the user trains." {
        let mut cache = self.cache.lock().unwrap();
        check(&mut cache, conditions::read_students)?;
        if let Some(id) = self.user.training_location {
            locations::table
                .find(id)
                .first(&**cache.database())
                .map(Some)
                .map_err(stringify_error)
        } else {
            Ok(None)
        }
    }

    field instructing_locations(&executor) -> Result<Vec<Location>, String>
    as "The places where the user is an instructor." {
        let mut cache = self.cache.lock().unwrap();
        check(&mut cache, conditions::read_instructors)?;
        instructor_locations::table
            .inner_join(locations::table)
            .filter(instructor_locations::instructor_id.eq(self.user.id))
            .select((
                locations::id,
                locations::name,
                locations::address,
                locations::lat,
                locations::lng
            ))
            .get_results(&**cache.database())
            .map_err(stringify_error)
    }
});

graphql_object!(Query: Context as "Query" |&self| {
    field roles(&executor) -> Vec<Role>
    as "A list of roles." {
        //LONG: Add a permission `read_roles` or something.
        //LONG: Surely there is an easier way to do this.
        conditions::ROLES.iter().cloned().map(Role).collect()
    }

    //LONG: Consider moving the read_location_info checks into the actual reading functions.
    field location(&executor, id: i64) -> Result<Location, String>
    as "The location with the given ID." {
        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::read_location_info)?;
        locations::table.find(id)
            .first(&**cache.database())
            .map_err(stringify_error)
    }

    //LONG: Way to search and filter locations.
    field locations(
        &executor,
        offset: Option<i64>,
        limit: Option<i64>
    ) -> Result<Vec<Location>, String>
    as "All the locations, alphabetized by name." {
        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::read_location_info)?;
        let db = &**cache.database();
        sleiss!(locations::table.order(locations::name), offset, limit, db)
            .map_err(stringify_error)
    }

    field user(&executor, id: Option<i64>) -> Result<UserWrapper, String>
    as "The user with the given ID, or oneself if an ID is not given." {
        let ctx = executor.context();
        let id =
            if let Some(id) = id { id }
            else if let Some(user) = ctx.user { user }
            else { return Err("unauthorized".to_owned()) };

        let mut cache = cache(ctx, Some(id));
        users::table.find(id)
            .first(&**cache.database())
            .map(|user| UserWrapper {
                user: user,
                cache: Mutex::new(cache)
            }).map_err(stringify_error)
    }

    //LONG: Maybe add more constraints.
    field users(
        &executor,
        offset: Option<i64>,
        limit: Option<i64>,
        query: Option<String>
    ) -> Result<Vec<UserWrapper>, String>
    as "All the users." {
        let ctx = executor.context();
        let mut first_cache = cache(ctx, None);
        let res = if let Some(query) = query {
            check(&mut first_cache, conditions::search_users)?;
            sleiss!(
                users::table.order(
                    word_similarity(query, users::username
                        .concat(" ")
                        .concat(users::first_name)
                        .concat(" ")
                        .concat(users::last_name)).desc()),
                offset, limit, &**first_cache.database()
            )
        } else {
            let db = &**first_cache.database();
            sleiss!(users::table.order(users::id), offset, limit, db)
        };

        res.map(|users| users.into_iter()
            .map(|user: User| UserWrapper {
                cache: Mutex::new(cache(ctx, Some(user.id))),
                user: user
            }).collect())
        .map_err(stringify_error)
    }
});

graphql_object!(Mutate: Context as "Mutate" |&self| {
    field addUser(
        &executor,
        first_name: String,
        last_name: String,
        username: String,
        password: String,
        email: String
    ) -> Result<UserWrapper, String>
    as "Add a user." {
        #[derive(Insertable)]
        #[table_name="users"]
        struct NewUser<'a> {
            first_name: String,
            last_name: String,
            username: String,
            email: String,
            password: &'a [u8]
        }

        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::create_user)?;

        let hash = auth::hash(password.as_bytes());
        let new_user = NewUser {
            first_name: first_name,
            last_name: last_name,
            username: username.to_lowercase(),
            password: hash.as_ref(),
            email: email.to_lowercase()
        };

        diesel::insert(&new_user)
            .into(users::table)
            .get_result(&**cache.database())
            .map(|user| UserWrapper {
                user: user,
                cache: Mutex::new(cache)
            })
            .map_err(stringify_error)
    }

    field removeUser(
        &executor,
        id: i64
    ) -> Result<UserWrapper, String>
    as "Remove the user with the given ID." {
        let mut cache = cache(executor.context(), Some(id));
        check(&mut cache, conditions::delete_user)?;
        diesel::delete(users::table.find(id))
            .get_result(&**cache.database())
            .map(|user| UserWrapper {
                user: user,
                cache: Mutex::new(cache)
            })
            .map_err(stringify_error)
    }

    field editUser(
        &executor,
        id: i64,
        first_name: Option<String>,
        last_name: Option<String>,
        username: Option<String>,
        password: Option<String>,
        email: Option<String>,
        role: Option<i64>
    ) -> Result<UserWrapper, String>
    as "Update the attributes of the user with the given ID." {
        #[derive(AsChangeset)]
        #[table_name="users"]
        struct UserChanges<'a> {
            first_name: Option<String>,
            last_name: Option<String>,
            username: Option<String>,
            password: Option<&'a [u8]>,
            email: Option<String>,
            role: Option<i64>
        }

        let mut cache = cache(executor.context(), Some(id));
        if first_name.is_some() || last_name.is_some() {
            check(&mut cache, conditions::edit_name)?;
        }
        if username.is_some() {
            check(&mut cache, conditions::edit_username)?;
        }
        if email.is_some() {
            check(&mut cache, conditions::edit_email_address)?;
        }
        if password.is_some() {
            check(&mut cache, conditions::edit_password)?;
        }
        if role.is_some() {
            check(&mut cache, conditions::edit_role)?;
        }

        let hash = password.map(|x| auth::hash(x.as_bytes()));
        let changes = UserChanges {
            first_name: first_name,
            last_name: last_name,
            username: username.map(|x| x.to_lowercase()),
            password: hash.as_ref().map(|x| x.as_ref()),
            email: email.map(|x| x.to_lowercase()),
            role: role
        };

        diesel::update(users::table.find(id))
            .set(&changes)
            .get_result(&**cache.database())
            .map(|user| UserWrapper {
                user: user,
                cache: Mutex::new(cache)
            })
            .map_err(stringify_error)
    }

    field editTrainingLocation(
        &executor,
        student: i64,
        location: Option<i64>
    ) -> Result<UserWrapper, String>
    as "Set the user's training location. Pass null to unset." {
        #[derive(AsChangeset)]
        #[table_name="users"]
        struct UserChanges {
            training_location: Option<Option<i64>>
        }

        let mut cache = cache(executor.context(), Some(student));
        check(&mut cache, conditions::edit_students)?;

        diesel::update(users::table.find(student))
            .set(&UserChanges { training_location: Some(location) })
            .get_result(&**cache.database())
            .map(|user| UserWrapper {
                user: user,
                cache: Mutex::new(cache)
            })
            .map_err(stringify_error)
    }

    field addLocation(
        &executor,
        name: String,
        address: String,
        lat: f64,
        lng: f64
    ) -> Result<Location, String>
    as "Add a location." {
        #[derive(Insertable)]
        #[table_name="locations"]
        struct NewLocation {
            name: String,
            address: String,
            lat: f64,
            lng: f64
        }

        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::create_location)?;

        let new_location = NewLocation {
            name: name,
            address: address,
            lat: lat,
            lng: lng
        };

        diesel::insert(&new_location)
            .into(locations::table)
            .get_result(&**cache.database())
            .map_err(stringify_error)
    }

    field removeLocation(
        &executor,
        id: i64
    ) -> Result<Location, String>
    as "Remove the location with the given ID." {
        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::delete_location)?;
        diesel::delete(locations::table.find(id))
            .get_result(&**cache.database())
            .map_err(stringify_error)
    }

    field editLocation(
        &executor,
        id: i64,
        name: Option<String>,
        address: Option<String>,
        lat: Option<f64>,
        lng: Option<f64>
    ) -> Result<Location, String>
    as "Edit the location with the given ID." {
        #[derive(AsChangeset)]
        #[table_name="locations"]
        struct LocationChanges {
            name: Option<String>,
            address: Option<String>,
            lat: Option<f64>,
            lng: Option<f64>
        }
        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::edit_location_info)?;
        let changes = LocationChanges {
            name: name,
            address: address,
            lat: lat,
            lng: lng
        };
        diesel::update(locations::table.find(id))
            .set(&changes)
            .get_result(&**cache.database())
            .map_err(stringify_error)
    }

    field assignInstructor(
        &executor,
        user: i64,
        location: i64
    ) -> Result<(), String>
    as "Assign a user to instruct at a particular location." {
        #[derive(Insertable)]
        #[table_name="instructor_locations"]
        struct NewAssignment {
            instructor_id: i64,
            location_id: i64
        }

        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::edit_instructors)?;

        diesel::insert(&NewAssignment {
            instructor_id: user,
            location_id: location
        })
        .into(instructor_locations::table)
        .execute(&**cache.database())
        .map_err(stringify_error)?;
        Ok(())
    }

    field unassignInstructor(
        &executor,
        user: i64,
        location: i64
    ) -> Result<(), String>
    as "Remove an instructor from a particular location." {
        let mut cache = cache(executor.context(), None);
        check(&mut cache, conditions::edit_instructors)?;
        diesel::delete(instructor_locations::table.filter(
            instructor_locations::instructor_id.eq(user)
            .and(instructor_locations::location_id.eq(location))
        ))
        .execute(&**cache.database())
        .map_err(stringify_error)?;
        Ok(())
    }
});

#[inline]
fn stringify_error(err: diesel::result::Error) -> String {
    error!("Database Error: {:?}.", err);
    use diesel::result::{Error, DatabaseErrorKind};
    (match err {
        Error::NotFound => "not found",
        Error::DatabaseError(DatabaseErrorKind::UniqueViolation, ..) =>
            "unique violation",
        _ => "server error"
    }).to_owned()
}

#[inline]
fn cache<'a>(ctx: &'a Context, target: Option<i64>) -> Cache<'a> {
    Cache::empty(ctx.user, target, &ctx.database)
}

#[inline]
fn check(
    cache: &mut Cache,
    perm: conditions::Conditions
) -> Result<(), String> {
    perm.check(cache)
        .and_then(|b| if b { Ok(()) } else { Err("unauthorized") })
        .map_err(Into::<String>::into)
}

pub fn word_similarity<T, U>(l: T, r: U)
-> WordSim<T::Expression, U::Expression>
where T: AsExpression<Text>, U: AsExpression<Text> {
    WordSim(l.as_expression(), r.as_expression())
}

pub struct WordSim<T, U>(T, U);

impl<L, R> Expression for WordSim<L, R> {
    type SqlType = Float;
}

impl<L, R> QueryFragment<Pg> for WordSim<L, R> where
    L: QueryFragment<Pg>, R: QueryFragment<Pg>
{
    fn to_sql(&self, out: &mut PgQueryBuilder) -> BuildQueryResult {
        out.push_sql("word_similarity(");
        self.0.to_sql(out)?;
        out.push_sql(",");
        self.1.to_sql(out)?;
        out.push_sql(")");
        Ok(())
    }

    fn collect_binds(&self, out: &mut <Pg as diesel::backend::Backend>::BindCollector) -> QueryResult<()> {
        try!(self.0.collect_binds(out));
        try!(self.1.collect_binds(out));
        Ok(())
    }

    fn is_safe_to_cache_prepared(&self) -> bool {
        self.0.is_safe_to_cache_prepared() && self.1.is_safe_to_cache_prepared()
    }
}

impl_query_id!(WordSim<T, U>);

impl<T, U, QS> SelectableExpression<QS>
    for WordSim<T, U> where
        WordSim<T, U>: AppearsOnTable<QS>,
        T: SelectableExpression<QS>,
        U: SelectableExpression<QS>
{}

impl<T, U, QS> AppearsOnTable<QS>
    for WordSim<T, U> where
        WordSim<T, U>: Expression,
        T: AppearsOnTable<QS>,
        U: AppearsOnTable<QS>
{}
