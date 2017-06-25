table! {
    use diesel::types::*;

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
    use diesel::types::*;

    locations {
        id -> BigInt,
        name -> Text,
        address -> Text,
        lat -> Double,
        lng -> Double,
    }
}

table! {
    use diesel::types::*;
    
    instructor_locations {
        id -> BigInt,
        instructor_id -> BigInt,
        location_id -> BigInt,
    }
}
