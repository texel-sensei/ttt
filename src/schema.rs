// @generated automatically by Diesel CLI.

diesel::table! {
    frames (id) {
        id -> Integer,
        project -> Integer,
        start -> Timestamp,
        end -> Nullable<Timestamp>,
    }
}

diesel::table! {
    projects (id) {
        id -> Integer,
        name -> Text,
        archived -> Bool,
        last_access_time -> Timestamp,
    }
}

diesel::joinable!(frames -> projects (project));

diesel::allow_tables_to_appear_in_same_query!(
    frames,
    projects,
);
