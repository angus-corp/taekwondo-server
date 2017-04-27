#![allow(non_upper_case_globals, non_camel_case_types, dead_code)]

use database::Database;
use diesel::expression::any;
use diesel::{self, select};
use diesel::prelude::*;
use diesel::expression::exists;
use r2d2;
use r2d2_diesel;
use schema::{users, instructor_locations};
pub use self::user_defined::*;

type DbPointer = r2d2::PooledConnection
    <r2d2_diesel::ConnectionManager<diesel::pg::PgConnection>>;

#[derive(Debug)]
pub enum Conditions {
    anyone,
    all(&'static [Conditions]),
    any(&'static [Conditions]),
    has_role(Role), // That one is sufficiently privileged.
    own_student, // Affecting a property that belongs to one's student.
    own // Affecting a property that belongs to oneself.
}

pub struct Cache<'a> {
    pub user: Option<i64>,
    pub target: Option<i64>,
    pub pool: &'a Database,
    pub database: Option<DbPointer>,
    pub role: Option<i64>,
    pub student: Option<bool>
}

impl<'a> Cache<'a> {
    pub fn empty(
        user: Option<i64>,
        target: Option<i64>,
        pool: &'a Database
    ) -> Cache<'a> {
        Cache {
            user: user,
            target: target,
            pool: pool,
            database: None,
            role: None,
            student: None
        }
    }

    pub fn same(&mut self) -> bool {
        self.user.is_some() && self.user == self.target
    }

    pub fn role(&mut self) -> Result<i64, &'static str> {
        if let Some(role) = self.role {
            return Ok(role);
        }

        //LONG: Maybe handle this case better (a has_role should return false).
        let user = match self.user {
            Some(user) => user,
            _ => return Err("unauthorized")
        };

        let role = users::table
            .find(user)
            .select(users::role)
            .get_result(&**self.database())
            .map_err(|_| "server error")?;

        self.role = Some(role);
        Ok(role)
    }

    pub fn student(&mut self) -> Result<bool, &'static str> {
        if let Some(student) = self.student {
            return Ok(student);
        }

        let (user, target) = match (self.user, self.target) {
            (Some(user), Some(target)) => (user, target),
            (_, None) => return Ok(false), // Doesn't make sense without target.
            (None, Some(_)) => return Err("unauthorized")
        };

        let exists = select(exists(
                instructor_locations::table.filter(
                    instructor_locations::location_id.nullable()
                    .eq(any( //LONG: Optimize.
                        users::table
                            .find(target)
                            .select(users::training_location)))
                    .and(instructor_locations::instructor_id.eq(user))
                )
            ))
            .get_result::<bool>(&**self.database())
            .map_err(|_| "server error")?;

        Ok(exists)
    }

    pub fn database(&mut self) -> &DbPointer {
        if let Some(ref db) = self.database {
            return db;
        }

        let db = self.pool.get().unwrap();
        self.database = Some(db);
        //LONG: No need to unwrap?
        self.database.as_ref().unwrap()
    }
}

impl Conditions {
    pub fn check(&self, cache: &mut Cache) -> Result<bool, &'static str> {
        use self::Conditions::*;

        match self {
            &anyone =>
                Ok(true),
            &all(ref parts) =>
                Ok(parts.iter().all(|x| x.check(cache) == Ok(true))),
            &any(ref parts) =>
                Ok(parts.iter().any(|x| x.check(cache) == Ok(true))),
            &has_role(minimum) =>
                Ok(minimum as i64 <= cache.role()?),
            &own_student =>
                cache.student(),
            &own =>
                Ok(cache.same())
        }
    }
}

// Credit goes to [@krdln](users.rust-lang.org/users/krdln).
macro_rules! cond {
    // end recursion
    (@array $array:tt {}) => {
        $array
    };
    
    // parse comma (and run nested cond!)
    (@array [ $($array:tt)* ] { $($current:tt)* } , $($tail:tt)*) => {
        cond!(@array [ $($array)* cond!($($current)*) , ] {} $($tail)* )
    };
    
    // parse anything else
    (@array $array:tt { $($current:tt)* } $x:tt $($tail:tt)*) => {
        cond!(@array $array { $($current)* $x } $($tail)* )
    };
    
    // add trailing comma
    (@array $array:tt $current:tt) => {
        cond!(@array $array $current ,)
    };
    
    (all( $($tt:tt)* )) => {
        all(&cond!( @array [] {} $($tt)* ))
    };
    (any( $($tt:tt)* )) => {
        any(&cond!( @array [] {} $($tt)* ))
    };
    ($x:expr) => {
        $x
    };
}

macro_rules! permissions {
    ([ $( $a:ident = $b:tt )|* ]
     $( [ $x:ident $($y:tt)* ] )*) => {
        use $crate::conditions::Conditions;
        use $crate::conditions::Conditions::*;
        use self::Role::*;

        #[derive(Copy, Clone, Debug)]
        pub enum Role {
            $($a = $b),*
        }

        pub static ROLES: &'static [Role] = &[$($a),*];

        impl Role {
            pub fn to_string(self) -> &'static str {
                match self {
                    $($a => stringify!($a)),*
                }
            }

            pub fn from_int(int: i64) -> Option<Role> {
                match int {
                    $($b => Some($a),)*
                    _ => None
                }
            }
        }

        $( pub const $x : Conditions = cond!( $($y)* ); )*
    };
}

//LONG: Something better than this dirty trick.
mod user_defined {
    #![forbid(dead_code)]
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/permissions.rs"));
}
