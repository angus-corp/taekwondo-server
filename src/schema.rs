use diesel::types::*;

table! {
    users {
        id -> BigInt,
        first_name -> Text,
        last_name -> Text,
        username -> Text,
        email -> Text,
        password -> Binary,
        training_location -> Nullable<BigInt>,
        role -> BigInt,
    }
}

table! {
    locations {
        id -> BigInt,
        name -> Text,
        address -> Text,
        lat -> Double,
        lng -> Double,
    }
}

table! {
    instructor_locations {
        id -> BigInt,
        instructor_id -> BigInt,
        location_id -> BigInt,
    }
}

sql_function!(
    lower,
    LowerT,
    (a: Text) -> Text
);
